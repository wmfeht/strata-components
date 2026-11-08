//! Wayland offers: selection offers (data device and data control) and drag offers.
//!
//! P0 owns every offer proxy. A subscriber holds an offer handle that shares the proxy, so a
//! read of an offer P0 has since destroyed fails with [`ReadError::Gone`] instead of sending a
//! request on a dead object. Offers hold the connection weakly: one kept after shutdown never
//! keeps winit's display binding alive (dropping it late crashed S2's runs).
//!
//! On winit's display every request on an offer must come from the UI thread, and is refused
//! from any other ([`ReadError::OffUiThread`], [`Error::Refused`]). A request upgrades the weak
//! connection for a moment to flush; on another thread, racing the UI thread's shutdown, that
//! moment could hold the last strong reference, and the backend would then be dropped on that
//! thread, perhaps after winit closed the display. A connection of our own has no such rule.
//!
//! A read's `done` never runs while an offer's lock is held, so it may call the offer again.
//!
//! A drag offer checks each request against [`DropState`] before sending it: the requests
//! `wl_data_offer` forbids are fatal protocol errors, and on winit's display a protocol error
//! ends winit's connection too.

use std::fmt;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::ThreadId;

use wayland_backend::client::{ObjectId, WeakBackend};
use wayland_client::protocol::wl_data_device_manager::DndAction;
use wayland_client::protocol::wl_data_offer::WlDataOffer;
use wayland_client::{Connection, Proxy};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::ExtDataControlOfferV1;
use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1;

use super::super::data::ReadLimits;
use super::super::error::{Error, ReadError};
use super::super::event::{Action, Done, ReadOffer};
use super::super::pipe;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The connection an offer belongs to, and the thread its requests must come from.
#[derive(Clone)]
pub(super) struct Home {
    pub(super) backend: WeakBackend,
    /// winit's display: the UI thread. A connection of our own: None, any thread.
    pub(super) ui: Option<ThreadId>,
}

impl Home {
    /// Whether this thread may send requests on the connection.
    pub(super) fn here(&self) -> bool {
        self.ui.is_none_or(|ui| ui == std::thread::current().id())
    }

    pub(super) fn flush(&self) {
        if let Some(b) = self.backend.upgrade() {
            let _ = Connection::from_backend(b).flush();
        }
    }
}

/// An offer proxy of any of the three protocols.
pub(super) enum AnyOffer {
    Device(WlDataOffer),
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}

impl AnyOffer {
    fn receive(&self, mime: &str, fd: std::os::fd::BorrowedFd<'_>) {
        match self {
            AnyOffer::Device(o) => o.receive(mime.to_owned(), fd),
            AnyOffer::Ext(o) => o.receive(mime.to_owned(), fd),
            AnyOffer::Wlr(o) => o.receive(mime.to_owned(), fd),
        }
    }

    fn destroy(&self) {
        match self {
            AnyOffer::Device(o) => o.destroy(),
            AnyOffer::Ext(o) => o.destroy(),
            AnyOffer::Wlr(o) => o.destroy(),
        }
    }
}

/// Sends `receive` for `mime` on `offer`: the read end of the pipe the source writes into.
/// Call it with the offer's lock held, and start the read after releasing it.
fn receive(offer: &AnyOffer, home: &Home, mime: &str) -> Result<OwnedFd, ReadError> {
    let (r, w) = pipe::pipe().map_err(|e| ReadError::Io(e.to_string()))?;
    offer.receive(mime, w.as_fd());
    home.flush();
    // Our copy of the write end must close, or the read never sees EOF.
    drop(w);
    Ok(r)
}

/// Starts the read of a pipe from [`receive`], or reports why there is none. No lock held.
fn finish_read(r: Result<OwnedFd, ReadError>, limits: ReadLimits, done: Done) {
    match r {
        Ok(fd) => pipe::spawn_read(fd, limits, done),
        Err(e) => done(Err(e)),
    }
}

/// A selection offer: the proxy until P0 replaces or destroys it.
pub(super) struct WlSelection {
    mimes: Vec<String>,
    proxy: Mutex<Option<AnyOffer>>,
    home: Home,
}

impl WlSelection {
    pub(super) fn new(proxy: Option<AnyOffer>, mimes: Vec<String>, home: Home) -> Arc<Self> {
        Arc::new(WlSelection {
            mimes,
            proxy: Mutex::new(proxy),
            home,
        })
    }

