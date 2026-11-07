//! The peer on X11: one x11rb connection. A thread blocks on X events and the stdin thread
//! reads commands; both feed one channel that the main thread works through, so all state
//! lives on one thread.
//!
//! Selections are `CLIPBOARD` and `PRIMARY`, with `TARGETS` and INCR both ways. Drag and drop is
//! XDND version 5: the source runs on the implicit grab of the press on its own window.
//!
//! `map` replies once the window is in the root's `_NET_CLIENT_LIST`, which wlroots updates
//! when the compositor maps the window's `wl_surface`. Up to wlroots 0.20.2, a window whose
//! first buffer reaches the compositor before its pairing with the X window (they travel on
//! different sockets) is never mapped in sway; wlroots fixed it after 0.20.2 ("no
//! synchronization between the two" in `xwayland_surface_associate`). A window that isn't
//! listed within [`MAP_RETRY`] is unmapped and mapped again, which pairs a new surface.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use strata_test_peer::protocol::{
    Action, Backend, Command, Event, Exported, Mapped, Offer, Request, Selection, Targets, Via,
};
use x11rb::connection::Connection as _;
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xfixes::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux,
    ConnectionExt as _, CreateWindowAux, EventMask, NotifyMode, PropMode, Property,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::{out, read_stdin};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        CLIPBOARD, PRIMARY, TARGETS, TIMESTAMP, MULTIPLE, INCR, UTF8_STRING,
        _NET_WM_NAME, _NET_WM_PID, _NET_CLIENT_LIST,
        XdndAware, XdndTypeList, XdndSelection, XdndEnter, XdndPosition, XdndStatus,
        XdndLeave, XdndDrop, XdndFinished, XdndActionCopy, XdndActionMove, XdndActionAsk,
    }
}

/// Data larger than this goes by INCR, in chunks this size. It stays well under the core
/// protocol's request limit (256 KiB) without BIG-REQUESTS.
const INCR_CHUNK: usize = 64 * 1024;

const XDND_VERSION: u32 = 5;

/// How long a mapped window may stay out of `_NET_CLIENT_LIST` before the peer maps it again.
const MAP_RETRY: Duration = Duration::from_millis(500);

/// How many times the peer maps its window before `map` fails.
const MAP_ATTEMPTS: u32 = 10;

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("x11: {e}")
}

enum Input {
    Command(Request),
    X(XEvent),
    Lost(String),
}

/// What a selection conversion the peer asked for is for.
enum Conversion {
    Targets(u64),
    Read(u64),
    Drop {
        source: Window,
        mime: String,
        action: Option<Action>,
    },
}

struct IncrOut {
    requestor: Window,
    property: Atom,
    target: Atom,
    data: Vec<u8>,
    offset: usize,
}

struct DropIn {
    source: Window,
    mime: Option<String>,
    action: Option<Action>,
}

struct DragOut {
    mimes: Vec<String>,
    action: Action,
    target: Option<Window>,
    accepted: bool,
}

struct X {
    conn: Arc<RustConnection>,
    atoms: Atoms,
    root: Window,
    win: Window,
    /// The offers for each selection the peer owns (`CLIPBOARD`, `PRIMARY`, `XdndSelection`).
    owned: HashMap<Atom, Vec<Offer>>,
    /// Conversions in flight, by the property they land in, with the selection and target.
    pending: HashMap<Atom, (Atom, Atom, Conversion)>,
    incr_in: HashMap<Atom, (Conversion, Vec<u8>)>,
    incr_out: Vec<IncrOut>,
    next_property: u32,
    map_reply: Option<u64>,
    /// While `map` waits: when to map again, and how many times it has mapped.
    map_retry: Option<(Instant, u32)>,
    pressed: bool,
    drop_policy: Option<(Vec<String>, Action)>,
    drop: Option<DropIn>,
    drag: Option<DragOut>,
}

