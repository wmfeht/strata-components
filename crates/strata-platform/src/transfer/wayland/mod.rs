//! Wayland: protocols bound on winit's own display (shared), or on a connection of our own.
//!
//! **Shared** binds a second `wl_seat` (with its own `wl_pointer` and `wl_keyboard`, for input
//! serials only, never input), `wl_data_device`, and data control if asked, in a queue of our
//! own on winit's display. Rune exposes no display handle (draft U9, not filed), so it is
//! recovered from the window's surface ([`display`]). The reader thread ([`reader`]) asks the
//! UI thread for a turn, which runs [`Conn::dispatch`]: handlers run on the UI thread, where
//! `WindowHandle::with_raw_handle` may be called. Any protocol error on these objects ends
//! winit's connection with ours, so requests the protocols forbid are refused before they are
//! sent.
//!
//! **Own** connects to `WAYLAND_DISPLAY` and binds data control alone: no surface, serial or
//! focus needed, and it works on libwayland 1.22. Its thread dispatches by itself.

mod control;
mod display;
mod offer;
mod reader;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use rune::WindowHandle;
use rune::raw_window_handle::RawWindowHandle;
use wayland_backend::client::ObjectId;
use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_data_source::{self, WlDataSource};
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, event_created_child,
};

use super::Turn;
use super::data::SourceSpec;
use super::error::{Error, Unsupported};
use super::event::{Action, Event, Route, Selection, SelectionOffer, SourceId, Subscribers};
use super::gate::Gate;
use super::pipe;
use control::Control;
pub use offer::DragOffer;
use offer::{AnyOffer, Home, WlSelection};
use reader::Reader;

/// Which data-control protocol to bind when the compositor has both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Prefer {
    /// `ext_data_control_v1`, the standard one.
    #[default]
    Ext,
    /// `zwlr_data_control_v1`, which older wlroots compositors have alone.
    Wlr,
}

const BTN_LEFT: u32 = 0x110;

const ON_UI: &str = "winit's display: call on the UI thread";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Shared,
    Own,
}

/// The current selection offer on one route.
struct Current {
    offer: Arc<WlSelection>,
}

/// A drag in progress over this client.
struct Drag {
    offer: DragOffer,
    /// The app window it is over (its key), or None for a surface that is no window of the
    /// app's (winit's decoration subsurfaces): those get no events.
    window: Option<String>,
}

enum SourceProxy {
    Device(WlDataSource),
    Control(control::Source),
}

impl SourceProxy {
    fn destroy(&self) {
        match self {
            SourceProxy::Device(s) => s.destroy(),
            SourceProxy::Control(s) => s.destroy(),
        }
    }
}

struct Source {
    spec: SourceSpec,
    proxy: SourceProxy,
}

/// What the queue's handlers update. Handlers of a shared connection run on the UI thread.
pub(crate) struct State {
    kind: Kind,
    home: Home,
    windows: Arc<Mutex<Vec<WindowHandle>>>,
    seat: Option<WlSeat>,
    /// For P1's drag icons, bound on winit's display only.
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    ddm: Option<WlDataDeviceManager>,
    device: Option<WlDataDevice>,
    control: Option<Control>,
    /// The newest key or button serial; a data-device write needs a recent one.
    latest_serial: Option<u32>,
    /// The newest left-button press serial, for a drag (P1).
    press_serial: Option<u32>,
    /// Offers being announced: their MIME types arrive before the event that uses them.
    announced: HashMap<ObjectId, Vec<String>>,
    /// A drag offer's source actions, which also arrive before its `enter`.
    source_actions: HashMap<ObjectId, u32>,
    selections: HashMap<(Selection, Route), Current>,
    drag: Option<Drag>,
    /// Dropped drag offers the app hasn't finished, closed at shutdown.
    dropped: Vec<DragOffer>,
    sources: HashMap<u64, Source>,
    events: Vec<Event>,
    /// Room for the state of protocols other modules bind on this queue (P4's
    /// `zxdg_importer_v2`): one value per type, which their own `Dispatch<_, _> for State`
    /// impls reach through [`State::extension`].
    extensions: HashMap<TypeId, Box<dyn Any + Send>>,
}

