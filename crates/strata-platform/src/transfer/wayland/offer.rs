//! Wayland offers: selection offers (data device and data control) and drag offers.
//!
//! P0 owns every offer proxy. A subscriber holds an offer handle that shares the proxy, so a
//! read of an offer P0 has since destroyed fails with [`ReadError::Gone`] instead of sending a
//! request on a dead object. Offers hold the connection weakly: one kept after shutdown never
//! keeps winit's display binding alive (dropping it late crashed S2's runs).
//!
//! A drag offer checks each request against [`DropState`] before sending it: the requests
//! `wl_data_offer` forbids are fatal protocol errors, and on winit's display a protocol error
//! ends winit's connection too.

use std::fmt;
use std::os::fd::AsFd;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

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

pub(super) fn flush(backend: &WeakBackend) {
    if let Some(b) = backend.upgrade() {
        let _ = Connection::from_backend(b).flush();
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

/// Sends `receive` for `mime` on `offer` and reads the pipe on a worker.
fn receive(offer: &AnyOffer, backend: &WeakBackend, mime: &str, limits: ReadLimits, done: Done) {
    let (r, w) = match pipe::pipe() {
        Ok(p) => p,
        Err(e) => return done(Err(ReadError::Io(e.to_string()))),
    };
    offer.receive(mime, w.as_fd());
    flush(backend);
    // Our copy of the write end must close, or the read never sees EOF.
    drop(w);
    pipe::spawn_read(r, limits, done);
}

/// A selection offer: the proxy until P0 replaces or destroys it.
pub(super) struct WlSelection {
    mimes: Vec<String>,
    proxy: Mutex<Option<AnyOffer>>,
    backend: WeakBackend,
}

impl WlSelection {
    pub(super) fn new(proxy: AnyOffer, mimes: Vec<String>, backend: WeakBackend) -> Arc<Self> {
        Arc::new(WlSelection {
            mimes,
            proxy: Mutex::new(Some(proxy)),
            backend,
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
        // The lock spans the request, so a close on another thread can't land between the
        // liveness check and `receive`.
        let proxy = lock(&self.proxy);
        match proxy.as_ref() {
            Some(p) => receive(p, &self.backend, mime, limits, done),
            None => done(Err(ReadError::Gone)),
        }
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
    backend: WeakBackend,
}

/// A drag over one of the app's windows (Wayland): its MIME types, what the app accepts, the
/// read after the drop, and `finish`. P0 keeps a dropped offer alive until `finish`, though
/// `leave` follows the drop at once.
#[derive(Clone)]
pub struct DragOffer(Arc<DragInner>);

impl DragOffer {
    pub(super) fn new(
        proxy: WlDataOffer,
        mimes: Vec<String>,
        serial: u32,
        backend: WeakBackend,
    ) -> Self {
        DragOffer(Arc::new(DragInner {
            id: proxy.id(),
            mimes,
            serial,
            cell: Mutex::new(DragCell {
                state: DropState::new(proxy.version()),
                proxy: Some(proxy),
            }),
            backend,
        }))
    }

    pub fn mimes(&self) -> &[String] {
        &self.0.mimes
    }

    /// Accepts `mime` (or nothing) at the current position. Send it on enter and on every
    /// motion: the source shows whether a drop would land.
    pub fn accept(&self, mime: Option<&str>) -> Result<(), Error> {
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
        flush(&self.0.backend);
        Ok(())
    }

    /// The actions this target takes, and the one it prefers.
    pub fn set_actions(&self, actions: &[Action], preferred: Option<Action>) -> Result<(), Error> {
        let mask = actions.iter().fold(0, |m, a| m | a.bits());
        let pref = preferred.map_or(0, Action::bits);
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
        flush(&self.0.backend);
        Ok(())
    }

    /// Reads one type, during the drag or after the drop. `done` runs off the UI thread.
    pub fn read(
        &self,
        mime: &str,
        limits: ReadLimits,
        done: impl FnOnce(Result<Vec<u8>, ReadError>) + Send + 'static,
    ) {
        if !self.0.mimes.iter().any(|m| m == mime) {
            return done(Err(ReadError::NotOffered));
        }
        let cell = lock(&self.0.cell);
        match &cell.proxy {
            Some(p) => receive(
                &AnyOffer::Device(p.clone()),
                &self.0.backend,
                mime,
                limits,
                Box::new(done),
            ),
            None => done(Err(ReadError::Gone)),
        }
    }

    /// Ends the drop once its reads are done: the source learns the drop succeeded.
    pub fn finish(&self) -> Result<(), Error> {
        let mut cell = lock(&self.0.cell);
        let send_finish = cell.state.finish().map_err(Error::Refused)?;
        if let Some(p) = cell.proxy.take() {
            if send_finish {
                p.finish();
            }
            p.destroy();
        }
        drop(cell);
        flush(&self.0.backend);
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

impl fmt::Debug for DragOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DragOffer").field(&self.0.mimes).finish()
    }
}

#[cfg(test)]
mod tests;
