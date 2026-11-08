//! The thread that reads a Wayland connection for our queue.
//!
//! On winit's display this is S2's driver C: the thread takes part in libwayland's read
//! protocol (`prepare_read`, poll, `read_events`), so winit can't consume the socket without
//! it, and after each read that queued events it asks the UI thread for a turn and waits for
//! it. A watcher that only polls the fd races winit's reads (S2 Q2). The invariants:
//!
//! - It never waits on the UI thread while it holds a read intent: it reads or cancels first.
//! - After the stop flag, its only libwayland call is `cancel_read` on an intent it already
//!   holds. The stop pipe wakes a poll at once; the gate releases a turn wait at once.
//! - It polls with no timeout, and waits for a turn with none. A reader the app forgot to stop
//!   stays parked when the event loop ends, rather than calling into a display winit freed.
//!   That is narrower, not safe: stopping it before `run` returns is required.
//!
//! On a connection of our own the same loop runs, and dispatches on the thread itself.

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;
use std::thread::JoinHandle;

use wayland_backend::client::WaylandError;
use wayland_client::Connection;

use super::super::gate::Gate;
use super::super::pipe;

pub(super) struct Reader {
    gate: Arc<Gate>,
    stop: Option<OwnedFd>,
    thread: Option<JoinHandle<()>>,
}

impl Reader {
    /// `on_events` runs on the reader thread when our queue has events: after a read that
    /// queued some, or when a prepare finds some already queued. `on_fail` runs once if the
    /// connection fails; the thread then ends.
    pub(super) fn spawn(
        name: &str,
        conn: Connection,
        gate: Arc<Gate>,
        mut on_events: impl FnMut() + Send + 'static,
        on_fail: impl FnOnce(String) + Send + 'static,
    ) -> io::Result<Reader> {
        let (stop_r, stop_w) = pipe::pipe()?;
        let thread_gate = gate.clone();
        let thread = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let gate = thread_gate;
                let failed = loop {
                    if gate.stopped() {
                        break None;
                    }
                    let Some(guard) = conn.prepare_read() else {
                        on_events();
                        continue;
                    };
                    let revents = match pipe::poll(
                        &[
                            (guard.connection_fd(), libc::POLLIN),
                            (stop_r.as_fd(), libc::POLLIN),
                        ],
                        None,
                    ) {
                        Ok(r) => r,
                        Err(e) => break Some(format!("poll: {e}")),
                    };
                    if revents[1] != 0 || gate.stopped() {
                        // Dropping the guard is cancel_read on an intent we hold.
                        drop(guard);
                        break None;
                    }
                    if revents[0] & libc::POLLIN != 0 {
                        match guard.read() {
                            Ok(0) => {}
                            Ok(_) => on_events(),
                            Err(WaylandError::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(e) => break Some(e.to_string()),
                        }
                    } else if revents[0] & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                        drop(guard);
                        break Some("the display hung up".to_owned());
                    }
                };
                if let Some(e) = failed {
                    on_fail(e);
                }
            })?;
        Ok(Reader {
            gate,
            stop: Some(stop_w),
            thread: Some(thread),
        })
    }

    /// Stops the thread and joins it. Call it on the UI thread before winit closes the
    /// display; it returns at once whether the thread is in a poll or waiting for a turn.
    pub(super) fn stop(mut self) {
        self.gate.stop();
        // Closing the write end makes the read end readable (POLLHUP), which ends the poll.
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