impl State {
    fn new(kind: Kind, home: Home, windows: Arc<Mutex<Vec<WindowHandle>>>) -> State {
        State {
            kind,
            home,
            windows,
            seat: None,
            compositor: None,
            shm: None,
            pointer: None,
            keyboard: None,
            ddm: None,
            device: None,
            control: None,
            latest_serial: None,
            press_serial: None,
            announced: HashMap::new(),
            source_actions: HashMap::new(),
            selections: HashMap::new(),
            drag: None,
            dropped: Vec::new(),
            sources: HashMap::new(),
            events: Vec::new(),
            extensions: HashMap::new(),
        }
    }

    /// The extension state of type `T`, made with `Default` on first use. For other modules'
    /// handlers on this queue; see [`Conn::with_globals`].
    #[expect(
        dead_code,
        reason = "the extension point for P1 and P4, which have no handlers yet"
    )]
    pub(crate) fn extension<T: Any + Send + Default>(&mut self) -> &mut T {
        self.extensions
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut::<T>()
            .unwrap_or_else(|| unreachable!("keyed by its own type"))
    }

    /// Replaces the current offer of a route, destroying the old one.
    fn set_selection_offer(
        &mut self,
        selection: Selection,
        route: Route,
        offer: Option<AnyOffer>,
        id: Option<ObjectId>,
    ) {
        if let Some(old) = self.selections.remove(&(selection, route)) {
            old.offer.close();
        }
        let offer = offer.map(|proxy| {
            let mimes = id
                .and_then(|id| self.announced.remove(&id))
                .unwrap_or_default();
            let offer = WlSelection::new(Some(proxy), mimes, self.home.clone());
            self.selections.insert(
                (selection, route),
                Current {
                    offer: offer.clone(),
                },
            );
            SelectionOffer(offer)
        });
        self.events.push(Event::SelectionChanged {
            selection,
            route,
            offer,
        });
    }

    /// A reader asked one of our sources for `mime`: its writer runs on a worker.
    fn source_send(&mut self, id: u64, mime: String, fd: std::os::fd::OwnedFd) {
        if let Some(writer) = self.sources.get(&id).and_then(|s| s.spec.writer(&mime)) {
            pipe::spawn_write(fd, writer.clone());
        }
        self.events.push(Event::SourceAsked {
            source: SourceId::from_raw(id),
            mime,
        });
    }

    fn source_cancelled(&mut self, id: u64) {
        if let Some(s) = self.sources.remove(&id) {
            s.proxy.destroy();
        }
        self.events.push(Event::SourceCancelled {
            source: SourceId::from_raw(id),
        });
    }

    /// The key of the app window whose surface is `surface`. Only the pointers are compared:
    /// the event's surface may be stale by now, so no request is ever sent on it.
    fn window_of(&self, surface: &WlSurface) -> Option<String> {
        let ptr = surface.id().as_ptr().cast::<c_void>();
        let windows = lock(&self.windows);
        windows
            .iter()
            .find(|w| {
                w.with_raw_handle(|h| matches!(h.as_raw(), RawWindowHandle::Wayland(s) if s.surface.as_ptr() == ptr))
                    .unwrap_or(false)
            })
            .map(|w| w.key().to_owned())
    }

    /// A drag that left without a drop is over: its offer goes.
    fn end_drag(&mut self) {
        if let Some(drag) = self.drag.take() {
            if drag.offer.with_state(|s| s.dropped()) {
                self.dropped.retain(DragOffer::is_open);
                self.dropped.push(drag.offer);
            } else {
                drag.offer.close();
            }
        }
    }
}

