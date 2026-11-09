//! X2's session tests: the drag-and-drop wiring demo (`examples/dnd_wiring/`) against W3's
//! peer in a private headless sway, on Wayland and X11 (`mise run test-platform x2::`).
//!
//! Each test starts the demo on a fixture folder made in the session's private runtime
//! directory, finds its targets with the demo's `locate` command (window points from
//! `Cx::window_layout`), and drags with the session's virtual pointer. What the demo did
//! comes back as its JSON lines (`transfer`, `drag-end`, `spring`, …), and what the peer got
//! as its events.
//!
//! Work package: X2, `docs/work/phase-4-integration/X2-dnd-wiring.md`.

#[path = "../../strata-platform/tests/support/mod.rs"]
#[expect(
    dead_code,
    reason = "W3's harness, shared with strata-platform's session tests: X2 uses part of it"
)]
#[expect(
    unexpected_cfgs,
    unused_imports,
    reason = "the harness gates helpers on strata-platform's features, which this crate lacks \
              (so X2 uses none of those helpers: their expectations hold)"
)]
mod support;

mod x2 {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use serde_json::{Value, json};
    use strata_test_peer::protocol::{Action, Event, Offer};

    use crate::support::{Display, Peer, Proc, Session, Window, example, timeout};

    const URI_LIST: &str = "text/uri-list";
    const GNOME: &str = "x-special/gnome-copied-files";
    const KDE: &str = "application/x-kde-cutselection";

    /// The fixture: folders with a file each, a long folder for autoscroll, `usb` (another
    /// volume to the demo), `pinned`.
    fn fixture(s: &Session) -> PathBuf {
        let root = s.runtime_dir().join("root");
        for d in [
            "Documents/Reports",
            "Music",
            "Pictures",
            "pinned",
            "usb",
            "Deep/a/b/c/d/e",
        ] {
            std::fs::create_dir_all(root.join(d)).expect("fixture folder");
        }
        for (f, body) in [
            ("notes.txt", "notes"),
            ("todo.txt", "todo"),
            ("Documents/plan.txt", "plan"),
            ("Documents/Reports/q3.txt", "q3"),
            ("usb/stick.txt", "stick"),
        ] {
            std::fs::write(root.join(f), body).expect("fixture file");
        }
        for i in 0..60 {
            std::fs::write(root.join(format!("Music/track{i:02}.ogg")), "x").expect("track");
        }
        root.canonicalize().expect("the root")
    }

    fn uri(p: &Path) -> String {
        // The fixture's paths need no escaping.
        format!("file://{}", p.display())
    }

    pub(super) struct Demo {
        pub app: Proc,
        pub title: String,
        pub keys: Vec<&'static str>,
        pub root: PathBuf,
    }

    impl Demo {
        pub fn start(s: &Session, display: Display, two: bool, apply: bool) -> Demo {
            let root = fixture(s);
            let title = "x2 dnd".to_owned();
            let mut cmd = s.command(example("dnd_wiring"), display);
            cmd.arg("--root").arg(&root).args(["--title", &title]);
            if two {
                cmd.arg("--two-windows");
            }
            if apply {
                cmd.arg("--apply");
            }
            let app = s.spawn("dnd_wiring", cmd);
            let keys = if two {
                vec!["main", "second"]
            } else {
                vec!["main"]
            };
            let demo = Demo {
                app,
                title,
                keys,
                root,
            };
            for k in &demo.keys {
                let (_, v) = demo.wait(0, &format!("{k} attaching"), |v| {
                    (v["ev"] == "attached" || v["ev"] == "attach-error") && v["window"] == *k
                });
                assert_eq!(v["ev"], "attached", "{v}");
                assert_eq!(v["drag_in"], true, "{v}");
                assert_eq!(v["drag_out"], true, "{v}");
            }
            demo.wait(0, "the first frame", |v| v["ev"] == "frame");
            demo
        }

        fn title_of(&self, key: &str) -> String {
            if key == "main" {
                self.title.clone()
            } else {
                format!("{} {key}", self.title)
            }
        }

        pub fn window(&self, s: &Session, key: &str) -> Window {
            let (title, pid) = (self.title_of(key), self.app.pid());
            s.wait_window_where(&format!("the {key} window"), |w| {
                w.pid == pid && w.title.starts_with(&title) && w.content.width > 0.0
            })
        }

        pub fn send(&self, cmd: Value) -> usize {
            let mark = self.app.line_count();
            self.app.send_line(&cmd.to_string());
            mark
        }

        pub fn wait(
            &self,
            from: usize,
            what: &str,
            pred: impl Fn(&Value) -> bool,
        ) -> (usize, Value) {
            self.app.wait_json_from(from, what, pred)
        }

