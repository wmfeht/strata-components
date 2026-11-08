//! ICCCM selections on the actor thread: owning one (with a server timestamp), answering
//! `SelectionRequest`s (INCR above one chunk), converting another client's selection (INCR
//! too), and following owners through XFixes.

use std::sync::Arc;
use std::time::Instant;

use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xfixes::SelectionNotifyEvent as XfixesNotify;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, PropMode, Property,
    PropertyNotifyEvent, SELECTION_NOTIFY_EVENT, SelectionClearEvent, SelectionNotifyEvent,
    SelectionRequestEvent, Window,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use super::super::data::{ReadLimits, SourceSpec, WRITE_IDLE};
use super::super::error::ReadError;
use super::super::event::{Done, Event, Selection, SelectionOffer, SourceId};
use super::{Actor, Cmd, XOffer, lock};

/// A selection we hold.
pub(super) struct Owned {
    id: SourceId,
    spec: SourceSpec,
    time: u32,
}

/// An ownership request waiting for the server's timestamp.
pub(super) struct Owning {
    selection: Selection,
    id: SourceId,
    spec: SourceSpec,
}

/// A `SelectionRequest` to answer.
pub(super) struct Request {
    requestor: Window,
    selection: Atom,
    target: Atom,
    property: Atom,
    time: u32,
}

/// An answer too large for one property, sent in chunks as the requestor deletes each.
pub(super) struct Outgoing {
    requestor: Window,
    property: Atom,
    target: Atom,
    data: Arc<[u8]>,
    offset: usize,
    /// The zero-length chunk that ends it has been written.
    ended: bool,
    deadline: Instant,
}

impl Outgoing {
    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }
}

pub(super) enum Then {
    /// An app's read.
    User(Done),
    /// The `TARGETS` of a new owner, to announce its offer.
    Targets(Window),
}

/// A read waiting for a free property.
pub(super) struct Wanted {
    selection: Selection,
    target: Atom,
    limits: ReadLimits,
    then: Then,
}

impl Wanted {
    pub(super) fn fail(self, e: ReadError) {
        if let Then::User(done) = self.then {
            done(Err(e));
        }
    }
}

/// A conversion in flight, into one property of our window.
pub(super) struct Incoming {
    want: Wanted,
    property: Atom,
    /// Started INCR: chunks arrive as new values of the property.
    incr: bool,
    buf: Vec<u8>,
    deadline: Instant,
}

impl Incoming {
    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn fail(self, e: ReadError) {
        self.want.fail(e);
    }
}