/// One Wayland connection with our queue on it.
pub(crate) struct Conn {
    /// With the reader's clone, the only strong references to the backend: [`Conn::shutdown`]
    /// drops it, on the UI thread, so a `Conn` dropped later (after `run` returns) makes no
    /// libwayland call. The backend's own drop destroys its queue, which needs the display.
    queue: Mutex<Option<EventQueue<State>>>,
    /// The connection, weakly, and the thread that may use it (the UI thread, when shared).
    home: Home,
    state: Mutex<State>,
    qh: QueueHandle<State>,
    /// Kept for the protocols other modules bind later ([`Conn::with_globals`]).
    globals: Mutex<Option<GlobalList>>,
    gate: Arc<Gate>,
    reader: Mutex<Option<Reader>>,
    subs: Subscribers,
    /// A dispatch failed: the connection is dead, and that was reported once.
    failed: AtomicBool,
}

impl Conn {
    /// Binds on winit's display, found from `window`'s surface. Call it on the UI thread.
    pub(crate) fn shared(
        window: &WindowHandle,
        windows: Arc<Mutex<Vec<WindowHandle>>>,
        subs: Subscribers,
        turn: Turn,
        data_control: Option<Prefer>,
        as_if_old: bool,
    ) -> Result<Arc<Conn>, Error> {
        let display = display::winit_display(window, as_if_old)?;
        let backend = display::foreign_backend(display);
        let conn = Connection::from_backend(backend);
        Conn::bind(Kind::Shared, conn, windows, subs, data_control, Some(turn))
    }

    /// Connects to the compositor on a connection of our own, for data control alone.
    pub(crate) fn own(subs: Subscribers, prefer: Prefer) -> Result<Arc<Conn>, Error> {
        let conn = Connection::connect_to_env().map_err(|e| Error::Connection(e.to_string()))?;
        Conn::bind(Kind::Own, conn, Arc::default(), subs, Some(prefer), None)
    }

    fn bind(
        kind: Kind,
        conn: Connection,
        windows: Arc<Mutex<Vec<WindowHandle>>>,
        subs: Subscribers,
        data_control: Option<Prefer>,
        turn: Option<Turn>,
    ) -> Result<Arc<Conn>, Error> {
        let (globals, mut queue) =
            registry_queue_init::<State>(&conn).map_err(|e| Error::Connection(e.to_string()))?;
        let qh = queue.handle();
        let home = Home {
            backend: conn.backend().downgrade(),
            ui: (kind == Kind::Shared).then(|| std::thread::current().id()),
        };
        let mut state = State::new(kind, home.clone(), windows);
        state.seat = globals.bind::<WlSeat, _, _>(&qh, 1..=8, ()).ok();
        if kind == Kind::Shared {
            state.compositor = globals.bind::<WlCompositor, _, _>(&qh, 1..=6, ()).ok();
            state.shm = globals.bind::<WlShm, _, _>(&qh, 1..=2, ()).ok();
            state.ddm = globals
                .bind::<WlDataDeviceManager, _, _>(&qh, 1..=3, ())
                .ok();
            if let (Some(ddm), Some(seat)) = (&state.ddm, &state.seat) {
                state.device = Some(ddm.get_data_device(seat, &qh, ()));
            }
        }
        if let (Some(prefer), Some(seat)) = (data_control, &state.seat) {
            state.control = Control::bind(&globals, &qh, seat, prefer);
        }
        if kind == Kind::Own && state.control.is_none() {
            return Err(Unsupported::NoProtocol("data control").into());
        }
        // Capabilities arrive in this round trip, and the seat handler binds the pointer and
        // keyboard only once the seat has them: `get_pointer` on a seat without a pointer is a
        // protocol error.
        queue
            .roundtrip(&mut state)
            .map_err(|e| Error::Connection(e.to_string()))?;
        let gate = Arc::new(Gate::default());
        let c = Arc::new(Conn {
            queue: Mutex::new(Some(queue)),
            home,
            globals: Mutex::new(Some(globals)),
            failed: AtomicBool::new(false),
            state: Mutex::new(state),
            qh,
            gate: gate.clone(),
            reader: Mutex::new(None),
            subs: subs.clone(),
        });
        let weak = Arc::downgrade(&c);
        let failed = {
            let subs = subs.clone();
            move |message: String| subs.emit(vec![Event::Failed { message }])
        };
        let reader = match turn {
            Some(turn) => {
                let gate = gate.clone();
                Reader::spawn(
                    "transfer-wayland",
                    conn,
                    gate.clone(),
                    move || {
                        // Ask the UI thread to dispatch, then wait for it with no read intent held.
                        let before = gate.turns();
                        let weak: Weak<Conn> = weak.clone();
                        turn(Box::new(move || {
                            // On the UI thread, where Turn runs it; refused anywhere else.
                            if let Some(c) = weak.upgrade() {
                                let _ = c.dispatch();
                            }
                        }));
                        gate.wait(before);
                    },
                    failed,
                )
            }
            None => Reader::spawn(
                "transfer-wayland-own",
                conn,
                gate,
                move || {
                    if let Some(c) = weak.upgrade() {
                        c.dispatch_here();
                    }
                },
                failed,
            ),
        }
        .map_err(|e| Error::Connection(e.to_string()))?;
        *lock(&c.reader) = Some(reader);
        Ok(c)
    }

