//! The `single-instance` shim without Rune, for strata-platform's session tests (P8). It
//! claims `org.example.SingleInstanceProbe` and reports as JSON lines on stdout:
//!
//! - `{"ev":"claim","result":"primary"}`, `"forwarded"` with `"status"`, or `"standalone"`
//!   with `"reason"`;
//! - as the primary, one line per event: `launch` (with `remote`, `args` and `cwd` as hex
//!   bytes, `xdg` and `sid` tokens), `activate`, `open`, `action` and `reveal`.
//!
//! As the primary it answers each launch by its arguments: `--print` prints a line to the
//! launch's stdout, `--usage-error` a line to its stderr and exit status 1. Its stdin takes
//! `release` (stop being the primary) and `quit`; the action `quit` ends it too. With
//! `PROBE_SERVICE=1` the end of stdin doesn't: the bus starts it with none.
//!
//! The environment sets the rest: `PROBE_GATE=1` waits for a line on stdin before claiming,
//! `PROBE_SERVICE=1` and `PROBE_FILE_MANAGER1=1` set those options, and
//! `PROBE_BUS_TIMEOUT_MS` the bus timeout.

#[cfg(feature = "single-instance")]
fn main() {
    probe::main();
}

#[cfg(not(feature = "single-instance"))]
fn main() {
    eprintln!("run with --features single-instance");
}

#[cfg(feature = "single-instance")]
mod probe {
    use std::ffi::OsStr;
    use std::io::{BufRead, Write};
    use std::os::unix::ffi::OsStrExt;
    use std::time::Duration;

    use serde_json::{Value, json};
    use strata_platform::single_instance::{
        AppId, Claim, Event, Launch, Options, RevealMethod, Unclaimed, claim,
    };

    const APP_ID: &str = "org.example.SingleInstanceProbe";

    fn log(event: &str, fields: Value) {
        let mut line = json!({ "ev": event });
        if let (Some(obj), Value::Object(fields)) = (line.as_object_mut(), fields) {
            obj.extend(fields);
        }
        // A closed stdout (a test checks one) must not stop the probe.
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    fn hex(bytes: &OsStr) -> String {
        bytes
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn flag(name: &str) -> bool {
        std::env::var_os(name).is_some_and(|v| v == "1")
    }

    fn on_event(event: Event) {
        match event {
            Event::CommandLine(cmd) => {
                let args: Vec<String> = cmd.arguments().iter().map(|a| hex(a)).collect();
                log(
                    "launch",
                    json!({
                        "remote": cmd.is_remote(),
                        "args": args,
                        "cwd": cmd.cwd().map(|c| hex(c.as_os_str())),
                        "xdg": cmd.activation().xdg_activation_token,
                        "sid": cmd.activation().desktop_startup_id,
                    }),
                );
                let has = |a: &str| cmd.arguments().iter().any(|x| x == a);
                if has("--print") {
                    cmd.print("hello from the primary\n");
                }
                if has("--usage-error") {
                    cmd.print_err("usage: single-instance-probe [--print] [FILE...]\n");
                    cmd.exit(1);
                }
            }
            Event::Activate(activation) => log(
                "activate",
                json!({ "xdg": activation.xdg_activation_token, "sid": activation.desktop_startup_id }),
            ),
            Event::Open {
                uris,
                hint,
                activation,
            } => log(
                "open",
                json!({ "uris": uris, "hint": hint, "xdg": activation.xdg_activation_token }),
            ),
            Event::Action {
                name,
                parameter,
                activation,
            } => {
                log(
                    "action",
                    json!({
                        "name": name,
                        "parameter": parameter.map(|p| format!("{p:?}")),
                        "xdg": activation.xdg_activation_token,
                    }),
                );
                // A probe the bus started has no stdin to quit by.
                if name == "quit" {
                    std::process::exit(0);
                }
            }
            Event::Reveal(reveal) => log(
                "reveal",
                json!({
                    "method": match reveal.method {
                        RevealMethod::Folders => "folders",
                        RevealMethod::Items => "items",
                        RevealMethod::ItemProperties => "item-properties",
                    },
                    "uris": reveal.uris,
                    "startup_id": reveal.startup_id,
                }),
            ),
        }
    }

    pub fn main() {
        let mut lines = std::io::stdin().lock().lines();
        if flag("PROBE_GATE") {
            let _ = lines.next();
        }
        let id = AppId::new(APP_ID).expect("a valid app id");
        let mut options = Options {
            service: flag("PROBE_SERVICE"),
            file_manager1: flag("PROBE_FILE_MANAGER1"),
            ..Options::default()
        };
        if let Some(ms) = std::env::var("PROBE_BUS_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            options.bus_timeout = Duration::from_millis(ms);
        }
        let primary = match claim(&id, Launch::from_env(), &options) {
            Claim::Primary(p) => {
                log(
                    "claim",
                    json!({ "result": "primary", "pid": std::process::id() }),
                );
                p
            }
            Claim::Standalone(p, reason) => {
                let reason = match reason {
                    Unclaimed::NoBus(e) => format!("no-bus: {e}"),
                    Unclaimed::Timeout => "timeout".into(),
                    Unclaimed::Error(e) => format!("error: {e}"),
                };
                log("claim", json!({ "result": "standalone", "reason": reason }));
                p
            }
            Claim::Forwarded(status) => {
                log("claim", json!({ "result": "forwarded", "status": status }));
                std::process::exit(status);
            }
        };
        primary.attach(on_event);
        for line in lines {
            match line.as_deref().map(str::trim) {
                Ok("release") => {
                    primary.release();
                    log("released", json!({ "claimed": primary.is_claimed() }));
                }
                Ok("quit") | Err(_) => break,
                Ok(_) => {}
            }
        }
        if options.service {
            loop {
                std::thread::park();
            }
        }
        log("exit", json!({}));
    }
}
