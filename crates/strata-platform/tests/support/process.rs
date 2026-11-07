//! Processes a session spawns: stdout kept line by line (and logged), stderr logged, killed when
//! dropped.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use serde_json::Value;

use super::timeout;

#[derive(Default)]
struct Lines {
    lines: Vec<String>,
    closed: bool,
}

pub struct Proc {
    name: String,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    out: Arc<(Mutex<Lines>, Condvar)>,
    stdout_log: PathBuf,
    stderr_log: PathBuf,
}

impl Proc {
    pub(super) fn spawn(name: &str, mut cmd: Command, logs: &Path) -> Proc {
        let stdout_log = logs.join(format!("{name}.stdout.log"));
        let stderr_log = logs.join(format!("{name}.stderr.log"));
        let stderr = File::create(&stderr_log)
            .unwrap_or_else(|e| panic!("creating {}: {e}", stderr_log.display()));
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr);
        let mut child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("spawning {name} ({:?}): {e}", cmd.get_program()));
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("piped stdout");
        let out = Arc::new((Mutex::new(Lines::default()), Condvar::new()));
        let reader = out.clone();
        let mut log = File::create(&stdout_log)
            .unwrap_or_else(|e| panic!("creating {}: {e}", stdout_log.display()));
        std::thread::spawn(move || {
            let (lines, ready) = &*reader;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let _ = writeln!(log, "{line}");
                if let Ok(mut l) = lines.lock() {
                    l.lines.push(line);
                }
                ready.notify_all();
            }
            if let Ok(mut l) = lines.lock() {
                l.closed = true;
            }
            ready.notify_all();
        });
        Proc {
            name: name.to_owned(),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            out,
            stdout_log,
            stderr_log,
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.lock().map_or(0, |c| c.id())
    }

    /// Every stdout line so far.
    pub fn lines(&self) -> Vec<String> {
        self.out
            .0
            .lock()
            .map(|l| l.lines.clone())
            .unwrap_or_default()
    }

    /// Waits for the first stdout line, from line `from` on, that `f` maps to a value. Panics
    /// with the process's output once the session timeout passes or stdout closes first.
    pub(super) fn wait_for<T>(
        &self,
        from: usize,
        what: &str,
        mut f: impl FnMut(&str) -> Option<T>,
    ) -> (usize, T) {
        let deadline = Instant::now() + timeout();
        let (lines, ready) = &*self.out;
        let mut seen = from;
        let mut l = lines.lock().expect("process output lock");
        loop {
            while seen < l.lines.len() {
                if let Some(v) = f(&l.lines[seen]) {
                    return (seen, v);
                }
                seen += 1;
            }
            let now = Instant::now();
            if l.closed || now >= deadline {
                let why = if l.closed {
                    "its output closed"
                } else {
                    "timed out"
                };
                drop(l);
                panic!(
                    "{}: waiting for {what}: {why} ({})\n{}",
                    self.name,
                    self.status(),
                    self.tails()
                );
            }
            l = ready
                .wait_timeout(l, deadline - now)
                .expect("process output lock")
                .0;
        }
    }

    /// The first stdout line that is JSON and passes `pred`.
    pub fn wait_json(&self, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
        self.wait_for(0, what, |line| {
            serde_json::from_str::<Value>(line).ok().filter(|v| pred(v))
        })
        .1
    }

    pub(super) fn send_line(&self, line: &str) {
        let mut stdin = self.stdin.lock().expect("stdin lock");
        let ok = stdin
            .as_mut()
            .is_some_and(|s| writeln!(s, "{line}").and_then(|()| s.flush()).is_ok());
        drop(stdin);
        assert!(ok, "{}: its stdin is closed\n{}", self.name, self.tails());
    }

    /// Whether the process still runs, or how it exited, for failure messages.
    fn status(&self) -> String {
        let Ok(mut child) = self.child.lock() else {
            return "status unknown".into();
        };
        // A process whose output just closed may take a moment to exit.
        for _ in 0..50 {
            match child.try_wait() {
                Ok(Some(status)) => return format!("exited: {status}"),
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(e) => return format!("status unknown: {e}"),
            }
        }
        "still running".into()
    }

    /// Waits for the process to exit, within the session timeout.
    pub fn wait_exit(&mut self) -> ExitStatus {
        let deadline = Instant::now() + timeout();
        loop {
            let status = self.child.lock().expect("child lock").try_wait();
            match status {
                Ok(Some(status)) => return status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Ok(None) => panic!(
                    "{}: still running after the timeout\n{}",
                    self.name,
                    self.tails()
                ),
                Err(e) => panic!("{}: {e}", self.name),
            }
        }
    }

    /// The last lines of stdout and stderr, for failure messages.
    pub(super) fn tails(&self) -> String {
        format!(
            "{}\n{}",
            super::tail(&self.stdout_log, 30),
            super::tail(&self.stderr_log, 30)
        )
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if let Ok(mut stdin) = self.stdin.lock() {
            stdin.take();
        }
        if let Ok(child) = self.child.get_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