    pub(crate) fn data_control(&self) -> Option<&'static str> {
        lock(&self.state).control.as_ref().map(Control::name)
    }

    pub(crate) fn has_device(&self) -> bool {
        lock(&self.state).device.is_some()
    }

    /// Runs the queued events' handlers and hands their events to subscribers. Shared: the
    /// UI thread only, from the reader's turn or before reading a serial.
    pub(crate) fn dispatch(&self) -> Result<bool, Error> {
        // Handlers call `WindowHandle::with_raw_handle` and parse events naming winit's
        // objects: both only on the UI thread (see `reader`).
        if !self.home.here() {
            return Err(Error::Refused(ON_UI));
        }
        let any = self.dispatch_here();
        self.gate.turned();
        Ok(any)
    }

    fn dispatch_here(&self) -> bool {
        if self.failed.load(Ordering::Acquire) {
            return false;
        }
        let events = {
            let mut queue = lock(&self.queue);
            let Some(queue) = queue.as_mut() else {
                return false;
            };
            let mut state = lock(&self.state);
            if let Err(e) = queue.dispatch_pending(&mut state) {
                // The connection is dead (a protocol error, a closed socket): say so once, and
                // dispatch no more.
                self.failed.store(true, Ordering::Release);
                state.events.push(Event::Failed {
                    message: e.to_string(),
                });
            }
            let _ = queue.flush();
            std::mem::take(&mut state.events)
        };
        let any = !events.is_empty();
        self.subs.emit(events);
        any
    }

    /// Runs `f` with the registry's globals and the queue, to bind a protocol P0 doesn't (P4's
    /// `zxdg_importer_v2`). The module binding it implements `Dispatch<I, U> for State` for its
    /// objects and keeps its state in [`State::extension`]. None after shutdown, or off the
    /// UI thread on winit's display.
    #[expect(
        dead_code,
        reason = "the extension point for P1 and P4, which have no handlers yet"
    )]
    pub(crate) fn with_globals<R>(
        &self,
        f: impl FnOnce(&GlobalList, &QueueHandle<State>, &mut State) -> R,
    ) -> Option<R> {
        if !self.home.here() {
            return None;
        }
        let globals = lock(&self.globals);
        let mut state = lock(&self.state);
        Some(f(globals.as_ref()?, &self.qh, &mut state))
    }

    pub(crate) fn serials(&self) -> (Option<u32>, Option<u32>) {
        let state = lock(&self.state);
        (state.latest_serial, state.press_serial)
    }

    pub(crate) fn selection(&self, selection: Selection, route: Route) -> Option<SelectionOffer> {
        lock(&self.state)
            .selections
            .get(&(selection, route))
            .map(|c| SelectionOffer(c.offer.clone()))
    }

    pub(crate) fn owns(&self, source: SourceId) -> bool {
        lock(&self.state).sources.contains_key(&source.get())
    }

    /// Offers `spec` as `selection` on `route`. A data-device write uses the newest input
    /// serial: call [`Conn::dispatch`] first so the event that triggered the copy is in.
    pub(crate) fn set_selection(
        &self,
        selection: Selection,
        route: Route,
        spec: SourceSpec,
    ) -> Result<SourceId, Error> {
        if spec.is_empty() {
            return Err(Error::Refused("a source with no types"));
        }
        if !self.home.here() {
            return Err(Error::Refused(ON_UI));
        }
        let id = SourceId::next();
        let mut state = lock(&self.state);
        let proxy = match route {
            Route::DataDevice => {
                if selection != Selection::Clipboard {
                    return Err(Error::Refused("the data device has no primary selection"));
                }
                let (Some(ddm), Some(device)) = (&state.ddm, &state.device) else {
                    return Err(Error::NoRoute("data device"));
                };
                let serial = state.latest_serial.ok_or(Error::NoSerial)?;
                let source = ddm.create_data_source(&self.qh, id.get());
                for mime in spec.mimes() {
                    source.offer(mime.to_owned());
                }
                // No set_actions: on a selection source it is a protocol error.
                device.set_selection(Some(&source), serial);
                SourceProxy::Device(source)
            }
            Route::DataControl => {
                let control = state
                    .control
                    .as_ref()
                    .ok_or(Error::NoRoute("data control"))?;
                SourceProxy::Control(control.set_selection(&self.qh, id.get(), selection, &spec)?)
            }
            Route::X11 => return Err(Error::NoRoute("X11")),
        };
        state.sources.insert(id.get(), Source { spec, proxy });
        drop(state);
        self.home.flush();
        Ok(id)
    }

    /// Stops the reader, then destroys every object, on the UI thread for a shared
    /// connection, before winit closes the display (S2, step 6).
    pub(crate) fn shutdown(&self) {
        if let Some(reader) = lock(&self.reader).take() {
            reader.stop();
        }
        let mut state = lock(&self.state);
        for (_, current) in state.selections.drain() {
            current.offer.close();
        }
        if let Some(drag) = state.drag.take() {
            drag.offer.close();
        }
        for offer in state.dropped.drain(..) {
            offer.close();
        }
        for (_, source) in state.sources.drain() {
            source.proxy.destroy();
        }
        if let Some(control) = state.control.take() {
            control.destroy();
        }
        if let Some(device) = state.device.take()
            && device.version() >= 2
        {
            device.release();
        }
        if let Some(p) = state.pointer.take()
            && p.version() >= 3
        {
            p.release();
        }
        if let Some(k) = state.keyboard.take()
            && k.version() >= 3
        {
            k.release();
        }
        if let Some(seat) = state.seat.take()
            && seat.version() >= 5
        {
            seat.release();
        }
        if let Some(shm) = state.shm.take()
            && shm.version() >= 2
        {
            shm.release();
        }
        // wl_compositor has no destructor.
        state.compositor = None;
        state.extensions.clear();
        drop(state);
        drop(lock(&self.globals).take());
        self.home.flush();
        // The last strong references to the backend go now, while the display lives.
        drop(lock(&self.queue).take());
    }
}

