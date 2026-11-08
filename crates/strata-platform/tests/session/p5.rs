//! P5's session tests: the `settings` example against a fake Settings portal on the session's
//! private bus (`p5/fake_portal.rs`), served from this test process. Most need only the bus;
//! the last two open the example's window under headless sway, to check that changes reach
//! Rune's loop, also while another window covers it (upstream #139: a covered Wayland
//! window's `poll` and timers stop).
//!
//! The fake connects with `Session::bus().address()` only. The no-bus test points
//! `DBUS_SESSION_BUS_ADDRESS` at a socket that doesn't exist inside the session's own runtime
//! directory, rather than unsetting it, so nothing can fall back to another bus.
//!
//! Feature map: `settings/themes/portal-text-scaling` ("changing `text-scaling-factor` while
//! the window is open rescales it without a restart", as far as the change reaching the app;
//! "only `text-scaling-factor` in `org.gnome.desktop.interface` with a floating-point value is
//! applied"; "a factor of 0, below 0, NaN, or infinite leaves it unchanged"; "without a session
//! bus, or when `ReadAll` fails, the value stays and a log records it"), and the double-click
//! interval and animations switch that `browser/selection/click-modes` and the motion bullets
//! read, as data.
//!
//! Work package: P5, `docs/work/phase-1-leaves/P5-desktop-settings.md`.

mod fake_portal;

use std::time::{Duration, Instant};

use serde_json::Value;
use zbus::Variant;

use crate::support::{Display, Options, Proc, Session, example, timeout};
use fake_portal::FakePortal;

const INTERFACE: &str = "org.gnome.desktop.interface";
const MOUSE: &str = "org.gnome.desktop.peripherals.mouse";
const PRIVACY: &str = "org.gnome.desktop.privacy";
const APPEARANCE: &str = "org.freedesktop.appearance";

const ALL_KEYS: [&str; 9] = [
    "text-scaling-factor",
    "double-click",
    "drag-threshold",
    "cursor-blink",
    "cursor-blink-time",
    "cursor-blink-timeout",
    "enable-animations",
    "reduced-motion",
    "remember-recent-files",
];

/// Every key, each away from its default, plus a key the module doesn't read.
fn desktop() -> Vec<(&'static str, &'static str, Variant<'static>)> {
    vec![
        (INTERFACE, "text-scaling-factor", Variant::F64(1.25)),
        (MOUSE, "double-click", Variant::I32(1200)),
        (MOUSE, "drag-threshold", Variant::I32(12)),
        (INTERFACE, "cursor-blink", Variant::Bool(false)),
        (INTERFACE, "cursor-blink-time", Variant::I32(900)),
        (INTERFACE, "cursor-blink-timeout", Variant::I32(4)),
        (INTERFACE, "enable-animations", Variant::Bool(false)),
        (APPEARANCE, "reduced-motion", Variant::U32(1)),
        (PRIVACY, "remember-recent-files", Variant::Bool(false)),
        (INTERFACE, "font-name", Variant::from("Sans 11")),
    ]
}

fn bus_only() -> Session {
    Session::with(Options {
        compositor: false,
        ..Options::default()
    })
}

/// The example without a window, logging its events.
fn watch(s: &Session) -> Proc {
    let mut cmd = s.command(example("settings"), Display::None);
    cmd.arg("--no-window");
    let app = s.spawn("settings", cmd);
    app.wait_json("the watcher started", |v| v["ev"] == "started");
    app
}

fn snapshot(app: &Proc) -> Value {
    app.wait_json("the snapshot", |v| v["ev"] == "snapshot")["settings"].clone()
}