pub fn run() -> Result<(), String> {
    let (conn, screen) = x11rb::connect(None).map_err(err)?;
    let conn = Arc::new(conn);
    let root = conn.setup().roots[screen].root;
    let atoms = Atoms::new(conn.as_ref())
        .map_err(err)?
        .reply()
        .map_err(err)?;
    let win = conn.generate_id().map_err(err)?;
    let mask = EventMask::PROPERTY_CHANGE
        | EventMask::STRUCTURE_NOTIFY
        | EventMask::FOCUS_CHANGE
        | EventMask::BUTTON_PRESS
        | EventMask::BUTTON_RELEASE
        | EventMask::POINTER_MOTION;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        win,
        root,
        0,
        0,
        300,
        200,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new()
            .background_pixel(0x0080_8080)
            .event_mask(mask),
    )
    .map_err(err)?;
    conn.change_property32(
        PropMode::REPLACE,
        win,
        atoms.XdndAware,
        AtomEnum::ATOM,
        &[XDND_VERSION],
    )
    .map_err(err)?;
    conn.xfixes_query_version(5, 0)
        .map_err(err)?
        .reply()
        .map_err(err)?;
    let owner_changes = xfixes::SelectionEventMask::SET_SELECTION_OWNER
        | xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
        | xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE;
    for selection in [atoms.CLIPBOARD, atoms.PRIMARY] {
        conn.xfixes_select_selection_input(win, selection, owner_changes)
            .map_err(err)?;
    }
    // The window manager lists mapped windows in the root's _NET_CLIENT_LIST.
    conn.change_window_attributes(
        root,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )
    .map_err(err)?;
    conn.flush().map_err(err)?;

    let (tx, rx) = channel();
    let commands = tx.clone();
    read_stdin(
        move |req| {
            let _ = commands.send(Input::Command(req));
        },
        || {},
    );
    let events = conn.clone();
    std::thread::spawn(move || {
        loop {
            match events.wait_for_event() {
                Ok(e) => {
                    if tx.send(Input::X(e)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Input::Lost(e.to_string()));
                    break;
                }
            }
        }
    });

    out::event(Event::Ready {
        backend: Backend::X11,
        pid: std::process::id(),
        globals: Vec::new(),
        data_control: None,
    });
    let mut x = X {
        conn,
        atoms,
        root,
        win,
        owned: HashMap::new(),
        pending: HashMap::new(),
        incr_in: HashMap::new(),
        incr_out: Vec::new(),
        next_property: 0,
        map_reply: None,
        map_retry: None,
        pressed: false,
        drop_policy: None,
        drop: None,
        drag: None,
    };
    loop {
        let wait = x.map_retry.map_or(Duration::from_secs(3600), |(at, _)| {
            at.saturating_duration_since(Instant::now())
        });
        let input = match rx.recv_timeout(wait) {
            Ok(input) => input,
            Err(RecvTimeoutError::Timeout) => {
                x.map_again();
                let _ = x.conn.flush();
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match input {
            Input::Command(req) => {
                if !x.command(req) {
                    break;
                }
            }
            Input::X(e) => x.event(e),
            Input::Lost(e) => return Err(format!("x11 connection lost: {e}")),
        }
        let _ = x.conn.flush();
    }
    Ok(())
}

impl X {
    fn command(&mut self, req: Request) -> bool {
        let id = req.id;
        let result = match req.command {
            Command::Quit => {
                out::ok(id);
                return false;
            }
            Command::Sync => self
                .conn
                .get_input_focus()
                .map_err(err)
                .and_then(|c| c.reply().map_err(err))
                .map(|_| out::ok(id)),
            Command::Map {
                title,
                width,
                height,
            } => self.map(id, &title, width, height),
            Command::Devices { .. }
            | Command::PointerMove { .. }
            | Command::PointerButton { .. }
            | Command::PointerPath { .. }
            | Command::Key { .. }
            | Command::Type { .. } => Err(
                "input comes from a Wayland peer's virtual devices; Xwayland passes it on".into(),
            ),
            Command::SelectionSet {
                selection,
                via,
                offers,
            } => self.selection_set(id, selection, via, offers),
            Command::SelectionTargets { selection, via } => Self::x11_only(via).and_then(|()| {
                let target = self.atoms.TARGETS;
                self.convert(
                    self.selection_atom(selection),
                    target,
                    Conversion::Targets(id),
                )
            }),
            Command::SelectionRead {
                selection,
                via,
                mime,
            } => Self::x11_only(via).and_then(|()| {
                let target = self.atom(&mime)?;
                self.convert(self.selection_atom(selection), target, Conversion::Read(id))
            }),
            Command::DragStart { offers, actions } => self.drag_start(id, offers, actions),
            Command::DropAccept { mimes, action } => {
                self.drop_policy = (!mimes.is_empty()).then_some((mimes, action));
                out::ok(id);
                Ok(())
            }
            Command::Export => {
                out::value(
                    id,
                    Exported {
                        handle: format!("x11:{:x}", self.win),
                    },
                );
                Ok(())
            }
        };
        if let Err(e) = result {
            out::reply(id, Err(e));
        }
        true
    }

    fn x11_only(via: Via) -> Result<(), String> {
        match via {
            Via::X11 => Ok(()),
            other => Err(format!("an X11 peer has no {other:?}; use via: x11")),
        }
    }

    fn atom(&self, name: &str) -> Result<Atom, String> {
        Ok(self
            .conn
            .intern_atom(false, name.as_bytes())
            .map_err(err)?
            .reply()
            .map_err(err)?
            .atom)
    }

    fn atom_name(&self, atom: Atom) -> String {
        self.conn
            .get_atom_name(atom)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.name).into_owned())
            .unwrap_or_default()
    }

    fn selection_atom(&self, selection: Selection) -> Atom {
        match selection {
            Selection::Clipboard => self.atoms.CLIPBOARD,
            Selection::Primary => self.atoms.PRIMARY,
        }
    }

    fn selection_of(&self, atom: Atom) -> Option<Selection> {
        if atom == self.atoms.CLIPBOARD {
            Some(Selection::Clipboard)
        } else if atom == self.atoms.PRIMARY {
            Some(Selection::Primary)
        } else {
            None
        }
    }

    fn action_atom(&self, action: Action) -> Atom {
        match action {
            Action::Copy => self.atoms.XdndActionCopy,
            Action::Move => self.atoms.XdndActionMove,
            Action::Ask => self.atoms.XdndActionAsk,
        }
    }

    fn action_of(&self, atom: Atom) -> Option<Action> {
        [Action::Copy, Action::Move, Action::Ask]
            .into_iter()
            .find(|a| self.action_atom(*a) == atom)
    }

    fn map(&mut self, id: u64, title: &str, width: u32, height: u32) -> Result<(), String> {
        let c = &self.conn;
        let class = b"strata-test-peer\0strata-test-peer\0";
        c.change_property8(
            PropMode::REPLACE,
            self.win,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )
        .map_err(err)?;
        c.change_property8(
            PropMode::REPLACE,
            self.win,
            self.atoms._NET_WM_NAME,
            self.atoms.UTF8_STRING,
            title.as_bytes(),
        )
        .map_err(err)?;
        c.change_property8(
            PropMode::REPLACE,
            self.win,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            class,
        )
        .map_err(err)?;
        c.change_property32(
            PropMode::REPLACE,
            self.win,
            self.atoms._NET_WM_PID,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )
        .map_err(err)?;
        c.configure_window(
            self.win,
            &ConfigureWindowAux::new().width(width).height(height),
        )
        .map_err(err)?;
        c.map_window(self.win).map_err(err)?;
        self.map_reply = Some(id);
        self.map_retry = Some((Instant::now() + MAP_RETRY, 1));
        Ok(())
    }

    /// Whether the window manager lists the peer's window as mapped.
    fn listed(&self) -> bool {
        self.conn
            .get_property(
                false,
                self.root,
                self.atoms._NET_CLIENT_LIST,
                AtomEnum::WINDOW,
                0,
                4096,
            )
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|mut v| v.any(|w| w == self.win)))
            .unwrap_or(false)
    }

    /// Replies to `map` once the window is listed.
    fn check_mapped(&mut self) {
        if self.map_reply.is_some() && self.listed() {
            self.map_retry = None;
            if let Some(id) = self.map_reply.take() {
                out::value(
                    id,
                    Mapped {
                        xid: Some(self.win),
                    },
                );
            }
        }
    }

    /// The window is still not listed: unmap it and map it again (see the module docs).
    fn map_again(&mut self) {
        self.check_mapped();
        let Some((_, attempts)) = self.map_retry else {
            return;
        };
        if attempts >= MAP_ATTEMPTS {
            self.map_retry = None;
            if let Some(id) = self.map_reply.take() {
                out::reply(
                    id,
                    Err(format!(
                        "the window manager never listed the window ({attempts} maps)"
                    )),
                );
            }
            return;
        }
        eprintln!("strata-test-peer: window not mapped by the compositor; mapping it again");
        let _ = self.conn.unmap_window(self.win);
        let _ = self.conn.map_window(self.win);
        self.map_retry = Some((Instant::now() + MAP_RETRY, attempts + 1));
    }

    fn selection_set(
        &mut self,
        id: u64,
        selection: Selection,
        via: Via,
        offers: Vec<Offer>,
    ) -> Result<(), String> {
        Self::x11_only(via)?;
        let atom = self.selection_atom(selection);
        self.owned.insert(atom, offers);
        self.conn
            .set_selection_owner(self.win, atom, x11rb::CURRENT_TIME)
            .map_err(err)?;
        let owner = self
            .conn
            .get_selection_owner(atom)
            .map_err(err)?
            .reply()
            .map_err(err)?
            .owner;
        if owner != self.win {
            self.owned.remove(&atom);
            return Err("the X server did not give the peer the selection".into());
        }
        out::ok(id);
        Ok(())
    }

    /// Asks the selection's owner for `target`, into a property of the peer's own.
    fn convert(&mut self, selection: Atom, target: Atom, why: Conversion) -> Result<(), String> {
        self.convert_at(selection, target, why, x11rb::CURRENT_TIME)
    }

    fn convert_at(
        &mut self,
        selection: Atom,
        target: Atom,
        why: Conversion,
        time: u32,
    ) -> Result<(), String> {
        self.next_property += 1;
        let property = self.atom(&format!("STRATA_PEER_{}", self.next_property))?;
        self.pending.insert(property, (selection, target, why));
        self.conn
            .convert_selection(self.win, selection, target, property, time)
            .map_err(err)?;
        Ok(())
    }

    fn drag_start(
        &mut self,
        id: u64,
        offers: Vec<Offer>,
        actions: Vec<Action>,
    ) -> Result<(), String> {
        if !self.pressed {
            return Err("a drag needs a press on the peer's window that is still held".into());
        }
        let mimes: Vec<String> = offers.iter().map(|o| o.mime.clone()).collect();
        let types = mimes
            .iter()
            .map(|m| self.atom(m))
            .collect::<Result<Vec<_>, _>>()?;
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.win,
                self.atoms.XdndTypeList,
                AtomEnum::ATOM,
                &types,
            )
            .map_err(err)?;
        self.owned.insert(self.atoms.XdndSelection, offers);
        self.conn
            .set_selection_owner(self.win, self.atoms.XdndSelection, x11rb::CURRENT_TIME)
            .map_err(err)?;
        self.drag = Some(DragOut {
            mimes,
            action: actions.first().copied().unwrap_or(Action::Copy),
            target: None,
            accepted: false,
        });
        out::ok(id);
        Ok(())
    }

    fn message(&self, to: Window, ty: Atom, data: [u32; 5]) {
        let event = ClientMessageEvent::new(32, to, ty, data);
        let _ = self.conn.send_event(false, to, EventMask::NO_EVENT, event);
    }

    // ---- events ----

    fn event(&mut self, e: XEvent) {
        match e {
            XEvent::MapNotify(m) if m.window == self.win => self.check_mapped(),
            XEvent::FocusIn(f) if f.event == self.win && f.mode == NotifyMode::NORMAL => {
                out::event(Event::Focus { focused: true });
            }
            XEvent::FocusOut(f) if f.event == self.win && f.mode == NotifyMode::NORMAL => {
                out::event(Event::Focus { focused: false });
            }
            XEvent::ButtonPress(b) if b.event == self.win => {
                self.pressed = true;
                out::event(Event::PointerButton { pressed: true });
            }
            XEvent::ButtonRelease(b) if b.event == self.win => {
                self.pressed = false;
                if self.drag.is_some() {
                    self.drag_release();
                }
                out::event(Event::PointerButton { pressed: false });
            }
            XEvent::MotionNotify(m) if self.drag.is_some() => {
                self.drag_motion(m.root_x, m.root_y);
            }
            XEvent::SelectionRequest(r) => self.serve(r),
            XEvent::SelectionClear(c) => {
                self.owned.remove(&c.selection);
                if let Some(selection) = self.selection_of(c.selection) {
                    out::event(Event::SelectionCancelled {
                        selection,
                        via: Via::X11,
                    });
                }
            }
            XEvent::XfixesSelectionNotify(n) => {
                if let Some(selection) = self.selection_of(n.selection) {
                    out::event(Event::SelectionChanged {
                        selection,
                        via: Via::X11,
                        mimes: None,
                    });
                }
            }
            XEvent::SelectionNotify(n) => self.converted(n),
            XEvent::PropertyNotify(p) if p.window == self.root => {
                if p.atom == self.atoms._NET_CLIENT_LIST {
                    self.check_mapped();
                }
            }
            XEvent::PropertyNotify(p) => self.property(p.window, p.atom, p.state),
            XEvent::ClientMessage(m) => self.client_message(&m),
            _ => {}
        }
    }

    /// A reader asks for one of the peer's selections.
    fn serve(&mut self, r: SelectionRequestEvent) {
        let property = if r.property == x11rb::NONE {
            r.target
        } else {
            r.property
        };
        let ok = self.answer(&r, property).unwrap_or_else(|e| {
            out::error(e);
            false
        });
        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: r.time,
            requestor: r.requestor,
            selection: r.selection,
            target: r.target,
            property: if ok { property } else { x11rb::NONE },
        };
        let _ = self
            .conn
            .send_event(false, r.requestor, EventMask::NO_EVENT, notify);
    }

    fn answer(&mut self, r: &SelectionRequestEvent, property: Atom) -> Result<bool, String> {
        let Some(offers) = self.owned.get(&r.selection) else {
            return Ok(false);
        };
        let a = &self.atoms;
        if r.target == a.TARGETS {
            let mut list = vec![a.TARGETS, a.TIMESTAMP];
            for o in offers.clone() {
                list.push(self.atom(&o.mime)?);
            }
            self.conn
                .change_property32(
                    PropMode::REPLACE,
                    r.requestor,
                    property,
                    AtomEnum::ATOM,
                    &list,
                )
                .map_err(err)?;
            return Ok(true);
        }
        if r.target == a.TIMESTAMP {
            self.conn
                .change_property32(
                    PropMode::REPLACE,
                    r.requestor,
                    property,
                    AtomEnum::INTEGER,
                    &[x11rb::CURRENT_TIME],
                )
                .map_err(err)?;
            return Ok(true);
        }
        if r.target == a.MULTIPLE {
            return Ok(false);
        }
        let mime = self.atom_name(r.target);
        let Some(offer) = offers.iter().find(|o| o.mime == mime) else {
            return Ok(false);
        };
        let data = offer
            .data
            .bytes()
            .map_err(|e| format!("serving {mime}: {e}"))?;
        out::event(Event::SourceSend { mime });
        if data.len() <= INCR_CHUNK {
            self.conn
                .change_property8(PropMode::REPLACE, r.requestor, property, r.target, &data)
                .map_err(err)?;
            return Ok(true);
        }
        // INCR: announce the size, then send a chunk each time the reader deletes the last.
        // The reader's window must report property changes to us; ours already does.
        if r.requestor != self.win {
            self.conn
                .change_window_attributes(
                    r.requestor,
                    &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
                )
                .map_err(err)?;
        }
        self.conn
            .change_property32(
                PropMode::REPLACE,
                r.requestor,
                property,
                self.atoms.INCR,
                &[data.len() as u32],
            )
            .map_err(err)?;
        self.incr_out.push(IncrOut {
            requestor: r.requestor,
            property,
            target: r.target,
            data,
            offset: 0,
        });
        Ok(true)
    }

    fn property(&mut self, window: Window, atom: Atom, state: Property) {
        // INCR out: the reader deleted the last chunk; send the next, then an empty one.
        if state == Property::DELETE
            && let Some(i) = self
                .incr_out
                .iter()
                .position(|o| o.requestor == window && o.property == atom)
        {
            let o = &mut self.incr_out[i];
            let end = (o.offset + INCR_CHUNK).min(o.data.len());
            let chunk = o.data[o.offset..end].to_vec();
            o.offset = end;
            let _ = self.conn.change_property8(
                PropMode::REPLACE,
                o.requestor,
                o.property,
                o.target,
                &chunk,
            );
            if chunk.is_empty() {
                self.incr_out.remove(i);
            }
            return;
        }
        // INCR in: a chunk landed on the peer's window; an empty one ends the transfer.
        if window == self.win && state == Property::NEW_VALUE && self.incr_in.contains_key(&atom) {
            let Some(reply) = self
                .conn
                .get_property(true, self.win, atom, AtomEnum::ANY, 0, u32::MAX / 4)
                .ok()
                .and_then(|c| c.reply().ok())
            else {
                return;
            };
            if reply.value.is_empty() {
                if let Some((why, data)) = self.incr_in.remove(&atom) {
                    self.finish(why, Ok(data));
                }
            } else if let Some((_, data)) = self.incr_in.get_mut(&atom) {
                data.extend_from_slice(&reply.value);
            }
        }
    }

    /// A conversion the peer asked for has landed, or was refused.
    fn converted(&mut self, n: SelectionNotifyEvent) {
        if n.property == x11rb::NONE {
            // Refused, or nobody owns the selection: the property is gone, so match the
            // selection and target asked for.
            let key = self
                .pending
                .iter()
                .find(|(_, (s, t, _))| *s == n.selection && *t == n.target)
                .map(|(k, _)| *k);
            if let Some((_, _, why)) = key.and_then(|k| self.pending.remove(&k)) {
                self.finish(
                    why,
                    Err("the owner refused the conversion, or there is none".into()),
                );
            }
            return;
        }
        let Some((_, _, why)) = self.pending.remove(&n.property) else {
            return;
        };
        let reply = match self
            .conn
            .get_property(true, self.win, n.property, AtomEnum::ANY, 0, u32::MAX / 4)
            .map_err(err)
            .and_then(|c| c.reply().map_err(err))
        {
            Ok(r) => r,
            Err(e) => return self.finish(why, Err(e)),
        };
        if reply.type_ == self.atoms.INCR {
            self.incr_in.insert(n.property, (why, Vec::new()));
            return;
        }
        if matches!(why, Conversion::Targets(_)) {
            let atoms: Vec<Atom> = reply.value32().map(Iterator::collect).unwrap_or_default();
            let mut bytes = Vec::with_capacity(atoms.len() * 4);
            atoms
                .iter()
                .for_each(|a| bytes.extend_from_slice(&a.to_ne_bytes()));
            return self.finish(why, Ok(bytes));
        }
        self.finish(why, Ok(reply.value));
    }

    fn finish(&mut self, why: Conversion, data: Result<Vec<u8>, String>) {
        match why {
            Conversion::Read(id) => out::reply(
                id,
                data.and_then(|d| serde_json::to_value(out::received(&d)).map_err(err)),
            ),
            Conversion::Targets(id) => {
                let mimes = data.ok().map(|bytes| {
                    bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|c| u32::from_ne_bytes(*c))
                        .filter(|a| {
                            ![
                                self.atoms.TARGETS,
                                self.atoms.TIMESTAMP,
                                self.atoms.MULTIPLE,
                            ]
                            .contains(a)
                        })
                        .map(|a| self.atom_name(a))
                        .filter(|n| !n.is_empty())
                        .collect()
                });
                out::value(id, Targets { mimes });
            }
            Conversion::Drop {
                source,
                mime,
                action,
            } => {
                let accepted = data.is_ok();
                let action_atom = action
                    .filter(|_| accepted)
                    .map_or(x11rb::NONE, |a| self.action_atom(a));
                self.message(
                    source,
                    self.atoms.XdndFinished,
                    [self.win, u32::from(accepted), action_atom, 0, 0],
                );
                match data {
                    Ok(d) => out::event(Event::Dropped {
                        mime,
                        action,
                        data: out::received(&d),
                    }),
                    Err(e) => out::error(format!("drop of {mime}: {e}")),
                }
            }
        }
    }

    // ---- XDND ----

    fn client_message(&mut self, m: &ClientMessageEvent) {
        let a = &self.atoms;
        let l = m.data.as_data32();
        let ty = m.type_;
        if ty == a.XdndEnter {
            self.drop_enter(l);
        } else if ty == a.XdndPosition {
            self.drop_position(l);
        } else if ty == a.XdndLeave {
            self.drop = None;
            out::event(Event::DropLeave);
        } else if ty == a.XdndDrop {
            self.drop_drop(l);
        } else if ty == a.XdndStatus {
            let action = self.action_of(l[4]);
            if let Some(d) = self.drag.as_mut() {
                d.accepted = l[1] & 1 != 0;
                let action = action.filter(|_| d.accepted);
                out::event(Event::DragAction { action });
            }
        } else if ty == a.XdndFinished {
            let action = self.action_of(l[2]);
            self.drag = None;
            self.owned.remove(&self.atoms.XdndSelection);
            out::event(Event::DragFinished { action });
        }
    }

    fn drop_enter(&mut self, l: [u32; 5]) {
        let source = l[0];
        let mut types: Vec<Atom> = l[2..5].iter().copied().filter(|t| *t != 0).collect();
        if l[1] & 1 != 0
            && let Some(list) = self
                .conn
                .get_property(
                    false,
                    source,
                    self.atoms.XdndTypeList,
                    AtomEnum::ATOM,
                    0,
                    1024,
                )
                .ok()
                .and_then(|c| c.reply().ok())
        {
            types = list.value32().map(Iterator::collect).unwrap_or_default();
        }
        let mimes: Vec<String> = types.into_iter().map(|t| self.atom_name(t)).collect();
        let (mime, action) = match &self.drop_policy {
            Some((want, action)) => (
                want.iter().find(|m| mimes.contains(m)).cloned(),
                Some(*action),
            ),
            None => (None, None),
        };
        self.drop = Some(DropIn {
            source,
            action: action.filter(|_| mime.is_some()),
            mime,
        });
        out::event(Event::DropEnter { mimes });
    }

    fn drop_position(&mut self, l: [u32; 5]) {
        let Some(d) = &self.drop else { return };
        let (rx, ry) = ((l[2] >> 16) as i16, (l[2] & 0xffff) as i16);
        let local = self
            .conn
            .translate_coordinates(self.root, self.win, rx, ry)
            .ok()
            .and_then(|c| c.reply().ok())
            .map_or((0, 0), |r| (r.dst_x, r.dst_y));
        let accept = d.mime.is_some();
        let action = d.action.map_or(x11rb::NONE, |a| self.action_atom(a));
        // XdndStatus: bit 0 accepts, bit 1 asks for a position on every motion.
        self.message(
            d.source,
            self.atoms.XdndStatus,
            [self.win, u32::from(accept) | 2, 0, 0, action],
        );
        out::event(Event::DropMotion {
            x: f64::from(local.0),
            y: f64::from(local.1),
        });
    }

    fn drop_drop(&mut self, l: [u32; 5]) {
        let Some(d) = self.drop.take() else { return };
        let Some(mime) = d.mime else {
            self.message(d.source, self.atoms.XdndFinished, [self.win, 0, 0, 0, 0]);
            return;
        };
        let target = match self.atom(&mime) {
            Ok(t) => t,
            Err(e) => return out::error(e),
        };
        let why = Conversion::Drop {
            source: d.source,
            mime,
            action: d.action,
        };
        if let Err(e) = self.convert_at(self.atoms.XdndSelection, target, why, l[2]) {
            out::error(e);
        }
    }

    /// The XdndAware top-level window under a root point, other than the peer's own.
    fn aware_at(&self, x: i16, y: i16) -> Option<Window> {
        let mut w = self.root;
        for _ in 0..8 {
            let r = self
                .conn
                .translate_coordinates(self.root, w, x, y)
                .ok()?
                .reply()
                .ok()?;
            if r.child == x11rb::NONE || r.child == self.win {
                return None;
            }
            w = r.child;
            let aware = self
                .conn
                .get_property(false, w, self.atoms.XdndAware, AtomEnum::ATOM, 0, 1)
                .ok()?
                .reply()
                .ok()?;
            if aware.value_len > 0 {
                return Some(w);
            }
        }
        None
    }

    fn drag_motion(&mut self, x: i16, y: i16) {
        let target = self.aware_at(x, y);
        let Some(d) = self.drag.as_ref() else { return };
        let (old, mimes, action) = (d.target, d.mimes.clone(), d.action);
        if old != target {
            if let Some(old) = old {
                self.message(old, self.atoms.XdndLeave, [self.win, 0, 0, 0, 0]);
            }
            if let Some(t) = target {
                let more = u32::from(mimes.len() > 3);
                let mut l = [self.win, (XDND_VERSION << 24) | more, 0, 0, 0];
                for (i, m) in mimes.iter().take(3).enumerate() {
                    l[2 + i] = self.atom(m).unwrap_or(x11rb::NONE);
                }
                self.message(t, self.atoms.XdndEnter, l);
            }
            if let Some(d) = self.drag.as_mut() {
                d.target = target;
                d.accepted = false;
            }
        }
        if let Some(t) = target {
            let position = ((x as u16 as u32) << 16) | (y as u16 as u32);
            self.message(
                t,
                self.atoms.XdndPosition,
                [
                    self.win,
                    0,
                    position,
                    x11rb::CURRENT_TIME,
                    self.action_atom(action),
                ],
            );
        }
    }

    fn drag_release(&mut self) {
        let Some(d) = self.drag.as_ref() else { return };
        match d.target {
            Some(t) if d.accepted => {
                self.message(
                    t,
                    self.atoms.XdndDrop,
                    [self.win, 0, x11rb::CURRENT_TIME, 0, 0],
                );
                out::event(Event::DragDropped);
            }
            target => {
                if let Some(t) = target {
                    self.message(t, self.atoms.XdndLeave, [self.win, 0, 0, 0, 0]);
                }
                self.drag = None;
                self.owned.remove(&self.atoms.XdndSelection);
                out::event(Event::DragCancelled);
            }
        }
    }
}
