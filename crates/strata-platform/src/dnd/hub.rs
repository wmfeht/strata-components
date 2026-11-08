//! The process's drag and drop, on the UI thread: P0's substrate, the app's windows, the
//! drag this process started, and the routing of what the platform reports to the window it
//! concerns.
//!
//! Everything here runs on the UI thread, inside a window's `AppHandle::update_if` (a
//! [`Job`]). Other threads (P0's reader, the X11 actor, read workers) only post jobs.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use rune::raw_window_handle::RawWindowHandle;
use rune::{DragOutcome, Mods, WindowHandle};

use crate::transfer::{self, Subscription, Transfer, Turn, Wants};

use super::api::{Actions, DragAction, DragItem, Error, Status};
use super::icon::{self, DragImage};
use super::local;
use super::negotiate;
use super::session::Release;
use super::{wayland, x11};

/// Work for a window, posted from any thread and run on the UI thread in that window's
/// update.
pub(crate) enum Job {
    /// Route what the platform reported, then handle this window's share.
    Pump,
    /// The reads of drop `id` are done.
    Dropped(u64),
    /// A type read while the drag hovers.
    Peeked {
        serial: u64,
        mime: String,
        bytes: Vec<u8>,
    },
    /// The picture of drag `id`, rendered.
    Icon { id: u64, image: Option<DragImage> },
    /// P0's turn: dispatch its queue (see [`transfer::Turn`]).
    Run(Box<dyn FnOnce() + Send>),
}

pub(crate) type Poster = Arc<dyn Fn(Job) + Send + Sync>;

/// A drag over one of the app's windows, as the platform reports it.
pub(crate) enum InEvent {
    Enter {
        port: Rc<dyn InPort>,
        mimes: Vec<String>,
        actions: Actions,
        x: f64,
        y: f64,
        mods: Option<Mods>,
    },
    /// The source's actions, known after the enter (Wayland).
    Actions(Actions),
    Motion {
        x: f64,
        y: f64,
        mods: Option<Mods>,
    },
    Leave,
    Drop,
    /// The platform connection failed: the drag is gone.
    Failed,
}

/// What a window hears.
pub(crate) enum Routed {
    In(InEvent),
    /// A drag this window started ended.
    Ended {
        id: u64,
        outcome: DragOutcome,
    },
}

/// The platform side of one drag over a window.
pub(crate) trait InPort {
    /// What the target takes at the current point: a type and an action, or nothing.
    fn status(&self, accept: Option<&str>, action: Option<DragAction>);
    /// Reads a type, during the drag or after the drop. `done` runs once, on any thread.
    fn read(&self, mime: &str, done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>);
    /// The action the drop took, given what the target chose: on Wayland the compositor's.
    fn dropped_action(&self, chosen: DragAction) -> DragAction;
    /// Ends the drop with `action`, or refuses it.
    fn finish(&self, action: Option<DragAction>);
}

/// A window, to the hub: generic code behind a trait object.
pub(crate) trait Sink {
    fn queue(&self, r: Routed);
    fn post(&self, job: Job);
}

struct Win {
    key: String,
    handle: WindowHandle,
    sink: Rc<dyn Sink>,
    core: Rc<dyn Any>,
}

/// The drag this process started.
struct Out {
    id: u64,
    window: String,
    token: Option<String>,
    payload: Option<Rc<dyn Any>>,
    mods: Mods,
    x11: bool,
    wl: wayland::OutState,
}

pub(crate) struct Hub {
    transfer: Transfer,
    sub: Subscription,
    wins: Vec<Win>,
    /// Every window's poster: anything any thread hears wakes them all, and the first to
    /// run routes it.
    posters: Arc<Mutex<Vec<Poster>>>,
    /// The window a Wayland drag is over, and whether the drag is this process's own.
    wl_in: Option<(String, bool)>,
    out: Option<Out>,
    x11: Option<x11::Inbox>,
    next: u64,
}

type Diag = Rc<dyn Fn(&str)>;