fn known(settings: &Value) -> Vec<String> {
    settings["known"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn assert_defaults(settings: &Value) {
    assert_eq!(settings["text_scale"], 1.0, "{settings}");
    assert_eq!(settings["double_click_ms"], 400, "{settings}");
    assert_eq!(settings["drag_threshold"], 8, "{settings}");
    assert_eq!(settings["cursor_blink"], true, "{settings}");
    assert_eq!(settings["cursor_blink_time_ms"], 1200, "{settings}");
    assert_eq!(settings["cursor_blink_timeout_s"], 10, "{settings}");
    assert_eq!(settings["enable_animations"], true, "{settings}");
    assert_eq!(settings["reduced_motion"], false, "{settings}");
    assert_eq!(settings["remember_recent_files"], true, "{settings}");
    assert!(known(settings).is_empty(), "{settings}");
}

fn assert_desktop(settings: &Value) {
    assert_eq!(settings["text_scale"], 1.25, "{settings}");
    assert_eq!(settings["double_click_ms"], 1200, "{settings}");
    assert_eq!(settings["drag_threshold"], 12, "{settings}");
    assert_eq!(settings["cursor_blink"], false, "{settings}");
    assert_eq!(settings["cursor_blink_time_ms"], 900, "{settings}");
    assert_eq!(settings["cursor_blink_timeout_s"], 4, "{settings}");
    assert_eq!(settings["enable_animations"], false, "{settings}");
    assert_eq!(settings["reduced_motion"], true, "{settings}");
    assert_eq!(settings["remember_recent_files"], false, "{settings}");
    assert_eq!(known(settings), ALL_KEYS, "{settings}");
}

/// The JSON lines from `from` on whose `ev` is `ev`.
fn events(app: &Proc, from: usize, ev: &str) -> Vec<Value> {
    app.lines()
        .iter()
        .skip(from)
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["ev"] == ev)
        .collect()
}

/// Waits for the next `changed` line for `key`, from line `from` on; returns the line after it.
fn wait_changed(app: &Proc, from: usize, key: &str) -> (usize, Value) {
    let (at, v) = app.wait_json_from(from, &format!("a change to {key}"), |v| {
        v["ev"] == "changed" && v["key"] == key
    });
    (at + 1, v["settings"].clone())
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn reads_the_snapshot() {
    let s = bus_only();
    let _portal = FakePortal::start(s.bus().address(), &desktop(), true);
    let app = watch(&s);

    assert_desktop(&snapshot(&app));
    assert!(events(&app, 0, "fallback").is_empty(), "{:?}", app.lines());
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn follows_every_change() {
    let s = bus_only();
    let portal = FakePortal::start(s.bus().address(), &desktop(), true);
    let app = watch(&s);
    snapshot(&app);
    let mut at = app.line_count();

    let changes: [(&str, &str, Variant<'static>, &str, Value); 9] = [
        (
            INTERFACE,
            "text-scaling-factor",
            Variant::F64(1.5),
            "text_scale",
            1.5.into(),
        ),
        (
            MOUSE,
            "double-click",
            Variant::I32(250),
            "double_click_ms",
            250.into(),
        ),
        (
            MOUSE,
            "drag-threshold",
            Variant::I32(3),
            "drag_threshold",
            3.into(),
        ),
        (
            INTERFACE,
            "cursor-blink",
            Variant::Bool(true),
            "cursor_blink",
            true.into(),
        ),
        (
            INTERFACE,
            "cursor-blink-time",
            Variant::I32(600),
            "cursor_blink_time_ms",
            600.into(),
        ),
        (
            INTERFACE,
            "cursor-blink-timeout",
            Variant::I32(20),
            "cursor_blink_timeout_s",
            20.into(),
        ),
        (
            INTERFACE,
            "enable-animations",
            Variant::Bool(true),
            "enable_animations",
            true.into(),
        ),
        (
            APPEARANCE,
            "reduced-motion",
            Variant::U32(0),
            "reduced_motion",
            false.into(),
        ),
        (
            PRIVACY,
            "remember-recent-files",
            Variant::Bool(true),
            "remember_recent_files",
            true.into(),
        ),
    ];
    for (namespace, key, value, field, expected) in changes {
        portal.set(namespace, key, value);
        let (next, settings) = wait_changed(&app, at, key);
        assert_eq!(settings[field], expected, "{key}: {settings}");
        at = next;
    }
    assert_eq!(
        events(&app, at - 1, "changed")
            .last()
            .map(|v| v["settings"]["animations"].clone()),
        Some(true.into())
    );

    // Noise: another namespace, a key the module doesn't read, the value it already has, and
    // unusable values (twice: one fallback line), then a real change that ends it.
    let noise_from = app.line_count();
    portal.emit(
        "org.example.other",
        "text-scaling-factor",
        Variant::F64(3.0),
    );
    portal.emit(INTERFACE, "font-name", Variant::from("Serif 12"));
    portal.set(INTERFACE, "text-scaling-factor", Variant::F64(1.5));
    portal.emit(INTERFACE, "text-scaling-factor", Variant::I32(2));
    portal.emit(INTERFACE, "text-scaling-factor", Variant::F64(0.0));
    portal.emit(INTERFACE, "text-scaling-factor", Variant::F64(f64::NAN));
    portal.emit(
        INTERFACE,
        "text-scaling-factor",
        Variant::F64(f64::INFINITY),
    );
    portal.emit(INTERFACE, "text-scaling-factor", Variant::F64(-1.0));
    portal.set(MOUSE, "double-click", Variant::I32(700));
    let (_, settings) = wait_changed(&app, noise_from, "double-click");

    let changed = events(&app, noise_from, "changed");
    assert_eq!(changed.len(), 1, "only the last change: {changed:?}");
    assert_eq!(settings["text_scale"], 1.5, "{settings}");
    let fallbacks = events(&app, noise_from, "fallback");
    assert_eq!(
        fallbacks.len(),
        1,
        "one report for the bad values: {fallbacks:?}"
    );
    assert_eq!(fallbacks[0]["kind"], "invalid");
    assert_eq!(fallbacks[0]["key"], "text-scaling-factor");
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn no_portal_gives_the_defaults() {
    let s = bus_only();
    let app = watch(&s);

    assert_defaults(&snapshot(&app));
    let (_, fallback) = app.wait_json_from(0, "the no-portal fallback", |v| v["ev"] == "fallback");
    assert_eq!(fallback["kind"], "no-portal", "{fallback}");
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn a_portal_that_cannot_start_gives_the_defaults_at_once() {
    let s = bus_only();
    s.bus()
        .add_service("org.freedesktop.portal.Desktop", "/bin/false");
    let started = Instant::now();
    let app = watch(&s);

    assert_defaults(&snapshot(&app));
    let (_, fallback) = app.wait_json_from(0, "the no-portal fallback", |v| v["ev"] == "fallback");
    assert_eq!(fallback["kind"], "no-portal", "{fallback}");
    // The activation fails fast; the watcher's 10 s bound isn't what ends the wait.
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn no_session_bus_gives_the_defaults() {
    let s = bus_only();
    // A socket that doesn't exist, in the session's own runtime directory: never unset, so
    // nothing can fall back to another bus.
    let missing = s.runtime_dir().join("no-bus");
    assert!(!missing.exists());
    let mut cmd = s.command(example("settings"), Display::None);
    cmd.arg("--no-window").env(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}", missing.display()),
    );
    let app = s.spawn("settings", cmd);

    assert_defaults(&snapshot(&app));
    let (_, fallback) = app.wait_json_from(0, "the no-bus fallback", |v| v["ev"] == "fallback");
    assert_eq!(fallback["kind"], "no-bus", "{fallback}");
}

/// utime + stime of `pid`, in clock ticks.
fn cpu_ticks(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("the example's stat");
    // Fields after the parenthesised name: state is field 3, utime 14 and stime 15.
    let rest = &stat[stat.rfind(')').expect("stat's name") + 2..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    fields[11].parse::<u64>().unwrap_or(0) + fields[12].parse::<u64>().unwrap_or(0)
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn waits_without_polling() {
    let s = bus_only();
    let portal = FakePortal::start(s.bus().address(), &desktop(), true);
    let app = watch(&s);
    snapshot(&app);
    let before = cpu_ticks(app.pid());

    std::thread::sleep(timeout() / 5);

    let reads: Vec<_> = portal.calls();
    assert_eq!(reads.len(), 1, "one ReadAll, then nothing: {reads:?}");
    assert_eq!(reads[0].method, "ReadAll");
    assert!(reads[0].sender.starts_with(':'), "{reads:?}");
    let spent = cpu_ticks(app.pid()) - before;
    // A busy loop would spend about every tick of the wait (100 a second).
    assert!(spent < 10, "{spent} ticks idle");
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn follows_a_portal_that_starts_late_and_restarts() {
    let s = bus_only();
    let app = watch(&s);
    assert_defaults(&snapshot(&app));
    app.wait_json("the no-portal fallback", |v| {
        v["ev"] == "fallback" && v["kind"] == "no-portal"
    });

    // A portal takes the name: it is read, and every key arrives as a change.
    let late = app.line_count();
    let portal = FakePortal::start(s.bus().address(), &desktop(), true);
    // The last key in the module's order.
    let (at, settings) = wait_changed(&app, late, "remember-recent-files");
    assert_desktop(&settings);
    assert_eq!(events(&app, late, "changed").len(), ALL_KEYS.len());

    // It leaves: the values stay.
    portal.release();
    let (at, _) = app.wait_json_from(at, "the portal-lost fallback", |v| {
        v["ev"] == "fallback" && v["kind"] == "portal-lost"
    });

    // It comes back with one value changed while it was gone: that one change arrives.
    portal.set_quietly(INTERFACE, "text-scaling-factor", Variant::F64(2.0));
    portal.acquire();
    let (end, settings) = wait_changed(&app, at, "text-scaling-factor");
    assert_eq!(settings["text_scale"], 2.0, "{settings}");
    assert_eq!(settings["double_click_ms"], 1200, "{settings}");
    portal.set(MOUSE, "double-click", Variant::I32(300));
    let (_, last) = wait_changed(&app, end, "double-click");
    assert_eq!(last["double_click_ms"], 300);
    assert_eq!(events(&app, at, "changed").len(), 2, "{:?}", app.lines());
}

/// The example's window under sway, with the fake portal; returns once its first frame is
/// drawn and its first update ran on Rune's loop.
fn window(s: &Session) -> Proc {
    let app = s.spawn("settings", s.command(example("settings"), Display::Wayland));
    app.wait_json("ready", |v| v["ev"] == "ready");
    app.wait_json("the first frame", |v| v["ev"] == "frame");
    let update = app.wait_json("the first update", |v| v["ev"] == "update");
    assert_desktop(&update["settings"]);
    s.wait_window(app.pid());
    app
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn changes_reach_runes_loop() {
    let s = Session::start();
    let portal = FakePortal::start(s.bus().address(), &desktop(), true);
    let app = window(&s);
    let at = app.line_count();

    portal.set(INTERFACE, "text-scaling-factor", Variant::F64(2.0));

    let (_, update) = app.wait_json_from(at, "the update on Rune's loop", |v| {
        v["ev"] == "update" && v["settings"]["text_scale"] == 2.0
    });
    assert_eq!(update["settings"]["double_click_ms"], 1200);
}

#[test]
#[ignore = "needs a private session: mise run test-platform"]
fn changes_reach_a_covered_window() {
    let s = Session::start();
    let portal = FakePortal::start(s.bus().address(), &desktop(), true);
    let app = window(&s);
    let cover = s.peer(Display::Wayland);
    cover.map("cover");
    let covering = s.wait_window(cover.pid());
    s.sway_command(&format!("[con_id={}] fullscreen enable", covering.id));
    let fullscreen = |tree: &Value| -> bool {
        fn find(node: &Value, id: i64) -> Option<&Value> {
            if node["id"] == id {
                return Some(node);
            }
            ["nodes", "floating_nodes"]
                .iter()
                .filter_map(|k| node[*k].as_array())
                .flatten()
                .find_map(|n| find(n, id))
        }
        find(tree, covering.id).is_some_and(|n| n["fullscreen_mode"] == 1)
    };
    assert!(fullscreen(&s.tree()), "the cover is full screen");
    let at = app.line_count();

    portal.set(INTERFACE, "text-scaling-factor", Variant::F64(1.75));

    app.wait_json_from(at, "the update while covered", |v| {
        v["ev"] == "update" && v["settings"]["text_scale"] == 1.75
    });
}