// ---- handlers ----

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlDataDeviceManager, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: <WlDataDeviceManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlCompositor,
        _: <WlCompositor as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// `format` events list the pixel formats; P1's icons use ARGB8888, which every `wl_shm` has.
impl Dispatch<WlShm, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: <WlShm as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSurface, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlSurface,
        _: <WlSurface as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        st: &mut Self,
        seat: &WlSeat,
        e: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = e
        else {
            return;
        };
        // Serials only matter on winit's display: they come from input to the app's windows.
        if st.kind != Kind::Shared {
            return;
        }
        if caps.contains(wl_seat::Capability::Pointer) && st.pointer.is_none() {
            st.pointer = Some(seat.get_pointer(qh, ()));
        }
        // One keyboard per display: each bind makes the compositor send every keyboard of
        // the client its keymap again, winit's too.
        if caps.contains(wl_seat::Capability::Keyboard) && st.keyboard.is_none() {
            st.keyboard = Some(seat.get_keyboard(qh, ()));
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        st: &mut Self,
        _: &WlPointer,
        e: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            serial,
            button,
            state,
            ..
        } = e
        {
            st.latest_serial = Some(serial);
            if button == BTN_LEFT && matches!(state, WEnum::Value(wl_pointer::ButtonState::Pressed))
            {
                st.press_serial = Some(serial);
            }
        }
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        st: &mut Self,
        _: &WlKeyboard,
        e: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            // Dropping the fd closes it.
            wl_keyboard::Event::Keymap { fd, .. } => drop(fd),
            wl_keyboard::Event::Key { serial, .. } => st.latest_serial = Some(serial),
            _ => {}
        }
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    event_created_child!(State, WlDataDevice, [wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ())]);

    fn event(
        st: &mut Self,
        _: &WlDataDevice,
        e: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_data_device::Event::DataOffer { id } => {
                st.announced.insert(id.id(), Vec::new());
            }
            wl_data_device::Event::Enter {
                serial,
                surface,
                x,
                y,
                id,
            } => {
                st.end_drag();
                let Some(proxy) = id else {
                    return;
                };
                let mimes = st.announced.remove(&proxy.id()).unwrap_or_default();
                let actions = st.source_actions.remove(&proxy.id());
                // Anything else announced for an earlier drag is stale now.
                st.source_actions.clear();
                let offer = DragOffer::new(proxy, mimes, serial, st.home.clone());
                let window = st.window_of(&surface);
                if let Some(window) = &window {
                    st.events.push(Event::DragEnter {
                        window: window.clone(),
                        offer: offer.clone(),
                        x,
                        y,
                    });
                    if let Some(a) = actions {
                        st.events.push(Event::DragActions {
                            source: Action::all_in(a),
                            chosen: None,
                        });
                    }
                }
                st.drag = Some(Drag { offer, window });
            }
            wl_data_device::Event::Motion { x, y, .. } => {
                if st.drag.as_ref().is_some_and(|d| d.window.is_some()) {
                    st.events.push(Event::DragMotion { x, y });
                }
            }
            wl_data_device::Event::Leave => {
                if st.drag.as_ref().is_some_and(|d| d.window.is_some()) {
                    st.events.push(Event::DragLeave);
                }
                // A dropped offer lives on until the app finishes it: the compositor sends
                // leave right after the drop.
                st.end_drag();
            }
            wl_data_device::Event::Drop => {
                if let Some(drag) = &st.drag {
                    drag.offer.with_state(|s| s.on_drop());
                    if drag.window.is_some() {
                        st.events.push(Event::Drop);
                    }
                }
            }
            wl_data_device::Event::Selection { id } => {
                let oid = id.as_ref().map(Proxy::id);
                st.set_selection_offer(
                    Selection::Clipboard,
                    Route::DataDevice,
                    id.map(AnyOffer::Device),
                    oid,
                );
            }
            _ => {}
        }
    }
}

