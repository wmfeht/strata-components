//! [`Csd`]: what the app keeps in its state, and where each window button's request goes.

use rune::Tx;

use super::x11::Link;
use super::{Backend, Bands, Edge, Frame, Gap, Outcome, Request, Route};

/// The window's state, where it is known. `None` is unknown: Rune reports neither on Wayland
/// (draft U6, not filed), and X11 knows once the watcher has read the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowState {
    pub maximized: Option<bool>,
    pub fullscreen: Option<bool>,
}

/// Whether the app's own control for each action can work: `Some(false)` hides it, `None` is
/// unknown (the request goes out, and the window manager may ignore it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// Resizing from the bands.
    pub resize: Option<bool>,
    pub minimize: Option<bool>,
    pub maximize: Option<bool>,
    /// The window menu from the header.
    pub window_menu: Option<bool>,
}

/// What the X11 watcher reads: the window's `_NET_WM_STATE`, and its window manager's
/// `_NET_SUPPORTED` and `_NET_WM_ALLOWED_ACTIONS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub state: WindowState,
    pub capabilities: Capabilities,
}

/// What an observer ([`Csd::observe`]) hears.
#[derive(Debug)]
pub enum Notice<'a> {
    /// [`attach`](super::attach) linked a native window.
    Attached,
    /// A new [`Report`] from the X11 watcher.
    Report,
    /// A request's outcome.
    Outcome(&'a Outcome),
}

/// Client-side decorations for one window: its frame, its backend and what is known about it.
/// The app keeps one in its state per window and passes an accessor to [`decorate`](super::decorate)
/// and [`header`](super::header).
pub struct Csd {
    frame: Frame,
    backend: Backend,
    bands: Bands,
    report: Report,
    link: Option<Link>,
    last: Option<Outcome>,
    revision: u64,
    observer: Option<fn(&Csd, Notice<'_>)>,
}

impl std::fmt::Debug for Csd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Csd")
            .field("frame", &self.frame)
            .field("backend", &self.backend)
            .field("report", &self.report)
            .field("linked", &self.link.is_some())
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

impl Csd {
    /// A window that opens with `frame` on `backend` ([`Backend::from_env`] before the window
    /// exists; [`attach`](super::attach) confirms it).
    pub fn new(frame: Frame, backend: Backend) -> Csd {
        Csd {
            frame,
            backend,
            bands: Bands::default(),
            report: Report {
                state: WindowState::default(),
                capabilities: defaults(frame, backend),
            },
            link: None,
            last: None,
            revision: 0,
            observer: None,
        }
    }

    /// Bands of another width.
    pub fn with_bands(mut self, bands: Bands) -> Csd {
        self.bands = bands;
        self
    }