thread_local! {
    static HUB: RefCell<Option<Hub>> = const { RefCell::new(None) };
    static DIAG: RefCell<Option<Diag>> = const { RefCell::new(None) };
}

/// Reports something that went wrong outside a call that could return it.
pub(crate) fn diagnostic(message: &str) {
    let f = DIAG.with(|d| d.borrow().clone());
    match f {
        Some(f) => f(message),
        None => eprintln!("strata-platform dnd: {message}"),
    }
}

pub(crate) fn set_diagnostics(f: Rc<dyn Fn(&str)>) {
    DIAG.with(|d| *d.borrow_mut() = Some(f));
}

fn with<R>(f: impl FnOnce(&mut Hub) -> R) -> Option<R> {
    HUB.with(|h| h.borrow_mut().as_mut().map(f))
}

fn wake_all(posters: &Arc<Mutex<Vec<Poster>>>) {
    let all = posters
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    for p in all {
        p(Job::Pump);
    }
}

/// Attaches `window` (its core `core`, reached through `sink`), binding the substrate on
/// the first call. On the UI thread, from `App::on_window`.
pub(crate) fn attach(
    window: &WindowHandle,
    sink: Rc<dyn Sink>,
    core: Rc<dyn Any>,
    poster: Poster,
) -> Result<Status, Error> {
    let fresh = HUB.with(|h| h.borrow().is_none());
    if fresh {
        let turn_poster = poster.clone();
        let turn: Turn = Arc::new(move |f| turn_poster(Job::Run(f)));
        // Default wants: P0 binds once per process, so a clipboard (P2) attached later
        // still finds data control.
        let (t, _) = Transfer::attach(window, turn, Wants::default())?;
        let posters: Arc<Mutex<Vec<Poster>>> = Arc::default();
        let wake = posters.clone();
        let sub = t.subscribe(move || wake_all(&wake));
        let x11 = match t.x11() {
            Some(x) => {
                let wake = posters.clone();
                let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake_all(&wake));
                Some(x11::install(&t, x, notify)?)
            }
            None => None,
        };
        if let Some(conn) = t.shared_conn() {
            let wake = posters.clone();
            wayland::set_notify(conn, Arc::new(move || wake_all(&wake)));
        }
        // P1's objects go before P0's teardown, on the UI thread.
        t.before_shutdown(Box::new(|_| teardown()));
        HUB.with(|h| {
            *h.borrow_mut() = Some(Hub {
                transfer: t,
                sub,
                wins: Vec::new(),
                posters,
                wl_in: None,
                out: None,
                x11,
                next: 1,
            });
        });
    }
    let key = window.key().to_owned();
    let xid = window
        .with_raw_handle(|h| match h.as_raw() {
            RawWindowHandle::Xlib(w) => Some(w.window as u32),
            RawWindowHandle::Xcb(w) => Some(w.window.get()),
            _ => None,
        })
        .ok()
        .flatten();
    with(|hub| {
        // Rune recreates a window's native side (a Wayland hide and show): keep one entry.
        hub.wins.retain(|w| w.key != key);
        hub.wins.push(Win {
            key: key.clone(),
            handle: window.clone(),
            sink,
            core,
        });
        let mut posters = hub.posters.lock().unwrap_or_else(PoisonError::into_inner);
        posters.push(poster);
        drop(posters);
        if !fresh {
            // P0 tracks the windows a drag can enter.
            let _ = Transfer::attach(window, Arc::new(|_| {}), Wants::default());
        }
        if let (Some(x), Some(xid)) = (hub.transfer.x11(), xid) {
            x11::add_window(x, &key, xid);
        }
        status(&hub.transfer)
    })
    .ok_or(Error::NotAttached)
}

fn status(t: &Transfer) -> Status {
    let s = t.status();
    let wl = t.shared_conn().is_some();
    let x11 = s.backend == "x11";
    Status {
        backend: s.backend,
        drag_in: wl || x11,
        drag_out: wl || x11,
        unsupported: (!wl && !x11).then(|| {
            s.unsupported.as_ref().map_or(
                "winit's display isn't reachable".to_owned(),
                ToString::to_string,
            )
        }),
    }
}

