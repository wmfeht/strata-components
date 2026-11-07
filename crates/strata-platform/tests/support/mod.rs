//! The compositor and D-Bus harness (W3): each test gets a private session of its own.
//!
//! [`Session::start`] brings up, under a private `XDG_RUNTIME_DIR`:
//!
//! 1. a session bus ([`Bus`]): a `dbus-daemon` with only the session's service directory;
//! 2. a nested headless sway, with Xwayland (`sway.rs` lists the traps it works around);
//! 3. an input peer ([`Session::input`]): a virtual pointer and keyboard, without which a
//!    headless seat gives no client pointer events or keyboard focus;
//! 4. a watchdog that stops sway and the bus if the test process dies without tearing down.
//!
//! Processes start through [`Session::command`], with an environment built from nothing
//! ([`Session::env`]), so none can reach the desktop's display, bus or runtime directory.
//! Dropping the session stops everything and removes the runtime directory. Logs stay in
//! `target/tmp/session/<test>/`.
//!
//! Every test that starts a session is `#[ignore]`d: `mise run test-platform` runs them, and
//! `mise run test` never does. See docs/testing.md, "Compositor and D-Bus tests".

mod bus;
mod gpu;
mod peer;
mod process;
mod sway;

use std::ffi::{OsStr, OsString};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

pub use bus::Bus;
pub use peer::Peer;
pub use process::Proc;
pub use sway::{Shell, Window};

/// The display a process connects to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    /// The session's sway, over Wayland (`WAYLAND_DISPLAY` only).
    Wayland,
    /// The session's Xwayland (`DISPLAY` only), so winit takes its X11 backend.
    X11,
    /// Neither: a process that needs only the bus.
    None,
}

pub struct Options {
    /// Start sway. Off for tests that need only the bus (P5, P8).
    pub compositor: bool,
    /// The headless output's size in pixels.
    pub output: (u32, u32),
    pub scale: f64,
    /// Xwayland, started by sway on first use.
    pub xwayland: bool,
    /// The input peer's virtual pointer and keyboard.
    pub input: bool,
    /// Lines appended to the sway config.
    pub sway_config: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            compositor: true,
            output: (1280, 800),
            scale: 1.0,
            xwayland: true,
            input: true,
            sway_config: String::new(),
        }
    }
}

