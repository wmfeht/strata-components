//! Headless tests of the decorations on a `rune::Runtime`: where presses go, what the header
//! keeps of Rune's drag region, and where each window button's request goes.
//!
//! Feature map: `app/window`, "dragging … header moves the window and dragging a window edge
//! resizes it", "Maximize maximizes the window …", "Minimize minimizes the window and Close
//! closes it", and the double-click probe.

use rune::{Cx, El, Env, PointerPress, Runtime, TitlebarAction, Tx, col, el, row};

use super::frame::pick;
use super::*;

const SIZE: (f32, f32) = (400.0, 300.0);

struct State {
    csd: Csd,
    button_presses: u32,
    field_menus: u32,
}

fn access(s: &mut State) -> &mut Csd {
    &mut s.csd
}

/// A header with a button and a field that has its own menu, over a body.
fn view(s: &State, _: &mut Cx<State>) -> El {
    let bar =
        row()
            .key("bar")
            .w_full()
            .h(40.0)
            .child(
                el().key("button")
                    .size(40.0, 40.0)
                    .role("button")
                    .label("Button")
                    .on_press(|s: &mut State| s.button_presses += 1),
            )
            .child(el().key("field").size(80.0, 40.0).on_secondary_press(
                |s: &mut State, _: PointerPress, _: &mut Tx| s.field_menus += 1,
            ));
    let content = col()
        .w_full()
        .h_full()
        .child(header(bar, &s.csd, access))
        .child(el().grow(1.0));
    decorate(content, &s.csd, access)
}