impl Dispatch<WlDataOffer, ()> for State {
    fn event(
        st: &mut Self,
        o: &WlDataOffer,
        e: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = o.id();
        let drag = st.drag.as_ref().filter(|d| d.offer.is(&id));
        match e {
            wl_data_offer::Event::Offer { mime_type } => {
                if let Some(mimes) = st.announced.get_mut(&id) {
                    mimes.push(mime_type);
                }
            }
            wl_data_offer::Event::SourceActions {
                source_actions: WEnum::Value(a),
            } => match drag {
                Some(d) if d.window.is_some() => st.events.push(Event::DragActions {
                    source: Action::all_in(a.bits()),
                    chosen: None,
                }),
                Some(_) => {}
                // Not entered yet: told at enter.
                None => {
                    st.source_actions.insert(id, a.bits());
                }
            },
            wl_data_offer::Event::Action {
                dnd_action: WEnum::Value(a),
            } => {
                if let Some(d) = drag {
                    d.offer.with_state(|s| s.on_action(a.bits()));
                    if d.window.is_some() {
                        st.events.push(Event::DragActions {
                            source: Vec::new(),
                            chosen: Action::from_bits(a.bits()),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlDataSource, u64> for State {
    fn event(
        st: &mut Self,
        _: &WlDataSource,
        e: wl_data_source::Event,
        id: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_data_source::Event::Send { mime_type, fd } => st.source_send(*id, mime_type, fd),
            wl_data_source::Event::Cancelled => st.source_cancelled(*id),
            _ => {}
        }
    }
}