/// How long any wait lasts: 10 s times `STRATA_TEST_TIMEOUT_SCALE` (CI sets 3).
pub fn timeout() -> Duration {
    let scale = std::env::var("STRATA_TEST_TIMEOUT_SCALE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|s| *s > 0.0)
        .unwrap_or(1.0);
    Duration::from_secs_f64(10.0 * scale)
}

/// An example of strata-platform, built beside the test binary
/// (`target/<profile>/examples/<name>`). `mise run test-platform` builds them first:
/// `cargo test` leaves examples under hashed names only.
pub fn example(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    let path = exe
        .parent()
        .and_then(|deps| deps.parent())
        .map(|profile| profile.join("examples").join(name))
        .expect("target directory");
    assert!(
        path.is_file(),
        "no example at {}: build it with `cargo build -p strata-platform --examples \
         --all-features`, or run `mise run test-platform`, which does",
        path.display()
    );
    path
}

/// The last `n` lines of a log, headed by its path.
fn tail(path: &Path, n: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    format!("--- {} ---\n{}", path.display(), lines[start..].join("\n"))
}

pub struct Session {
    name: String,
    dir: PathBuf,
    logs: PathBuf,
    icd: Option<OsString>,
    /// The desktop's `DISPLAY`, which the session's must never be.
    outer_display: Option<String>,
    // Dropped in this order: the input peer, sway (its clients go with it), the bus.
    input: Option<Peer>,
    sway: Option<sway::Sway>,
    bus: Bus,
    watchdog: Option<Child>,
}

impl Session {
    /// A session with the defaults: sway, Xwayland, the bus and the input peer.
    pub fn start() -> Session {
        Session::with(Options::default())
    }

    pub fn with(opts: Options) -> Session {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let name = match std::thread::current().name() {
            Some(t) if t != "main" => t.replace("::", "-"),
            _ => format!("session-{n}"),
        };
        let logs = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("session")
            .join(&name);
        let _ = std::fs::remove_dir_all(&logs);
        std::fs::create_dir_all(&logs).expect("session log directory");
        // Short, for the 108-byte limit on Unix socket paths.
        let dir = PathBuf::from(format!("/tmp/strata-session-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .unwrap_or_else(|e| panic!("creating {}: {e}", dir.display()));
        for sub in [
            "home",
            "home/.config",
            "home/.local/share",
            "home/.cache",
            "home/.local/state",
        ] {
            std::fs::create_dir_all(dir.join(sub)).expect("session home");
        }
        let icd = gpu::icd();
        let mut session = Session {
            name,
            outer_display: std::env::var("DISPLAY").ok().filter(|d| !d.is_empty()),
            bus: Bus::start(&dir, &logs, &base_env(&dir, None)),
            dir,
            logs,
            icd,
            input: None,
            sway: None,
            watchdog: None,
        };
        if opts.compositor {
            let renderer = gpu::renderer(session.icd.as_deref());
            let env = base_env(&session.dir, Some(session.bus.address()));
            session.sway = Some(sway::Sway::start(
                &session.dir,
                &session.logs,
                &opts,
                &renderer,
                &env,
            ));
            std::fs::write(
                session.logs.join("session.txt"),
                format!(
                    "renderer {renderer}\nvulkan icd {:?}\nruntime dir {}\n",
                    session.icd,
                    session.dir.display()
                ),
            )
            .expect("session notes");
        }
        session.watchdog = Some(session.watchdog());
        if opts.compositor && opts.input {
            let input = Peer::spawn(&session, Display::Wayland, "input");
            let (w, h) = opts.output;
            let logical = |px: u32| (f64::from(px) / opts.scale).round() as u32;
            input.devices([logical(w), logical(h)]);
            session.input = Some(input);
        }
        session
    }

    /// Stops sway and the bus once this process is gone, in case it dies without dropping
    /// the session (SIGKILL, a test timeout). sway's clients and Xwayland go with sway.
    fn watchdog(&self) -> Child {
        let mut pids = vec![self.bus.pid().to_string()];
        pids.extend(self.sway.as_ref().map(|s| s.pid().to_string()));
        Command::new("sh")
            .arg("-c")
            .arg(r#"p=$1; shift; while kill -0 "$p" 2>/dev/null; do sleep 0.5; done; kill "$@" 2>/dev/null"#)
            .arg("watchdog")
            .arg(std::process::id().to_string())
            .args(pids)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("starting the session watchdog")
    }

    fn sway(&self) -> &sway::Sway {
        self.sway
            .as_ref()
            .expect("this session has no compositor (Options::compositor)")
    }

    /// The environment of a process in this session, built from nothing.
    pub fn env(&self, display: Display) -> Vec<(OsString, OsString)> {
        let mut env: Vec<(OsString, OsString)> = base_env(&self.dir, Some(self.bus.address()))
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        let mut set = |k: &str, v: &OsStr| env.push((k.into(), v.to_owned()));
        match display {
            Display::Wayland => {
                set("WAYLAND_DISPLAY", self.sway().wayland_display().as_ref());
                set("XDG_SESSION_TYPE", "wayland".as_ref());
            }
            Display::X11 => {
                let x = self
                    .sway()
                    .x_display_name()
                    .expect("this session has no Xwayland (Options::xwayland)");
                assert!(
                    self.outer_display.as_deref() != Some(x),
                    "the session's DISPLAY {x} is the desktop's"
                );
                set("DISPLAY", x.as_ref());
                set("XDG_SESSION_TYPE", "x11".as_ref());
            }
            Display::None => {}
        }
        if self.sway.is_some() {
            set("XDG_CURRENT_DESKTOP", "sway".as_ref());
        }
        if let Some(icd) = &self.icd {
            set("VK_ICD_FILENAMES", icd);
            set("VK_DRIVER_FILES", icd);
        }
        set("RUNE_GPU_LOG", "1".as_ref());
        set("RUST_BACKTRACE", "1".as_ref());
        set("STRATA_PEER_DIR", self.logs.as_os_str());
        env
    }

    /// `program` with this session's environment ([`Session::env`]), run from its home.
    pub fn command(&self, program: impl AsRef<OsStr>, display: Display) -> Command {
        let mut cmd = Command::new(program);
        cmd.env_clear()
            .envs(self.env(display))
            .current_dir(self.dir.join("home"));
        cmd
    }

    /// Starts `cmd` (from [`Session::command`]), logging as `<name>.stdout.log` and
    /// `<name>.stderr.log` in the session's log directory.
    pub fn spawn(&self, name: &str, cmd: Command) -> Proc {
        let mut name = name.to_owned();
        let mut n = 1;
        while self.logs.join(format!("{name}.stdout.log")).exists() {
            n += 1;
            name = format!("{name}-{n}");
        }
        Proc::spawn(&name, cmd, &self.logs)
    }

    /// A new peer process on `display`.
    pub fn peer(&self, display: Display) -> Peer {
        let name = match display {
            Display::Wayland => "peer-wayland",
            Display::X11 => "peer-x11",
            Display::None => panic!("a peer needs a display"),
        };
        Peer::spawn(self, display, name)
    }

    /// The session's virtual pointer and keyboard, in layout points.
    pub fn input(&self) -> &Peer {
        self.input
            .as_ref()
            .expect("this session has no input peer (Options::input)")
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    /// Runs sway commands (`[con_id=4] focus`) and panics unless all succeed.
    pub fn sway_command(&self, cmd: &str) {
        self.sway().command(cmd);
    }

    /// Waits for a toplevel of process `pid` with a size.
    pub fn wait_window(&self, pid: u32) -> Window {
        self.wait_window_where(&format!("a window of pid {pid}"), |w| {
            w.pid == pid && w.content.width > 0.0
        })
    }

    fn wait_window_where(&self, what: &str, pred: impl Fn(&Window) -> bool) -> Window {
        let deadline = std::time::Instant::now() + timeout();
        loop {
            let windows = sway::windows(&self.sway().tree());
            if let Some(w) = windows.iter().find(|w| pred(w)) {
                return w.clone();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{}: no {what} in sway's tree, which has {windows:#?}",
                self.name
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Waits until each of `pids` has a toplevel and no two overlap: sway has tiled them side
    /// by side. The clients may not have drawn at their new size yet: sync each one before
    /// sending input.
    pub fn wait_windows(&self, pids: &[u32]) -> Vec<Window> {
        let deadline = std::time::Instant::now() + timeout();
        loop {
            let all = sway::windows(&self.sway().tree());
            let found: Vec<Window> = pids
                .iter()
                .filter_map(|p| all.iter().find(|w| w.pid == *p).cloned())
                .collect();
            let apart = found.len() == pids.len()
                && found.iter().enumerate().all(|(i, a)| {
                    found[i + 1..]
                        .iter()
                        .all(|b| !a.content.overlaps(&b.content))
                });
            if apart {
                return found;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{}: no windows of pids {pids:?} side by side in sway's tree, which has {all:#?}",
                self.name
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Moves the input peer's pointer to `at` until `peer` reports the pointer in its window.
    ///
    /// While sway resizes windows it shows saved snapshots of them, which take no pointer
    /// focus, so a motion then can reach no client, and a press after it goes nowhere. A sync
    /// with each client doesn't prove sway has applied the resize. So move, and move again a
    /// point off, until the peer has the pointer.
    pub fn point_at(&self, peer: &Peer, at: (f64, f64)) {
        let deadline = std::time::Instant::now() + timeout();
        let mut nudge = 0.0;
        loop {
            self.input().pointer_move((at.0 + nudge, at.1 + nudge));
            // An enter from before this motion may still be the peer's last word: let the
            // motion's own enter or leave arrive first.
            std::thread::sleep(Duration::from_millis(50));
            for _ in 0..10 {
                if peer.pointer_inside() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{}: the pointer at {at:?} never reached peer {}",
                self.name,
                peer.pid()
            );
            nudge = 1.0 - nudge;
        }
    }

    /// Gives `window` keyboard focus and waits until sway reports it.
    pub fn focus(&self, window: &Window) -> Window {
        self.sway_command(&format!("[con_id={}] focus", window.id));
        let id = window.id;
        self.wait_window_where("the window focused", |w| w.id == id && w.focused)
    }

    /// The private runtime directory (`XDG_RUNTIME_DIR`).
    pub fn runtime_dir(&self) -> &Path {
        &self.dir
    }

    /// sway's and the bus's process ids.
    pub fn pids(&self) -> Vec<u32> {
        let mut pids = vec![self.bus.pid()];
        pids.extend(self.sway.as_ref().map(sway::Sway::pid));
        pids
    }
}

/// What every process of a session gets: a private home and runtime directory, the session
/// bus, and settings that keep GLib from reaching outside (gvfs, dconf, the a11y bus).
fn base_env(dir: &Path, bus: Option<&str>) -> Vec<(String, String)> {
    let home = dir.join("home");
    let at = |sub: &str| home.join(sub).display().to_string();
    let mut env = vec![
        ("HOME".into(), home.display().to_string()),
        ("XDG_RUNTIME_DIR".into(), dir.display().to_string()),
        ("XDG_CONFIG_HOME".into(), at(".config")),
        ("XDG_DATA_HOME".into(), at(".local/share")),
        ("XDG_CACHE_HOME".into(), at(".cache")),
        ("XDG_STATE_HOME".into(), at(".local/state")),
        (
            "PATH".into(),
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
        ),
        ("LANG".into(), "C.UTF-8".into()),
        ("GIO_USE_VFS".into(), "local".into()),
        ("GVFS_DISABLE_FUSE".into(), "1".into()),
        ("GSETTINGS_BACKEND".into(), "memory".into()),
        ("NO_AT_BRIDGE".into(), "1".into()),
    ];
    if let Some(bus) = bus {
        env.push(("DBUS_SESSION_BUS_ADDRESS".into(), bus.into()));
    }
    env
}

impl Drop for Session {
    fn drop(&mut self) {
        self.input.take();
        if let Some(mut sway) = self.sway.take() {
            sway.stop();
        }
        if let Some(mut w) = self.watchdog.take() {
            let _ = w.kill();
            let _ = w.wait();
        }
        // The bus goes when the struct does; a FUSE mount a GIO service left would keep the
        // runtime directory from going.
        if let Ok(mounts) = std::fs::read_to_string("/proc/self/mountinfo") {
            for point in mounts.lines().filter_map(|l| l.split(' ').nth(4)) {
                if Path::new(point).starts_with(&self.dir) {
                    let _ = Command::new("fusermount3")
                        .args(["-u", "-z", point])
                        .status();
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
        if std::thread::panicking() {
            eprintln!("{}: session logs in {}", self.name, self.logs.display());
            if let Some(sway) = self.logs.join("sway.log").to_str() {
                eprintln!("{}", tail(Path::new(sway), 20));
            }
        }
    }
}