    /// Calls `f` on each [`Notice`], after it took effect: for logging, or to follow outcomes.
    pub fn observe(mut self, f: fn(&Csd, Notice<'_>)) -> Csd {
        self.observer = Some(f);
        self
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn backend(&self) -> Backend {
        self.backend
    }

    pub fn bands(&self) -> Bands {
        self.bands
    }

    pub fn state(&self) -> WindowState {
        self.report.state
    }

    pub fn capabilities(&self) -> Capabilities {
        self.report.capabilities
    }

    /// The last request's outcome.
    pub fn last_outcome(&self) -> Option<&Outcome> {
        self.last.as_ref()
    }

    /// Goes up by one each time the backend, state or capabilities change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether [`decorate`](super::decorate) mounts the resize bands: a borderless window
    /// whose bands can resize (not on Wayland, U6), that isn't maximised or full screen.
    pub fn shows_bands(&self) -> bool {
        let state = self.report.state;
        self.frame == Frame::Borderless
            && self.report.capabilities.resize != Some(false)
            && state.maximized != Some(true)
            && state.fullscreen != Some(true)
    }

    /// Whether [`header`](super::header) opens the window menu on a secondary click: only
    /// where the window manager says it has one.
    pub fn shows_window_menu(&self) -> bool {
        self.frame == Frame::Borderless && self.report.capabilities.window_menu == Some(true)
    }

    /// Takes the X11 watcher's report. Apps that start their own watcher call this; [`attach`]
    /// wires it for them.
    ///
    /// [`attach`]: super::attach
    pub fn apply(&mut self, report: Report) {
        if report != self.report {
            self.report = report;
            self.revision += 1;
            self.notify(Notice::Report);
        }
    }

    /// Links the native window [`attach`](super::attach) found.
    pub(super) fn attached(&mut self, backend: Backend, link: Option<Link>) {
        self.backend = backend;
        self.link = link;
        self.report = Report {
            state: WindowState::default(),
            capabilities: defaults(self.frame, backend),
        };
        self.revision += 1;
        self.notify(Notice::Attached);
    }

    /// The Close button.
    pub fn close(&mut self, tx: &mut Tx) -> Outcome {
        tx.close_window();
        self.done(sent(Request::Close, Route::Rune))
    }

    /// The Maximise or Restore button. Rune maximises or restores a borderless window too.
    pub fn toggle_maximized(&mut self, tx: &mut Tx) -> Outcome {
        tx.toggle_maximized();
        self.done(sent(Request::ToggleMaximize, Route::Rune))
    }

    /// The Minimise button. Rune minimises a window with a native frame and ignores a
    /// borderless one (U7), so on X11 this module asks the window manager itself.
    pub fn minimize(&mut self, tx: &mut Tx) -> Outcome {
        let request = Request::Minimize;
        let outcome = match (self.frame, self.backend) {
            (Frame::Native, _) => {
                tx.minimize();
                sent(request, Route::Rune)
            }
            (Frame::Borderless, Backend::Wayland) => unsupported(request, Gap::RuneMinimize),
            (Frame::Borderless, Backend::X11) => self.x11(request, |link| link.minimize()),
            (Frame::Borderless, Backend::Headless) => sent(request, Route::Headless),
        };
        self.done(outcome)
    }

    /// Starts an interactive resize from `edge`. Call it while the button is still down (a
    /// band's `on_drag` `Phase::Down`). `screen` is the pointer in desktop pixels
    /// (`Drag::screen_point`), else the X server is asked.
    pub fn begin_resize(&mut self, edge: Edge, screen: Option<(i32, i32)>) -> Outcome {
        let request = Request::Resize(edge);
        let outcome = match (self.frame, self.backend) {
            (Frame::Native, _) => unsupported(request, Gap::NativeFrame),
            (Frame::Borderless, Backend::Wayland) => unsupported(request, Gap::RuneResize),
            (Frame::Borderless, Backend::X11) => {
                self.x11(request, |link| link.begin_resize(edge, screen))
            }
            (Frame::Borderless, Backend::Headless) => sent(request, Route::Headless),
        };
        self.done(outcome)
    }

    /// Opens the window manager's window menu at the pointer.
    pub fn window_menu(&mut self) -> Outcome {
        let request = Request::WindowMenu;
        let outcome = match (self.frame, self.backend) {
            (Frame::Native, _) => unsupported(request, Gap::NativeFrame),
            (Frame::Borderless, Backend::Wayland) => unsupported(request, Gap::RuneWindowMenu),
            (Frame::Borderless, Backend::X11) if !self.shows_window_menu() => {
                unsupported(request, Gap::WindowManager)
            }
            (Frame::Borderless, Backend::X11) => self.x11(request, |link| link.window_menu()),
            (Frame::Borderless, Backend::Headless) => sent(request, Route::Headless),
        };
        self.done(outcome)
    }

    fn x11(&self, request: Request, send: impl FnOnce(&Link) -> Result<(), String>) -> Outcome {
        match &self.link {
            None => unsupported(request, Gap::NoWindow),
            Some(link) => match send(link) {
                Ok(()) => sent(request, Route::X11),
                Err(error) => Outcome::Failed { request, error },
            },
        }
    }

    fn done(&mut self, outcome: Outcome) -> Outcome {
        self.last = Some(outcome.clone());
        self.notify(Notice::Outcome(&outcome));
        outcome
    }

    fn notify(&self, notice: Notice<'_>) {
        if let Some(f) = self.observer {
            f(self, notice);
        }
    }
}

/// What can work before anything is read from the window manager.
fn defaults(frame: Frame, backend: Backend) -> Capabilities {
    match (frame, backend) {
        // The native frame resizes and has its own menu; the app's buttons go through Rune.
        (Frame::Native, _) => Capabilities {
            resize: Some(false),
            window_menu: Some(false),
            ..Capabilities::default()
        },
        // U6, U7, U8.
        (Frame::Borderless, Backend::Wayland) => Capabilities {
            resize: Some(false),
            minimize: Some(false),
            maximize: None,
            window_menu: Some(false),
        },
        // The watcher fills these in.
        (Frame::Borderless, Backend::X11 | Backend::Headless) => Capabilities::default(),
    }
}

fn sent(request: Request, route: Route) -> Outcome {
    Outcome::Sent { request, route }
}

fn unsupported(request: Request, gap: Gap) -> Outcome {
    Outcome::Unsupported { request, gap }
}
