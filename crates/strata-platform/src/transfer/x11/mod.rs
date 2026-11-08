//! X11: selections on a connection of our own, from one thread.
//!
//! Rune exposes no X connection (draft U9, not filed), and winit answers XDND for its window
//! itself, so everything here runs on a second x11rb connection with an unmapped
//! override-redirect window. One thread (the actor) does all of its I/O: a reply awaited on
//! another thread would read events off the socket that the actor's poll then never sees.
//! Calls from other threads become [`Cmd`]s on a channel, with a byte on a wake pipe.
//!
//! It owns `CLIPBOARD` and `PRIMARY` with `TARGETS`, `TIMESTAMP` and any MIME types, serves
//! large data with INCR, reads others' selections (INCR too), and hears every ownership change
//! through XFixes, so a lost selection shows at once.

mod selection;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xfixes::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, ConnectionExt as _, CreateWindowAux, EventMask, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

use super::data::{ReadLimits, SourceSpec};
use super::error::{Error, ReadError};
use super::event::{Done, Event, ReadOffer, Selection, SelectionOffer, SourceId, Subscribers};
use super::pipe;
use selection::{Incoming, Outgoing};

x11rb::atom_manager! {
    pub(crate) Atoms: AtomsCookie {
        CLIPBOARD,
        PRIMARY,
        TARGETS,
        TIMESTAMP,
        MULTIPLE,
        INCR,
        ATOM,
        INTEGER,
        TRANSFER_TIME: b"_STRATA_TRANSFER_TIME",
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

enum Cmd {
    Own {
        selection: Selection,
        id: SourceId,
        spec: SourceSpec,
    },
    Read {
        selection: Selection,
        owner: Window,
        mime: String,
        limits: ReadLimits,
        done: Done,
    },
    /// A lazy writer finished on a worker: answer the request that asked for it.
    Ready {
        request: selection::Request,
        bytes: Option<Arc<[u8]>>,
    },
    Quit,
}

/// What other threads may ask without a round trip to the actor.
#[derive(Default)]
struct Shared {
    offers: HashMap<Selection, SelectionOffer>,
    owned: HashSet<u64>,
}

/// The way into the actor, from any thread.
#[derive(Clone)]
struct Mailbox {
    tx: Sender<Cmd>,
    wake: Arc<File>,
}

impl Mailbox {
    /// Queues `cmd` and wakes the actor. Gives the command back if the actor is gone.
    fn post(&self, cmd: Cmd) -> Result<(), Cmd> {
        self.tx.send(cmd).map_err(|mpsc::SendError(cmd)| cmd)?;
        // Non-blocking: a full pipe already wakes the actor.
        let _ = (&*self.wake).write(&[1]);
        Ok(())
    }
}

/// The X11 side of the substrate: a handle to the actor thread.
pub(crate) struct X11 {
    mailbox: Mailbox,
    shared: Arc<Mutex<Shared>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl X11 {
    /// Connects to `$DISPLAY` and starts the actor.
    pub(crate) fn start(subs: Subscribers) -> Result<X11, Error> {
        let conn = |e: &dyn std::fmt::Display| Error::Connection(e.to_string());
        let (xconn, screen) = x11rb::connect(None).map_err(|e| conn(&e))?;
        let root = xconn.setup().roots[screen].root;
        let atoms = Atoms::new(&xconn)
            .map_err(|e| conn(&e))?
            .reply()
            .map_err(|e| conn(&e))?;
        let win = xconn.generate_id().map_err(|e| conn(&e))?;
        xconn
            .create_window(
                0,
                win,
                root,
                -10,
                -10,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new()
                    .override_redirect(1)
                    .event_mask(EventMask::PROPERTY_CHANGE),
            )
            .map_err(|e| conn(&e))?;
        xconn
            .xfixes_query_version(5, 0)
            .map_err(|e| conn(&e))?
            .reply()
            .map_err(|e| conn(&e))?;
        let mask = xfixes::SelectionEventMask::SET_SELECTION_OWNER
            | xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
            | xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE;
        for sel in [atoms.CLIPBOARD, atoms.PRIMARY] {
            xconn
                .xfixes_select_selection_input(win, sel, mask)
                .map_err(|e| conn(&e))?;
        }
        xconn.flush().map_err(|e| conn(&e))?;

        let (wake_r, wake_w) = pipe::pipe().map_err(|e| conn(&e))?;
        pipe::set_nonblocking(wake_r.as_fd()).map_err(|e| conn(&e))?;
        pipe::set_nonblocking(wake_w.as_fd()).map_err(|e| conn(&e))?;
        let (tx, rx) = mpsc::channel();
        let mailbox = Mailbox {
            tx,
            wake: Arc::new(File::from(wake_w)),
        };
        let shared = Arc::new(Mutex::new(Shared::default()));
        let chunk = (xconn.maximum_request_bytes() / 2).clamp(4096, 256 * 1024);
        let mut actor = Actor {
            conn: xconn,
            win,
            atoms,
            names: HashMap::new(),
            atom_of: HashMap::new(),
            owner_of: HashMap::new(),
            owned: HashMap::new(),
            owning: VecDeque::new(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
            free_props: Vec::new(),
            next_prop: 0,
            queued: VecDeque::new(),
            subs,
            shared: shared.clone(),
            mailbox: mailbox.clone(),
            events: Vec::new(),
            chunk,
        };
        let thread = std::thread::Builder::new()
            .name("transfer-x11".into())
            .spawn(move || actor.run(rx, wake_r))
            .map_err(|e| conn(&e))?;
        Ok(X11 {
            mailbox,
            shared,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Takes `selection` with `spec`. The actor completes it (after a round trip for the
    /// server's timestamp) without the caller waiting; a refusal arrives as
    /// [`Event::SourceCancelled`], and [`X11::owns`] turns true once it holds.
    pub(crate) fn set_selection(
        &self,
        selection: Selection,
        spec: SourceSpec,
    ) -> Result<SourceId, Error> {
        if spec.is_empty() {
            return Err(Error::Refused("a source with no types"));
        }
        let id = SourceId::next();
        let own = Cmd::Own {
            selection,
            id,
            spec,
        };
        self.mailbox.post(own).map_err(|_| Error::Shutdown)?;
        Ok(id)
    }

    pub(crate) fn selection(&self, selection: Selection) -> Option<SelectionOffer> {
        lock(&self.shared).offers.get(&selection).cloned()
    }

    pub(crate) fn owns(&self, source: SourceId) -> bool {
        lock(&self.shared).owned.contains(&source.get())
    }

    /// Stops the actor and joins it. Its window goes, which gives up every selection it holds.
    pub(crate) fn shutdown(&self) {
        let _ = self.mailbox.post(Cmd::Quit);
        if let Some(thread) = lock(&self.thread).take() {
            let _ = thread.join();
        }
    }
}

/// Another client's X11 selection, as of one owner.
struct XOffer {
    selection: Selection,
    owner: Window,
    mimes: Vec<String>,
    mailbox: Mailbox,
}

impl ReadOffer for XOffer {
    fn mimes(&self) -> &[String] {
        &self.mimes
    }

    fn read(&self, mime: &str, limits: ReadLimits, done: Done) {
        let cmd = Cmd::Read {
            selection: self.selection,
            owner: self.owner,
            mime: mime.to_owned(),
            limits,
            done,
        };
        if let Err(Cmd::Read { done, .. }) = self.mailbox.post(cmd) {
            done(Err(ReadError::Shutdown));
        }
    }
}

/// The actor's state; it lives on the actor thread.
struct Actor {
    conn: RustConnection,
    win: Window,
    atoms: Atoms,
    names: HashMap<Atom, String>,
    atom_of: HashMap<String, Atom>,
    /// Each selection's owner, as XFixes last said.
    owner_of: HashMap<Selection, Window>,
    owned: HashMap<Atom, selection::Owned>,
    /// Ownership requests waiting for a server timestamp, in order.
    owning: VecDeque<selection::Owning>,
    outgoing: Vec<Outgoing>,
    incoming: Vec<Incoming>,
    free_props: Vec<Atom>,
    next_prop: u32,
    /// Reads waiting for a free property.
    queued: VecDeque<selection::Wanted>,
    subs: Subscribers,
    shared: Arc<Mutex<Shared>>,
    mailbox: Mailbox,
    events: Vec<Event>,
    /// The largest property written at once; more goes by INCR.
    chunk: usize,
}

impl Actor {
    fn run(&mut self, rx: Receiver<Cmd>, wake: OwnedFd) {
        let mut wake = File::from(wake);
        for selection in [Selection::Clipboard, Selection::Primary] {
            self.check_owner(selection);
        }
        loop {
            let mut busy = false;
            loop {
                match self.conn.poll_for_event() {
                    Ok(Some(event)) => {
                        busy = true;
                        self.handle(event);
                    }
                    Ok(None) => break,
                    Err(e) => return self.fail(e.to_string()),
                }
            }
            while let Ok(cmd) = rx.try_recv() {
                busy = true;
                if matches!(cmd, Cmd::Quit) {
                    return self.quit();
                }
                self.command(cmd);
            }
            self.expire(Instant::now());
            if let Err(e) = self.conn.flush() {
                return self.fail(e.to_string());
            }
            self.subs.emit(std::mem::take(&mut self.events));
            if busy {
                // Handling may have read more events off the socket (replies do): drain them
                // before sleeping.
                continue;
            }
            let timeout = self
                .next_deadline()
                .map(|d| d.saturating_duration_since(Instant::now()));
            let ready = pipe::poll(
                &[
                    (self.conn.stream().as_fd(), libc::POLLIN),
                    (wake.as_fd(), libc::POLLIN),
                ],
                timeout,
            );
            match ready {
                Ok(r) if r[1] != 0 => {
                    let mut buf = [0; 64];
                    while matches!(wake.read(&mut buf), Ok(n) if n > 0) {}
                }
                Ok(_) => {}
                Err(e) => return self.fail(format!("poll: {e}")),
            }
        }
    }

    fn fail(&mut self, message: String) {
        self.events.push(Event::Failed { message });
        self.subs.emit(std::mem::take(&mut self.events));
        self.abort_reads();
    }

    fn quit(&mut self) {
        self.abort_reads();
        let _ = self.conn.destroy_window(self.win);
        let _ = self.conn.flush();
    }

    fn abort_reads(&mut self) {
        for read in self.incoming.drain(..) {
            read.fail(ReadError::Shutdown);
        }
        for wanted in self.queued.drain(..) {
            wanted.fail(ReadError::Shutdown);
        }
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

    fn atom(&mut self, name: &str) -> Option<Atom> {
        if let Some(a) = self.atom_of.get(name) {
            return Some(*a);
        }
        let atom = self
            .conn
            .intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom;
        self.atom_of.insert(name.to_owned(), atom);
        self.names.insert(atom, name.to_owned());
        Some(atom)
    }

    fn name(&mut self, atom: Atom) -> Option<String> {
        if let Some(n) = self.names.get(&atom) {
            return Some(n.clone());
        }
        let reply = self.conn.get_atom_name(atom).ok()?.reply().ok()?;
        let name = String::from_utf8_lossy(&reply.name).into_owned();
        self.names.insert(atom, name.clone());
        self.atom_of.insert(name.clone(), atom);
        Some(name)
    }

    /// A property of our window to receive one conversion into.
    fn take_prop(&mut self) -> Option<Atom> {
        if let Some(p) = self.free_props.pop() {
            return Some(p);
        }
        let n = self.next_prop;
        self.next_prop += 1;
        self.atom(&format!("_STRATA_TRANSFER_{n}"))
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.incoming
            .iter()
            .map(Incoming::deadline)
            .chain(self.outgoing.iter().map(Outgoing::deadline))
            .min()
    }
}