    /// Destroys the proxy; later reads fail with `Gone`.
    pub(super) fn close(&self) {
        if let Some(p) = lock(&self.proxy).take() {
            p.destroy();
        }
    }
}

impl ReadOffer for WlSelection {
    fn mimes(&self) -> &[String] {
        &self.mimes
    }

    fn read(&self, mime: &str, limits: ReadLimits, done: Done) {
        if !self.home.here() {
            return done(Err(ReadError::OffUiThread));
        }
        // The lock spans the request, so a close on another thread can't land between the
        // liveness check and `receive`; `done` runs after it is released.
        let r = match lock(&self.proxy).as_ref() {
            Some(p) => receive(p, &self.home, mime),
            None => Err(ReadError::Gone),
        };
        finish_read(r, limits, done);
    }
}

/// What a drag offer has done so far, to refuse the requests `wl_data_offer` forbids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DropState {
    /// The offer's protocol version: actions and `finish` need 3.
    pub(crate) version: u32,
    accepted: Option<String>,
    /// The action the compositor settled on (`wl_data_offer.action`).
    action: Option<u32>,
    dropped: bool,
    finished: bool,
}

impl DropState {
    pub(crate) fn new(version: u32) -> Self {
        DropState {
            version,
            ..DropState::default()
        }
    }

    pub(crate) fn accept(
        &mut self,
        mime: Option<&str>,
        offered: &[String],
    ) -> Result<(), &'static str> {
        if self.finished {
            return Err("the drop is finished");
        }
        if let Some(m) = mime
            && !offered.iter().any(|o| o == m)
        {
            return Err("a type the drag doesn't offer");
        }
        self.accepted = mime.map(str::to_owned);
        Ok(())
    }

    /// `invalid_action_mask` and `invalid_action`, and the version.
    pub(crate) fn set_actions(&mut self, actions: u32, preferred: u32) -> Result<(), &'static str> {
        if self.finished {
            return Err("the drop is finished");
        }
        if self.version < 3 {
            return Err("set_actions needs wl_data_offer version 3");
        }
        if actions & !7 != 0 {
            return Err("an action outside copy, move and ask");
        }
        if preferred != 0 && (preferred.count_ones() != 1 || preferred & actions == 0) {
            return Err("a preferred action that isn't one of the actions");
        }
        // After a drop whose action is ask, this request settles it, and the compositor sends
        // no further action event: the preferred action is the one taken.
        if self.dropped && self.action == Some(Action::Ask.bits()) && preferred != 0 {
            self.action = Some(preferred);
        }
        Ok(())
    }

    /// `invalid_finish`: only after the drop, with a type accepted and an action other than
    /// ask settled. True when the `finish` request exists (version 3); older offers are only
    /// destroyed.
    pub(crate) fn finish(&mut self) -> Result<bool, &'static str> {
        if self.finished {
            return Err("the drop is finished");
        }
        if !self.dropped {
            return Err("finish before the drop");
        }
        if self.accepted.is_none() {
            return Err("finish after accepting no type");
        }
        if self.version >= 3 {
            match self.action {
                None | Some(0) => return Err("finish before the compositor chose an action"),
                Some(a) if a == Action::Ask.bits() => {
                    return Err("finish while the action is still ask");
                }
                Some(_) => {}
            }
        }
        self.finished = true;
        Ok(self.version >= 3)
    }

    pub(crate) fn on_action(&mut self, action: u32) {
        self.action = Some(action);
    }

    pub(crate) fn on_drop(&mut self) {
        self.dropped = true;
    }

    pub(crate) fn dropped(&self) -> bool {
        self.dropped
    }
}

struct DragCell {
    proxy: Option<WlDataOffer>,
    state: DropState,
}

struct DragInner {
    id: ObjectId,
    mimes: Vec<String>,
    serial: u32,
    cell: Mutex<DragCell>,
    home: Home,
}

/// A drag over one of the app's windows (Wayland): its MIME types, what the app accepts, the
/// read after the drop, and `finish`. P0 keeps a dropped offer alive until `finish`, though
/// `leave` follows the drop at once.
#[derive(Clone)]
pub struct DragOffer(Arc<DragInner>);

impl DragOffer {
    pub(super) fn new(proxy: WlDataOffer, mimes: Vec<String>, serial: u32, home: Home) -> Self {
        let (id, version) = (proxy.id(), proxy.version());
        DragOffer::with(Some(proxy), id, version, mimes, serial, home)
    }