fn runtime(csd: Csd) -> Runtime<State> {
    let state = State {
        csd,
        button_presses: 0,
        field_menus: 0,
    };
    let mut rt = Runtime::new(state, view, SIZE, Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn borderless(backend: Backend) -> Csd {
    Csd::new(Frame::Borderless, backend)
}

/// A press and release at (x, y), 20 ms after the last frame. Returns the titlebar action the
/// press asked the shell for.
fn click(rt: &mut Runtime<State>, (x, y): (f32, f32)) -> Option<TitlebarAction> {
    rt.frame(0.01);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    let action = rt.take_titlebar_action();
    rt.pointer_up(x, y);
    rt.frame(0.01);
    action
}

fn secondary_click(rt: &mut Runtime<State>, (x, y): (f32, f32)) {
    rt.frame(0.01);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(0.01);
}

/// A report from outside any handler, as `AppHandle::update` delivers it.
fn deliver(rt: &mut Runtime<State>, report: Report) {
    rt.update(move |s, _| s.csd.apply(report));
    rt.frame(0.01);
}

/// A new `Csd` in place of the old, as `attach` makes for a re-created window.
fn replace(rt: &mut Runtime<State>, csd: Csd) {
    rt.update(move |s, _| s.csd = csd);
    rt.frame(0.01);
}

/// A point inside each edge's band in a 400 × 300 window.
fn inside(edge: Edge) -> (f32, f32) {
    let (w, h) = SIZE;
    match edge {
        Edge::North => (w / 2.0, 2.0),
        Edge::NorthEast => (w - 4.0, 4.0),
        Edge::East => (w - 2.0, h / 2.0),
        Edge::SouthEast => (w - 4.0, h - 4.0),
        Edge::South => (w / 2.0, h - 2.0),
        Edge::SouthWest => (4.0, h - 4.0),
        Edge::West => (2.0, h / 2.0),
        Edge::NorthWest => (4.0, 4.0),
    }
}

#[test]
fn a_press_on_each_band_asks_for_a_resize_that_way() {
    let mut rt = runtime(borderless(Backend::Headless));
    for edge in Edge::ALL {
        let (x, y) = inside(edge);
        rt.pointer_move(x, y);
        assert_eq!(
            rt.cursor(),
            edge.cursor(),
            "the cursor over the {edge:?} band"
        );
        let action = click(&mut rt, (x, y));
        assert_eq!(action, None, "the {edge:?} band doesn't move the window");
        assert_eq!(
            rt.state.csd.last_outcome(),
            Some(&Outcome::Sent {
                request: Request::Resize(edge),
                route: Route::Headless
            }),
        );
    }
}

#[test]
fn the_header_moves_the_window_and_a_double_press_maximises_it() {
    let mut rt = runtime(borderless(Backend::Headless));
    let bare = (300.0, 20.0);
    assert_eq!(click(&mut rt, bare), Some(TitlebarAction::Drag));
    assert_eq!(click(&mut rt, bare), Some(TitlebarAction::DoubleClick));
    rt.frame(1.0);
    assert_eq!(click(&mut rt, bare), Some(TitlebarAction::Drag));
    // Rune pairs presses within its fixed 0.5 s (draft U13, not filed).
    rt.frame(0.6);
    assert_eq!(
        click(&mut rt, bare),
        Some(TitlebarAction::Drag),
        "too slow to pair"
    );

    assert_eq!(click(&mut rt, (20.0, 20.0)), None, "the header's button");
    assert_eq!(rt.state.button_presses, 1, "keeps its press");
    assert_eq!(rt.state.csd.last_outcome(), None);
}

#[test]
fn a_secondary_click_on_the_header_opens_the_window_menu_only_where_there_is_one() {
    let mut rt = runtime(borderless(Backend::Headless));
    secondary_click(&mut rt, (300.0, 20.0));
    assert_eq!(
        rt.state.csd.last_outcome(),
        None,
        "the window manager's menu is unknown"
    );

    let mut report = Report::default();
    report.capabilities.window_menu = Some(true);
    deliver(&mut rt, report);
    secondary_click(&mut rt, (300.0, 20.0));
    let menu = Some(Outcome::Sent {
        request: Request::WindowMenu,
        route: Route::Headless,
    });
    assert_eq!(rt.state.csd.last_outcome().cloned(), menu);

    // The header's button has no menu of its own, so the window menu takes it, as GTK's header
    // bar does; the field keeps its own.
    replace(&mut rt, borderless(Backend::Headless));
    deliver(&mut rt, report);
    secondary_click(&mut rt, (20.0, 20.0));
    assert_eq!(rt.state.csd.last_outcome().cloned(), menu);
    replace(&mut rt, borderless(Backend::Headless));
    deliver(&mut rt, report);
    secondary_click(&mut rt, (80.0, 20.0));
    assert_eq!(rt.state.field_menus, 1);
    assert_eq!(rt.state.csd.last_outcome(), None);
}

#[test]
fn bands_are_mounted_only_where_they_can_resize() {
    let maximized = Report {
        state: WindowState {
            maximized: Some(true),
            fullscreen: Some(false),
        },
        ..Report::default()
    };
    let fullscreen = Report {
        state: WindowState {
            maximized: Some(false),
            fullscreen: Some(true),
        },
        ..Report::default()
    };
    let cases = [
        (
            "a native frame",
            Csd::new(Frame::Native, Backend::X11),
            None,
        ),
        (
            "borderless on Wayland (U6)",
            borderless(Backend::Wayland),
            None,
        ),
        ("maximised", borderless(Backend::Headless), Some(maximized)),
        (
            "full screen",
            borderless(Backend::Headless),
            Some(fullscreen),
        ),
    ];
    for (what, csd, report) in cases {
        let mut rt = runtime(csd);
        if let Some(report) = report {
            deliver(&mut rt, report);
        }
        let (x, y) = inside(Edge::East);
        rt.pointer_move(x, y);
        assert_eq!(
            rt.cursor(),
            rune::Cursor::Default,
            "{what}: no resize cursor"
        );
        click(&mut rt, (x, y));
        assert_eq!(
            rt.state.csd.last_outcome(),
            None,
            "{what}: the edge asks for nothing"
        );
    }

    // Restored, the bands come back.
    let mut rt = runtime(borderless(Backend::Headless));
    deliver(&mut rt, maximized);
    deliver(&mut rt, Report::default());
    click(&mut rt, inside(Edge::East));
    assert_eq!(
        rt.state.csd.last_outcome().map(Outcome::request),
        Some(Request::Resize(Edge::East))
    );
}

#[test]
fn the_bands_are_silent_to_assistive_technology() {
    let rt = runtime(borderless(Backend::Headless));
    assert!(rt.state.csd.shows_bands());
    let tree = rt.accessibility_json().to_string();
    assert!(
        tree.contains("\"button\""),
        "the tree holds the header's button: {tree}"
    );
    assert!(!tree.contains("csd-edge"), "no band is a node: {tree}");
}

/// Each request on each frame and backend: through Rune, through X11, or the gap that stops it.
#[test]
fn each_request_goes_where_it_can_work() {
    use Gap::*;
    let rune = |request| Outcome::Sent {
        request,
        route: Route::Rune,
    };
    let gap = |request, gap| Outcome::Unsupported { request, gap };
    let run = |csd: &mut Csd| {
        let mut tx = Tx::default();
        [
            csd.close(&mut tx),
            csd.toggle_maximized(&mut tx),
            csd.minimize(&mut tx),
            csd.begin_resize(Edge::East, None),
            csd.window_menu(),
        ]
    };
    use Request::{Close, Minimize, Resize, ToggleMaximize, WindowMenu};
    let resize = Resize(Edge::East);

    let mut native = Csd::new(Frame::Native, Backend::Wayland);
    assert_eq!(
        run(&mut native),
        [
            rune(Close),
            rune(ToggleMaximize),
            rune(Minimize),
            gap(resize, NativeFrame),
            gap(WindowMenu, NativeFrame),
        ]
    );

    let mut wayland = borderless(Backend::Wayland);
    assert_eq!(
        run(&mut wayland),
        [
            rune(Close),
            rune(ToggleMaximize),
            gap(Minimize, RuneMinimize),
            gap(resize, RuneResize),
            gap(WindowMenu, RuneWindowMenu),
        ]
    );
    let caps = wayland.capabilities();
    assert_eq!(
        (caps.resize, caps.minimize, caps.window_menu),
        (Some(false), Some(false), Some(false)),
        "so the app can hide what can't work"
    );
    assert_eq!(RuneResize.rune_draft(), Some("U6"));
    assert_eq!(RuneMinimize.rune_draft(), Some("U7"));
    assert_eq!(RuneWindowMenu.rune_draft(), Some("U8"));

    // X11 before `attach` links a window, and a window manager that lists no window menu.
    let mut x11 = borderless(Backend::X11);
    assert_eq!(
        run(&mut x11),
        [
            rune(Close),
            rune(ToggleMaximize),
            gap(Minimize, NoWindow),
            gap(resize, NoWindow),
            gap(WindowMenu, WindowManager),
        ]
    );
}

#[test]
fn an_observer_hears_each_outcome_and_report() {
    use std::cell::RefCell;
    thread_local! {
        static HEARD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }
    fn hear(csd: &Csd, notice: Notice<'_>) {
        let line = match notice {
            Notice::Attached => "attached".to_owned(),
            Notice::Report => format!("report {:?}", csd.state().maximized),
            Notice::Outcome(o) => format!("outcome {:?}", o.request()),
        };
        HEARD.with(|h| h.borrow_mut().push(line));
    }
    let mut csd = borderless(Backend::Headless).observe(hear);
    csd.minimize(&mut Tx::default());
    let mut report = Report::default();
    report.state.maximized = Some(true);
    csd.apply(report);
    csd.apply(report);
    assert_eq!(
        HEARD.with(|h| h.borrow().clone()),
        ["outcome Minimize", "report Some(true)"],
        "a report that changes nothing isn't heard"
    );
}

#[test]
fn auto_is_borderless_and_the_backend_is_the_one_winit_picks() {
    assert_eq!(Policy::Auto.frame(), Frame::Borderless);

    assert_eq!(
        pick(Some("wayland-1"), None, Some(":0")),
        Backend::Wayland,
        "Wayland first"
    );
    assert_eq!(pick(None, Some("5"), None), Backend::Wayland);
    assert_eq!(
        pick(Some(""), Some(""), Some(":0")),
        Backend::X11,
        "empty is unset"
    );
    assert_eq!(pick(None, None, Some("")), Backend::Headless);
}