/// The core of window `key`, if it is of state type `S`.
pub(crate) fn core<C: 'static>(key: &str) -> Option<Rc<C>> {
    with(|hub| {
        hub.wins
            .iter()
            .find(|w| w.key == key)
            .and_then(|w| w.core.clone().downcast::<C>().ok())
    })
    .flatten()
}

pub(crate) fn current_status() -> Option<Status> {
    with(|hub| status(&hub.transfer))
}

pub(crate) fn next_id() -> u64 {
    with(|hub| {
        hub.next += 1;
        hub.next
    })
    .unwrap_or(0)
}

/// Pixels per point of window `key`, for X11's pixel positions.
fn scale_of(hub: &Hub, key: &str) -> f64 {
    hub.wins
        .iter()
        .find(|w| w.key == key)
        .and_then(|w| w.handle.placement())
        .map_or(1.0, |p| p.scale)
        .max(0.01)
}

/// Routes everything the platform reported since the last call to the windows it concerns,
/// and wakes the ones that got something.
pub(crate) fn route() {
    let woken = with(|hub| {
        let mut woken: Vec<Rc<dyn Sink>> = Vec::new();
        let to = |hub: &Hub, key: &str, r: Routed, woken: &mut Vec<Rc<dyn Sink>>| {
            if let Some(w) = hub.wins.iter().find(|w| w.key == key) {
                w.sink.queue(r);
                if !woken.iter().any(|s| Rc::ptr_eq(s, &w.sink)) {
                    woken.push(w.sink.clone());
                }
            }
        };
        route_substrate(hub, &mut |hub, key, r| to(hub, key, r, &mut woken));
        route_wayland_source(hub, &mut |hub, key, r| to(hub, key, r, &mut woken));
        route_x11(hub, &mut |hub, key, r| to(hub, key, r, &mut woken));
        woken
    })
    .unwrap_or_default();
    for sink in woken {
        sink.post(Job::Pump);
    }
}

type To<'a> = dyn FnMut(&Hub, &str, Routed) + 'a;

/// Whether the drag over a window is the one this process started: its token is listed.
fn local_of(hub: &Hub, mimes: &[String]) -> bool {
    let token = local::token_in(mimes);
    token.is_some() && hub.out.as_ref().and_then(|o| o.token.as_deref()) == token
}

/// The modifiers of a Wayland drag over a window: known only for this process's own drag,
/// as they were at its start (the compositor's drag grab passes no more to anyone, on
/// wlroots). A drag from another client has none: its action is the compositor's.
fn local_mods(hub: &Hub, local: bool) -> Option<Mods> {
    hub.out.as_ref().filter(|o| local && !o.x11).map(|o| o.mods)
}