    fn with(
        proxy: Option<WlDataOffer>,
        id: ObjectId,
        version: u32,
        mimes: Vec<String>,
        serial: u32,
        home: Home,
    ) -> Self {
        DragOffer(Arc::new(DragInner {
            id,
            mimes,
            serial,
            cell: Mutex::new(DragCell {
                state: DropState::new(version),
                proxy,
            }),
            home,
        }))
    }

    fn on_ui(&self) -> Result<(), Error> {
        if self.0.home.here() {
            Ok(())
        } else {
            Err(Error::Refused(
                "a drag offer on winit's display takes requests on the UI thread only",
            ))
        }
    }

    pub fn mimes(&self) -> &[String] {
        &self.0.mimes
    }

    /// Accepts `mime` (or nothing) at the current position. Send it on enter and on every
    /// motion: the source shows whether a drop would land.
    pub fn accept(&self, mime: Option<&str>) -> Result<(), Error> {
        self.on_ui()?;
        let mut cell = lock(&self.0.cell);
        cell.state
            .accept(mime, &self.0.mimes)
            .map_err(Error::Refused)?;
        let proxy = cell
            .proxy
            .as_ref()
            .ok_or(Error::Refused("the drag is gone"))?;
        proxy.accept(self.0.serial, mime.map(str::to_owned));
        drop(cell);
        self.0.home.flush();
        Ok(())
    }

    /// The actions this target takes, and the one it prefers.
    pub fn set_actions(&self, actions: &[Action], preferred: Option<Action>) -> Result<(), Error> {
        let mask = actions.iter().fold(0, |m, a| m | a.bits());
        let pref = preferred.map_or(0, Action::bits);
        self.on_ui()?;
        let mut cell = lock(&self.0.cell);
        cell.state.set_actions(mask, pref).map_err(Error::Refused)?;
        let proxy = cell
            .proxy
            .as_ref()
            .ok_or(Error::Refused("the drag is gone"))?;
        proxy.set_actions(
            DndAction::from_bits_truncate(mask),
            DndAction::from_bits_truncate(pref),
        );
        drop(cell);
        self.0.home.flush();
        Ok(())
    }

    /// Reads one type, during the drag or after the drop. `done` runs once: on a worker with
    /// the data, or on this thread at once if the read can't start (a type not offered, the
    /// drag gone, the wrong thread). It may call this offer again.
    pub fn read(
        &self,
        mime: &str,
        limits: ReadLimits,
        done: impl FnOnce(Result<Vec<u8>, ReadError>) + Send + 'static,
    ) {
        if !self.0.mimes.iter().any(|m| m == mime) {
            return done(Err(ReadError::NotOffered));
        }
        if !self.0.home.here() {
            return done(Err(ReadError::OffUiThread));
        }
        let r = match lock(&self.0.cell).proxy.as_ref() {
            Some(p) => receive(&AnyOffer::Device(p.clone()), &self.0.home, mime),
            None => Err(ReadError::Gone),
        };
        finish_read(r, limits, Box::new(done));
    }

    /// Ends the drop once its reads are done: the source learns the drop succeeded.
    pub fn finish(&self) -> Result<(), Error> {
        self.on_ui()?;
        let mut cell = lock(&self.0.cell);
        let send_finish = cell.state.finish().map_err(Error::Refused)?;
        if let Some(p) = cell.proxy.take() {
            if send_finish {
                p.finish();
            }
            p.destroy();
        }
        drop(cell);
        self.0.home.flush();
        Ok(())
    }

    pub(super) fn is(&self, id: &ObjectId) -> bool {
        self.0.id == *id
    }

    pub(super) fn with_state<R>(&self, f: impl FnOnce(&mut DropState) -> R) -> R {
        f(&mut lock(&self.0.cell).state)
    }

    pub(super) fn is_open(&self) -> bool {
        lock(&self.0.cell).proxy.is_some()
    }

    /// Destroys the proxy without finishing: the drag left, or the substrate shuts down.
    pub(super) fn close(&self) {
        if let Some(p) = lock(&self.0.cell).proxy.take() {
            p.destroy();
        }
    }
}

#[cfg(test)]
impl DragOffer {
    /// A drag offer with no proxy, as one is once it is gone.
    pub(super) fn closed(mimes: Vec<String>, home: Home) -> Self {
        DragOffer::with(None, ObjectId::null(), 3, mimes, 0, home)
    }
}

impl fmt::Debug for DragOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DragOffer").field(&self.0.mimes).finish()
    }
}

#[cfg(test)]
mod tests;