impl Actor {
    pub(super) fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Own {
                selection,
                id,
                spec,
            } => {
                // ICCCM: own with a real timestamp, never CurrentTime. A zero-length append to
                // our own window makes the server send a PropertyNotify carrying its time.
                let _ = self.conn.change_property8(
                    PropMode::APPEND,
                    self.win,
                    self.atoms.TRANSFER_TIME,
                    AtomEnum::STRING,
                    &[],
                );
                self.owning.push_back(Owning {
                    selection,
                    id,
                    spec,
                });
            }
            Cmd::Read {
                selection,
                owner,
                mime,
                limits,
                done,
            } => {
                if self.owner_of.get(&selection) != Some(&owner) {
                    return done(Err(ReadError::Gone));
                }
                let Some(target) = self.atom(&mime) else {
                    return done(Err(ReadError::Io("couldn't intern the type".into())));
                };
                self.start_read(Wanted {
                    selection,
                    target,
                    limits,
                    then: Then::User(done),
                });
            }
            Cmd::Ready { request, bytes } => self.answer(request, bytes),
            Cmd::Quit => {}
        }
    }

    pub(super) fn handle(&mut self, event: XEvent) {
        match event {
            XEvent::PropertyNotify(e) => self.property(e),
            XEvent::SelectionRequest(r) => self.request(r),
            XEvent::SelectionNotify(n) => self.converted(n),
            XEvent::SelectionClear(c) => self.cleared(c),
            XEvent::XfixesSelectionNotify(n) => self.owner_changed(n),
            _ => {}
        }
    }

    fn property(&mut self, e: PropertyNotifyEvent) {
        if e.window == self.win && e.atom == self.atoms.TRANSFER_TIME {
            if let Some(owning) = self.owning.pop_front() {
                self.own(owning, e.time);
            }
            return;
        }
        if e.window == self.win && e.state == Property::NEW_VALUE {
            if let Some(i) = self
                .incoming
                .iter()
                .position(|r| r.incr && r.property == e.atom)
            {
                self.incr_chunk(i);
            }
            return;
        }
        if e.state == Property::DELETE
            && let Some(i) = self
                .outgoing
                .iter()
                .position(|o| o.requestor == e.window && o.property == e.atom)
        {
            self.next_chunk(i);
        }
    }

    fn own(&mut self, o: Owning, time: u32) {
        let sel = self.selection_atom(o.selection);
        let _ = self.conn.set_selection_owner(self.win, sel, time);
        let ours = self
            .conn
            .get_selection_owner(sel)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.owner == self.win);
        if !ours {
            // Another client took it in between: the take failed, as a loss would show.
            self.events.push(Event::SourceCancelled { source: o.id });
            return;
        }
        if let Some(old) = self.owned.insert(
            sel,
            Owned {
                id: o.id,
                spec: o.spec,
                time,
            },
        ) {
            // Our own earlier source on this selection is replaced: it is cancelled, as on
            // Wayland.
            lock(&self.shared).owned.remove(&old.id.get());
            self.events.push(Event::SourceCancelled { source: old.id });
        }
        lock(&self.shared).owned.insert(o.id.get());
    }

    fn cleared(&mut self, c: SelectionClearEvent) {
        if c.owner != self.win {
            return;
        }
        if let Some(owned) = self.owned.remove(&c.selection) {
            lock(&self.shared).owned.remove(&owned.id.get());
            self.events
                .push(Event::SourceCancelled { source: owned.id });
        }
    }

    fn owner_changed(&mut self, n: XfixesNotify) {
        if let Some(selection) = self.selection_of(n.selection) {
            self.new_owner(selection, n.owner);
        }
    }

    pub(super) fn check_owner(&mut self, selection: Selection) {
        let sel = self.selection_atom(selection);
        let owner = self
            .conn
            .get_selection_owner(sel)
            .ok()
            .and_then(|c| c.reply().ok())
            .map_or(NONE, |r| r.owner);
        if owner != NONE {
            self.new_owner(selection, owner);
        }
    }

    fn new_owner(&mut self, selection: Selection, owner: Window) {
        self.owner_of.insert(selection, owner);
        if owner == NONE {
            lock(&self.shared).offers.remove(&selection);
            self.events.push(Event::SelectionChanged {
                selection,
                route: super::super::event::Route::X11,
                offer: None,
            });
            return;
        }
        if owner == self.win {
            // Our own: no need to ask ourselves for TARGETS.
            let sel = self.selection_atom(selection);
            let mimes = self
                .owned
                .get(&sel)
                .map(|o| o.spec.mimes().map(str::to_owned).collect())
                .unwrap_or_default();
            self.announce(selection, owner, mimes);
            return;
        }
        let targets = self.atoms.TARGETS;
        self.start_read(Wanted {
            selection,
            target: targets,
            limits: ReadLimits::default(),
            then: Then::Targets(owner),
        });
    }

    fn announce(&mut self, selection: Selection, owner: Window, mimes: Vec<String>) {
        let offer = SelectionOffer(Arc::new(XOffer {
            selection,
            owner,
            mimes,
            mailbox: self.mailbox.clone(),
        }));
        lock(&self.shared).offers.insert(selection, offer.clone());
        self.events.push(Event::SelectionChanged {
            selection,
            route: super::super::event::Route::X11,
            offer: Some(offer),
        });
    }

    // ---- reading another client's selection ----

    fn start_read(&mut self, want: Wanted) {
        let Some(property) = self.take_prop() else {
            self.queued.push_back(want);
            return;
        };
        let sel = self.selection_atom(want.selection);
        let _ = self
            .conn
            .convert_selection(self.win, sel, want.target, property, CURRENT_TIME);
        let deadline = Instant::now() + want.limits.idle;
        self.incoming.push(Incoming {
            want,
            property,
            incr: false,
            buf: Vec::new(),
            deadline,
        });
    }

    fn converted(&mut self, n: SelectionNotifyEvent) {
        if n.requestor != self.win {
            return;
        }
        let Some(selection) = self.selection_of(n.selection) else {
            return;
        };
        let found = if n.property == NONE {
            // A refusal names no property: match the waiting conversion by what it asked.
            self.incoming
                .iter()
                .position(|r| !r.incr && r.want.selection == selection && r.want.target == n.target)
        } else {
            self.incoming
                .iter()
                .position(|r| !r.incr && r.property == n.property)
        };
        let Some(i) = found else {
            return;
        };
        if n.property == NONE {
            let read = self.incoming.swap_remove(i);
            self.free_props.push(read.property);
            return self.finish(read, Err(ReadError::NotOffered));
        }
        let property = self.incoming[i].property;
        let max = self.incoming[i].want.limits.max_bytes;
        match self.take_property(property, max) {
            Ok((ty, _size)) if ty == self.atoms.INCR => {
                // The property held the size; deleting it (done) asks for the first chunk.
                let read = &mut self.incoming[i];
                read.incr = true;
                read.deadline = Instant::now() + read.want.limits.idle;
            }
            Ok((_, data)) => {
                let read = self.incoming.swap_remove(i);
                self.free_props.push(read.property);
                self.finish(read, Ok(data));
            }
            Err(e) => {
                let read = self.incoming.swap_remove(i);
                self.free_props.push(read.property);
                self.finish(read, Err(e));
            }
        }
    }

    fn incr_chunk(&mut self, i: usize) {
        let property = self.incoming[i].property;
        let left = self.incoming[i]
            .want
            .limits
            .max_bytes
            .saturating_sub(self.incoming[i].buf.len());
        match self.take_property(property, left) {
            Ok((_, chunk)) if chunk.is_empty() => {
                let mut read = self.incoming.swap_remove(i);
                self.free_props.push(read.property);
                let buf = std::mem::take(&mut read.buf);
                self.finish(read, Ok(buf));
            }
            Ok((_, chunk)) => {
                let read = &mut self.incoming[i];
                read.buf.extend_from_slice(&chunk);
                read.deadline = Instant::now() + read.want.limits.idle;
            }
            Err(e) => {
                let read = self.incoming.swap_remove(i);
                // Chunks may still arrive into it: don't reuse it.
                self.finish(read, Err(e));
            }
        }
    }

    /// Reads a whole property of our window and deletes it: its type and bytes.
    fn take_property(&mut self, property: Atom, max: usize) -> Result<(Atom, Vec<u8>), ReadError> {
        let mut data = Vec::new();
        let mut offset = 0u32;
        let ty = loop {
            let reply = self
                .conn
                .get_property(false, self.win, property, AtomEnum::ANY, offset, 1 << 16)
                .map_err(|e| ReadError::Io(e.to_string()))?
                .reply()
                .map_err(|e| ReadError::Io(e.to_string()))?;
            data.extend_from_slice(&reply.value);
            if data.len() > max {
                let _ = self.conn.delete_property(self.win, property);
                return Err(ReadError::TooLarge);
            }
            if reply.bytes_after == 0 {
                break reply.type_;
            }
            // Offsets count 32-bit units.
            offset += (reply.value.len() / 4) as u32;
        };
        let _ = self.conn.delete_property(self.win, property);
        Ok((ty, data))
    }

    fn finish(&mut self, read: Incoming, result: Result<Vec<u8>, ReadError>) {
        match read.want.then {
            Then::User(done) => done(result),
            Then::Targets(owner) => {
                // Only the owner we asked still counts; a newer one has its own request.
                if self.owner_of.get(&read.want.selection) != Some(&owner) {
                    return self.pump();
                }
                let bytes = result.unwrap_or_default();
                let atoms: Vec<Atom> = bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_ne_bytes(*c))
                    .collect();
                let meta = [
                    self.atoms.TARGETS,
                    self.atoms.TIMESTAMP,
                    self.atoms.MULTIPLE,
                ];
                let mimes = atoms
                    .into_iter()
                    .filter(|a| *a != NONE && !meta.contains(a))
                    .filter_map(|a| self.name(a))
                    .collect();
                self.announce(read.want.selection, owner, mimes);
            }
        }
        self.pump();
    }

    /// Starts queued reads now that a property is free.
    fn pump(&mut self) {
        while !self.free_props.is_empty() {
            let Some(want) = self.queued.pop_front() else {
                break;
            };
            self.start_read(want);
        }
    }

    // ---- answering requests for our selections ----

    fn request(&mut self, r: SelectionRequestEvent) {
        // Obsolete clients send no property: answer in the target's name.
        let property = if r.property == NONE {
            r.target
        } else {
            r.property
        };
        let req = Request {
            requestor: r.requestor,
            selection: r.selection,
            target: r.target,
            property,
            time: r.time,
        };
        let Some(owned) = self.owned.get(&r.selection).filter(|_| r.owner == self.win) else {
            return self.notify(&req, NONE);
        };
        if r.target == self.atoms.TARGETS {
            let mimes: Vec<String> = owned.spec.mimes().map(str::to_owned).collect();
            let mut list = vec![self.atoms.TARGETS, self.atoms.TIMESTAMP];
            list.extend(mimes.iter().filter_map(|m| self.atom(m)));
            let _ = self.conn.change_property32(
                PropMode::REPLACE,
                req.requestor,
                req.property,
                self.atoms.ATOM,
                &list,
            );
            return self.notify(&req, req.property);
        }
        if r.target == self.atoms.TIMESTAMP {
            let time = owned.time;
            let _ = self.conn.change_property32(
                PropMode::REPLACE,
                req.requestor,
                req.property,
                self.atoms.INTEGER,
                &[time],
            );
            return self.notify(&req, req.property);
        }
        let (id, spec) = (owned.id, owned.spec.clone());
        let Some(mime) = self.name(r.target) else {
            return self.notify(&req, NONE);
        };
        // MULTIPLE and unknown types are refused.
        let Some(writer) = spec.writer(&mime).cloned() else {
            return self.notify(&req, NONE);
        };
        self.events.push(Event::SourceAsked { source: id, mime });
        if let Some(bytes) = writer.ready() {
            return self.answer(req, bytes);
        }
        // The writer runs on a worker; its bytes come back as a command.
        let mailbox = self.mailbox.clone();
        let spawned = std::thread::Builder::new()
            .name("transfer-write".into())
            .spawn(move || {
                let bytes = writer.get();
                let _ = mailbox.post(Cmd::Ready {
                    request: req,
                    bytes,
                });
            });
        if let Err(e) = spawned {
            eprintln!("strata-platform transfer: no thread for an X11 write: {e}");
        }
    }

    fn answer(&mut self, req: Request, bytes: Option<Arc<[u8]>>) {
        let Some(data) = bytes else {
            return self.notify(&req, NONE);
        };
        if data.len() <= self.chunk {
            let _ = self.conn.change_property8(
                PropMode::REPLACE,
                req.requestor,
                req.property,
                req.target,
                &data,
            );
            return self.notify(&req, req.property);
        }
        // INCR: announce the size, then send a chunk each time the requestor deletes the
        // property, ending with an empty one.
        let _ = self.conn.change_window_attributes(
            req.requestor,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        );
        let len = u32::try_from(data.len()).unwrap_or(u32::MAX);
        let _ = self.conn.change_property32(
            PropMode::REPLACE,
            req.requestor,
            req.property,
            self.atoms.INCR,
            &[len],
        );
        self.notify(&req, req.property);
        self.outgoing.push(Outgoing {
            requestor: req.requestor,
            property: req.property,
            target: req.target,
            data,
            offset: 0,
            ended: false,
            deadline: Instant::now() + WRITE_IDLE,
        });
    }

    fn next_chunk(&mut self, i: usize) {
        let out = &mut self.outgoing[i];
        if out.ended {
            self.outgoing.swap_remove(i);
            return;
        }
        let end = (out.offset + self.chunk).min(out.data.len());
        let chunk = &out.data[out.offset..end];
        let _ = self.conn.change_property8(
            PropMode::REPLACE,
            out.requestor,
            out.property,
            out.target,
            chunk,
        );
        out.ended = chunk.is_empty();
        out.offset = end;
        out.deadline = Instant::now() + WRITE_IDLE;
    }

    fn notify(&mut self, req: &Request, property: Atom) {
        let event = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: req.time,
            requestor: req.requestor,
            selection: req.selection,
            target: req.target,
            property,
        };
        let _ = self
            .conn
            .send_event(false, req.requestor, EventMask::NO_EVENT, event);
    }

    // ---- timeouts ----

    pub(super) fn expire(&mut self, now: Instant) {
        self.outgoing.retain(|o| o.deadline > now);
        let mut i = 0;
        while i < self.incoming.len() {
            if self.incoming[i].deadline <= now {
                // Its property is dropped from the pool: late data may still land in it.
                let read = self.incoming.swap_remove(i);
                self.finish(read, Err(ReadError::TimedOut));
            } else {
                i += 1;
            }
        }
    }
}