fn route_substrate(hub: &mut Hub, to: &mut To<'_>) {
    for event in hub.sub.drain() {
        match event {
            transfer::Event::DragEnter {
                window,
                offer,
                x,
                y,
            } => {
                let mimes = offer.mimes().to_vec();
                let local = local_of(hub, &mimes);
                let mods = local_mods(hub, local);
                hub.wl_in = Some((window.clone(), local));
                let port: Rc<dyn InPort> = Rc::new(wayland::WlIn::new(offer));
                to(
                    hub,
                    &window,
                    Routed::In(InEvent::Enter {
                        port,
                        mimes,
                        actions: Actions::NONE,
                        x,
                        y,
                        mods,
                    }),
                );
            }
            transfer::Event::DragActions { source, .. } if !source.is_empty() => {
                if let Some((w, _)) = hub.wl_in.clone() {
                    let actions = source.iter().map(|a| wayland::action_of(*a)).collect();
                    to(hub, &w, Routed::In(InEvent::Actions(actions)));
                }
            }
            transfer::Event::DragMotion { x, y } => {
                if let Some((w, local)) = hub.wl_in.clone() {
                    let mods = local_mods(hub, local);
                    to(hub, &w, Routed::In(InEvent::Motion { x, y, mods }));
                }
            }
            transfer::Event::DragLeave => {
                if let Some((w, _)) = hub.wl_in.take() {
                    to(hub, &w, Routed::In(InEvent::Leave));
                }
            }
            transfer::Event::Drop => {
                if let Some((w, _)) = hub.wl_in.clone() {
                    to(hub, &w, Routed::In(InEvent::Drop));
                }
            }
            transfer::Event::Failed { message } => {
                diagnostic(&format!("the platform connection failed: {message}"));
                let keys: Vec<String> = hub.wins.iter().map(|w| w.key.clone()).collect();
                for k in keys {
                    to(hub, &k, Routed::In(InEvent::Failed));
                }
                hub.wl_in = None;
                if let Some(out) = hub.out.take() {
                    to(
                        hub,
                        &out.window,
                        Routed::Ended {
                            id: out.id,
                            outcome: DragOutcome::Cancelled,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

fn route_wayland_source(hub: &mut Hub, to: &mut To<'_>) {
    let Some(conn) = hub.transfer.shared_conn().cloned() else {
        return;
    };
    for (id, ev) in wayland::drain(&conn) {
        let Some(out) = hub.out.as_mut().filter(|o| o.id == id) else {
            continue;
        };
        if let Some(outcome) = out.wl.on(ev) {
            let out = hub.out.take().expect("matched above");
            wayland::end(&conn, id);
            to(hub, &out.window, Routed::Ended { id, outcome });
        }
    }
}

fn route_x11(hub: &mut Hub, to: &mut To<'_>) {
    let Some(inbox) = hub.x11.clone() else {
        return;
    };
    for note in inbox.drain() {
        match note {
            x11::Note::Enter {
                key,
                port,
                mimes,
                actions,
                x,
                y,
                mods,
            } => {
                let s = scale_of(hub, &key);
                let port: Rc<dyn InPort> = Rc::new(port);
                to(
                    hub,
                    &key,
                    Routed::In(InEvent::Enter {
                        port,
                        mimes,
                        actions,
                        x: x / s,
                        y: y / s,
                        mods: Some(mods),
                    }),
                );
            }
            x11::Note::Motion { key, x, y, mods } => {
                let s = scale_of(hub, &key);
                to(
                    hub,
                    &key,
                    Routed::In(InEvent::Motion {
                        x: x / s,
                        y: y / s,
                        mods: Some(mods),
                    }),
                );
            }
            x11::Note::Leave { key } => to(hub, &key, Routed::In(InEvent::Leave)),
            x11::Note::Drop { key } => to(hub, &key, Routed::In(InEvent::Drop)),
            x11::Note::Ended { id, outcome } => {
                if let Some(out) = hub.out.take_if(|o| o.id == id) {
                    to(hub, &out.window, Routed::Ended { id, outcome });
                }
            }
        }
    }
}

/// The payload of the drag this process started, with its token, for a drop in this
/// process that lists that token.
pub(crate) fn local_payload(mimes: &[String]) -> Option<(String, Rc<dyn Any>)> {
    with(|hub| {
        if !local_of(hub, mimes) {
            return None;
        }
        let out = hub.out.as_ref()?;
        Some((out.token.clone()?, out.payload.clone()?))
    })
    .flatten()
}

/// Starts a drag from window `key`. The release tells the source's guard how Rune's
/// capture of the press ends.
pub(crate) fn start_drag(key: &str, item: DragItem, mods: Mods) -> Result<(u64, Release), Error> {
    if item.is_empty() {
        return Err(Error::Refused("a drag with nothing to carry"));
    }
    #[cfg(test)]
    if let Some(release) = tests::FAKE_START.get() {
        return Ok((tests::next_id(), release));
    }
    // One drag at a time: an earlier one this process lost track of ends now.
    cancel_drag();
    let id = next_id();
    let actions = negotiate::source_actions(item.actions, mods);
    let token = item.local.is_some().then(local::new_token);
    let spec = item.spec(token.as_deref());
    let started = with(|hub| -> Result<(bool, Release), Error> {
        let window = hub
            .wins
            .iter()
            .find(|w| w.key == key)
            .map(|w| w.handle.clone())
            .ok_or(Error::NotAttached)?;
        if let Some(x) = hub.transfer.x11() {
            x11::start(x, id, spec, actions)?;
            return Ok((true, Release::RealUp));
        }
        let conn = hub
            .transfer
            .shared_conn()
            .cloned()
            .ok_or_else(|| Error::Unsupported("no route to winit's display".into()))?;
        // The press that started this drag must be in before its serial is read (S2).
        hub.transfer.dispatch()?;
        wayland::start(&conn, &window, id, &spec, actions, item.icon.is_some())?;
        Ok((false, Release::Escape))
    })
    .ok_or(Error::NotAttached)??;
    let (x11, release) = started;
    with(|hub| {
        hub.out = Some(Out {
            id,
            window: key.to_owned(),
            token,
            payload: item.local.clone(),
            mods,
            x11,
            wl: wayland::OutState::default(),
        });
    });
    if let Some(icon) = item.icon.clone() {
        // The picture lands on the UI thread, which hands it to the platform: the Wayland
        // icon surface lives there, and a failure can be reported there.
        if let Some(posters) = with(|hub| hub.posters.clone()) {
            let done = Box::new(move |image| {
                let first = posters
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .first()
                    .cloned();
                if let Some(p) = first {
                    p(Job::Icon { id, image });
                }
            });
            if let Err(e) = icon::produce_later(icon, done) {
                diagnostic(&format!("the drag icon: {e}"));
            }
        }
    }
    Ok((id, release))
}

/// The picture of drag `id` is rendered (or failed to be): give it to the platform.
pub(crate) fn icon_ready(id: u64, image: Option<DragImage>) {
    let Some(image) = image else {
        return diagnostic("the drag icon couldn't be drawn");
    };
    with(|hub| {
        if !hub.out.as_ref().is_some_and(|o| o.id == id) {
            return;
        }
        if hub.transfer.x11().is_some() {
            x11::Handle::new(&hub.transfer).icon(id, Some(image));
        } else if let Some(conn) = hub.transfer.shared_conn() {
            wayland::set_icon(conn, id, &image);
        }
    });
}

/// Cancels the drag this process started, if any: its window hears `Cancelled`.
pub(crate) fn cancel_drag() {
    let ended = with(|hub| {
        let out = hub.out.take()?;
        if out.x11 {
            if let Some(x) = hub.transfer.x11() {
                x11::cancel(x, out.id);
            }
        } else if let Some(conn) = hub.transfer.shared_conn() {
            // Destroying the source ends the drag; its own events stop with it.
            wayland::end(conn, out.id);
        }
        let sink = hub
            .wins
            .iter()
            .find(|w| w.key == out.window)
            .map(|w| w.sink.clone())?;
        Some((sink, out.id))
    })
    .flatten();
    if let Some((sink, id)) = ended {
        sink.queue(Routed::Ended {
            id,
            outcome: DragOutcome::Cancelled,
        });
        sink.post(Job::Pump);
    }
}

/// Before P0's teardown, on the UI thread: the drag this process started ends, and nothing
/// of P1's is left on winit's display. The X11 hook undoes its window properties as the
/// actor quits.
fn teardown() {
    icon::stop();
    let hub = HUB.with(|h| h.borrow_mut().take());
    if let Some(hub) = hub
        && let (Some(out), Some(conn)) = (&hub.out, hub.transfer.shared_conn())
    {
        wayland::end(conn, out.id);
    }
}

pub(crate) fn shutdown() -> Result<bool, Error> {
    Ok(Transfer::shutdown()?)
}

/// A start that reaches no platform, for the headless tests of the source's guard.
#[cfg(test)]
pub(crate) mod tests {
    use std::cell::Cell;

    use super::Release;

    thread_local! {
        pub(crate) static FAKE_START: Cell<Option<Release>> = const { Cell::new(None) };
        static NEXT: Cell<u64> = const { Cell::new(0) };
    }

    pub(super) fn next_id() -> u64 {
        NEXT.with(|n| {
            n.set(n.get() + 1);
            n.get()
        })
    }
}
