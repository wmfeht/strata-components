//! Prints the desktop's settings and every change to them, as JSON lines on stdout (the
//! `settings` feature, P5).
//!
//! - `{"ev":"snapshot","settings":{…}}` once, first;
//! - `{"ev":"changed","key":"text-scaling-factor","settings":{…}}` for each change;
//! - `{"ev":"fallback","kind":"no-portal","detail":"…"}` the first time each fallback happens.
//!
//! Those come from the watcher's own thread. By default the example also opens a window that
//! shows the values, and logs `{"ev":"update","settings":{…}}` from inside the
//! `AppHandle::update` that brought each new value onto Rune's loop, plus `ready` and `frame`.
//! `--no-window` skips the window and Rune's loop, and exits when stdin closes.
//! `--first-answer-ms N` sets how long the start waits for the portal (default 100).
//!
//! `cargo run -p strata-platform --example settings --features settings` reads the desktop's
//! portal. The session tests run it on a private bus only (`tests/session/p5.rs`).

#[cfg(feature = "settings")]
mod demo {
    use std::io::{BufRead, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use rune::{App, AppSpec, Cx, El, WindowHandle, col, text};
    use serde_json::{Value, json};
    use strata_platform::settings::{DesktopSettings, Event, Fallback, Key, Options, Watcher};

    fn log(event: &str, fields: Value) {
        let mut line = json!({ "ev": event });
        if let (Some(obj), Value::Object(fields)) = (line.as_object_mut(), fields) {
            obj.extend(fields);
        }
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    fn settings_json(s: &DesktopSettings) -> Value {
        let known: Vec<&str> = Key::ALL
            .into_iter()
            .filter(|k| s.is_known(*k))
            .map(Key::name)
            .collect();
        json!({
            "text_scale": s.text_scale,
            "double_click_ms": s.double_click.as_millis() as u64,
            "drag_threshold": s.drag_threshold,
            "cursor_blink": s.cursor_blink,
            "cursor_blink_time_ms": s.cursor_blink_time.as_millis() as u64,
            "cursor_blink_timeout_s": s.cursor_blink_timeout.as_secs(),
            "enable_animations": s.enable_animations,
            "reduced_motion": s.reduced_motion,
            "remember_recent_files": s.remember_recent_files,
            "animations": s.animations(),
            "known": known,
        })
    }

    fn print(event: &Event) {
        match event {
            Event::Snapshot(s) => log("snapshot", json!({ "settings": settings_json(s) })),
            Event::Changed { setting, settings } => log(
                "changed",
                json!({ "key": setting.key().name(), "settings": settings_json(settings) }),
            ),
            Event::Fallback(f) => {
                let (kind, key, detail) = match f {
                    Fallback::NoBus(d) => ("no-bus", None, d.clone()),
                    Fallback::NoPortal(d) => ("no-portal", None, d.clone()),
                    Fallback::Unset(k) => ("unset", Some(k.name()), String::new()),
                    Fallback::Invalid { key, value } => {
                        ("invalid", Some(key.name()), value.clone())
                    }
                    Fallback::PortalLost => ("portal-lost", None, String::new()),
                    other => ("other", None, format!("{other:?}")),
                };
                log(
                    "fallback",
                    json!({ "kind": kind, "key": key, "detail": detail }),
                );
            }
            other => log("other", json!({ "event": format!("{other:?}") })),
        }
    }

    fn options(args: &[String]) -> Options {
        let first = args
            .iter()
            .position(|a| a == "--first-answer-ms")
            .and_then(|i| args.get(i + 1))
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(100);
        Options::default().first_answer(Duration::from_millis(first))
    }

    fn no_window(options: Options) {
        let watcher = match Watcher::start(options, |event| print(&event)) {
            Ok(watcher) => watcher,
            Err(error) => {
                log("exit-error", json!({ "error": error.to_string() }));
                std::process::exit(1);
            }
        };
        log(
            "started",
            json!({ "settings": settings_json(&watcher.snapshot()) }),
        );
        for line in std::io::stdin().lock().lines() {
            if line.is_err() {
                break;
            }
        }
        drop(watcher);
        log("stopped", json!({}));
    }

    #[derive(Default)]
    struct State {
        desktop: DesktopSettings,
    }

    fn view(s: &State, _: &mut Cx<State>) -> El {
        let d = &s.desktop;
        col()
            .w_full()
            .h_full()
            .center()
            .child(text(format!("text scale {}", d.text_scale)))
            .child(text(format!(
                "double click {} ms",
                d.double_click.as_millis()
            )))
            .child(text(format!("drag threshold {} px", d.drag_threshold)))
            .child(text(format!(
                "caret blink {} every {} ms for {} s",
                d.cursor_blink,
                d.cursor_blink_time.as_millis(),
                d.cursor_blink_timeout.as_secs()
            )))
            .child(text(format!(
                "animations {} (switch {}, reduced motion {})",
                d.animations(),
                d.enable_animations,
                d.reduced_motion
            )))
            .child(text(format!(
                "remember recent files {}",
                d.remember_recent_files
            )))
    }

    fn window(options: Options) {
        rune::tooling::set_app_id("strata-settings-example");
        let app = App::new(AppSpec::new(
            "settings",
            (480.0, 280.0),
            State::default,
            view,
        ));
        let handle = app.handle();
        let watcher = Watcher::start(options, move |event| {
            print(&event);
            let settings = match event {
                Event::Snapshot(settings) | Event::Changed { settings, .. } => settings,
                _ => return,
            };
            handle.update_if(move |s: &mut State, _| {
                log("update", json!({ "settings": settings_json(&settings) }));
                let changed = s.desktop != settings;
                s.desktop = settings;
                changed
            });
        });
        let watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                log("exit-error", json!({ "error": error.to_string() }));
                std::process::exit(1);
            }
        };
        let first = watcher.snapshot();
        static DRAWN: AtomicBool = AtomicBool::new(false);
        let result = app
            .init(move || State { desktop: first })
            .on_ready(|_: &WindowHandle| log("ready", json!({})))
            .on_frame_drawn(|_| {
                if !DRAWN.swap(true, Ordering::Relaxed) {
                    log("frame", json!({}));
                }
            })
            .try_run();
        drop(watcher);
        if let Err(error) = result {
            log("exit-error", json!({ "error": error }));
            std::process::exit(1);
        }
    }

    pub fn main() {
        let args = rune::tooling::extra_args();
        let options = options(args);
        if args.iter().any(|a| a == "--no-window") {
            no_window(options);
        } else {
            window(options);
        }
    }
}

#[cfg(feature = "settings")]
fn main() {
    demo::main();
}

#[cfg(not(feature = "settings"))]
fn main() {
    eprintln!("run with --features settings");
}