        /// Whether a line passing `pred` came from line `from` on, within `wait`.
        pub fn saw(&self, from: usize, wait: Duration, pred: impl Fn(&Value) -> bool) -> bool {
            let deadline = Instant::now() + wait;
            loop {
                if self.lines_from(from).iter().any(&pred) {
                    return true;
                }
                if Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        pub fn lines_from(&self, from: usize) -> Vec<Value> {
            self.app
                .lines()
                .iter()
                .skip(from)
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .collect()
        }

        /// Waits until window `key` has laid out at the size sway gave it.
        pub fn settle(&self, s: &Session, key: &str) -> Window {
            let deadline = Instant::now() + timeout();
            loop {
                let w = self.window(s, key);
                let laid = self
                    .lines_from(0)
                    .iter()
                    .rev()
                    .find(|v| v["ev"] == "layout" && v["window"] == key)
                    .and_then(|v| v["w"].as_f64());
                if laid.is_some_and(|lw| (lw - w.content.width).abs() < 1.0) {
                    return w;
                }
                assert!(
                    Instant::now() < deadline,
                    "window {key} never laid out at {:?}: {laid:?}",
                    w.content
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        /// Sends `cmd` to window `key` and waits for the line `ack`.
        pub fn command(&self, key: &str, mut cmd: Value, ack: &str) -> Value {
            cmd["window"] = json!(key);
            let mark = self.send(cmd);
            self.wait(mark, ack, |v| v["ev"] == ack && v["window"] == key)
                .1
        }

        /// Where each of `what` is in window `key`, in the screen's points (sway's layout).
        pub fn locate(&self, s: &Session, key: &str, what: &[Value]) -> HashMap<String, [f64; 4]> {
            let w = self.settle(s, key);
            let deadline = Instant::now() + timeout();
            loop {
                let mark = self.send(json!({ "cmd": "locate", "window": key, "what": what }));
                let mut out = HashMap::new();
                for item in what {
                    let name = item["name"].as_str().expect("named").to_owned();
                    let (_, v) = self.wait(mark, &format!("where {name} is ({item})"), |v| {
                        v["ev"] == "rect" && v["window"] == key && v["name"] == name.as_str()
                    });
                    if let Some(r) = v["r"].as_array() {
                        let r: Vec<f64> = r.iter().filter_map(Value::as_f64).collect();
                        if r.len() == 4 && r[2] > 0.0 && r[3] > 0.0 {
                            out.insert(name, [w.content.x + r[0], w.content.y + r[1], r[2], r[3]]);
                        }
                    }
                }
                if out.len() == what.len() {
                    return out;
                }
                assert!(
                    Instant::now() < deadline,
                    "never located {what:?} in {key}: {out:?}"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }

        /// The point `(fx, fy)` across what `item` names in window `key`.
        pub fn at(&self, s: &Session, key: &str, item: Value, fx: f64, fy: f64) -> (f64, f64) {
            let mut item = item;
            item["name"] = json!("it");
            let r = self.locate(s, key, &[item])["it"];
            (r[0] + r[2] * fx, r[1] + r[3] * fy)
        }

        /// Gives window `key` keyboard focus through sway and waits until the app has heard it
        /// settle (a click that focuses an unfocused Xwayland window can lose its press).
        pub fn focus(&self, s: &Session, key: &str) {
            let w = self.window(s, key);
            if !w.focused {
                s.focus(&w);
            }
            let last = || {
                self.lines_from(0)
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, v)| v["ev"] == "focus" && v["window"] == key)
                    .map(|(i, v)| (i, v["focused"] == true))
            };
            let deadline = Instant::now() + timeout();
            loop {
                let before = last();
                std::thread::sleep(Duration::from_millis(250));
                let after = last();
                if before == after && after.is_some_and(|(_, f)| f) {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "window {key} never settled focused: {after:?}"
                );
            }
        }

        pub fn quit(mut self) {
            let mark = self.send(json!({ "cmd": "quit", "window": "main" }));
            self.wait(mark, "the shutdown", |v| v["ev"] == "shutdown");
            let status = self.app.wait_exit();
            assert!(status.success(), "the demo exited with {status}");
            self.no_errors();
        }

        pub fn no_errors(&self) {
            let bad: Vec<String> = self
                .app
                .lines()
                .into_iter()
                .filter(|l| {
                    [
                        "diagnostic",
                        "drag-error",
                        "attach-error",
                        "exit-error",
                        "bad-command",
                    ]
                    .iter()
                    .any(|e| l.contains(&format!("\"{e}\"")))
                })
                .collect();
            assert!(bad.is_empty(), "the demo reported errors: {bad:?}");
        }
    }

    /// A peer with its window mapped, tiled beside the demo's.
    pub(super) fn peer_beside(s: &Session, display: Display, demo: &Demo) -> (Peer, Window) {
        let peer = s.peer(display);
        peer.map("x2 peer");
        let windows = s.wait_windows(&[peer.pid(), demo.app.pid()]);
        let w = windows
            .iter()
            .find(|w| w.pid == peer.pid())
            .expect("the peer's window")
            .clone();
        peer.sync();
        (peer, w)
    }

    /// Presses on `at` in the demo's window `key`, focused first, and waits for the press to
    /// reach a collection. After a relayout a first motion can reach no surface (W3's traps),
    /// so a press that doesn't arrive is released and tried again.
    fn press(s: &Session, demo: &Demo, key: &str, at: (f64, f64)) -> usize {
        demo.focus(s, key);
        for _ in 0..5 {
            s.input().pointer_move((at.0 + 1.0, at.1));
            std::thread::sleep(Duration::from_millis(100));
            s.input().pointer_move(at);
            std::thread::sleep(Duration::from_millis(150));
            let mark = demo.app.line_count();
            s.input().button(true);
            if demo.saw(mark, Duration::from_secs(2), |v| {
                v["ev"] == "press" && v["window"] == key
            }) {
                return mark;
            }
            s.input().button(false);
            std::thread::sleep(Duration::from_millis(300));
        }
        panic!("no press reached {key} at {at:?}");
    }

    /// Drags `item` in window `key` out to `to` (the peer's window) and drops it: the outcome.
    fn drag_out(
        s: &Session,
        demo: &Demo,
        key: &str,
        item: Value,
        to: (f64, f64),
    ) -> (usize, Value) {
        // A row's icon is near its start; a tile's `/icon` is the icon itself.
        let fx = if item.get("depth").is_none() && item["tab"] == 3 {
            0.5
        } else {
            0.1
        };
        let from = demo.at(s, key, item, fx, 0.5);
        let mark = press(s, demo, key, from);
        s.input()
            .path(((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0));
        demo.wait(mark, "the drag starting", |v| v["ev"] == "drag-start");
        s.input().path(to);
        std::thread::sleep(Duration::from_millis(150));
        s.input().button(false);
        (
            mark,
            demo.wait(mark, "the outcome", |v| v["ev"] == "drag-end").1,
        )
    }

    fn row(tab: u64, name: &str) -> Value {
        json!({ "tab": tab, "row": name })
    }

    // ---- Drag out ----

    /// operations/drag-and-drop: "Dragged items are offered as a GDK file list and a
    /// `text/uri-list`, so other applications can receive them" (the three formats Strata's
    /// port offers instead), from Columns, List and Icons in turn; the outcome reaches the
    /// demo, and the rows' dimming clears.
    fn drag_out_from_each_view(display: Display) {
        let s = Session::start();
        let demo = Demo::start(&s, display, false, false);
        let (peer, to) = peer_beside(&s, display, &demo);
        let notes = uri(&demo.root.join("notes.txt"));
        peer.accept_drops_with(
            &[URI_LIST, GNOME, KDE],
            &[Action::Copy, Action::Move],
            Action::Copy,
            true,
            None,
        );
        let views = [
            (1u64, json!({ "tab": 1, "row": "notes.txt", "depth": 0 })),
            (2, row(2, "notes.txt")),
            (3, row(3, "notes.txt")),
        ];
        for (tab, item) in views {
            select_tab(&demo, "main", tab);
            let peer_mark = peer.line_count();
            let (mark, end) = drag_out(&s, &demo, "main", item, to.at(0.5, 0.5));
            assert_eq!(end["outcome"], "copied", "tab {tab}: {end}");
            let got = dropped_on_peer(&peer, 3);
            for (mime, want) in [
                (URI_LIST, format!("{notes}\r\n")),
                (GNOME, format!("copy\n{notes}")),
                (KDE, "0".to_owned()),
            ] {
                let (text, action) = got.get(mime).unwrap_or_else(|| panic!("{mime}: {got:?}"));
                assert_eq!(text.as_deref(), Some(want.as_str()), "tab {tab}, {mime}");
                assert_eq!(*action, Some(Action::Copy));
            }
            let _ = peer_mark;
            // The drag's rows were dimmed; its end clears them (the demo's marks follow it).
            assert!(demo.saw(mark, Duration::ZERO, |v| v["ev"] == "drag-start"
                && v["items"] == json!(["notes.txt"])));
        }
        demo.quit();
    }

    /// The next `n` drops the peer reads, by type: their text and action (they finish in any
    /// order on X11).
    fn dropped_on_peer(peer: &Peer, n: usize) -> HashMap<String, (Option<String>, Option<Action>)> {
        let mut out = HashMap::new();
        for _ in 0..n {
            if let Event::Dropped { mime, action, data } =
                peer.wait_event("a drop read", |e| matches!(e, Event::Dropped { .. }))
            {
                out.insert(mime, (data.text, action));
            }
        }
        out
    }

    fn select_tab(demo: &Demo, key: &str, tab: u64) {
        demo.send(json!({ "cmd": "tab", "id": tab, "window": key }));
        let st = demo.command(key, json!({ "cmd": "state" }), "state");
        assert_eq!(st["tab"], tab, "{st}");
    }

    #[test]
    #[ignore = "needs a private session: mise run test-platform"]
    fn wayland_drag_out_from_each_view_offers_the_three_formats() {
        drag_out_from_each_view(Display::Wayland);
    }

    #[test]
    #[ignore = "needs a private session: mise run test-platform"]
    fn x11_drag_out_from_each_view_offers_the_three_formats() {
        drag_out_from_each_view(Display::X11);
    }
}
