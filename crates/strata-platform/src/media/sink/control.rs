//! The control thread: state changes, flushes, rebuilds, bus messages and the watchdog that
//! sees underruns and stalls.
//!
//! GStreamer's state changes can block on the device (a sound server that is slow to answer),
//! so none runs on the app's thread. The bus's sync handler runs on GStreamer's threads and only
//! forwards messages here, tagged with the generation `appsrc` was flushed to when they were
//! posted: a message from before a flush can't land the seek after it.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;

use super::pipeline::Parts;
use super::{Phase, RESUME_LEAD, Shared, SinkEvent, THREAD_NAME, UNDERRUN_MARGIN, lock, send_eos};

/// How often the watchdog reads the clock while playing.
const TICK: Duration = Duration::from_millis(20);

pub(super) enum Cmd {
    /// Bring the pipeline's state in line with what's wanted.
    Settle,
    Flush(u64),
    Rebuild(u64),
    /// A bus message, and the generation `appsrc` was flushed to when it was posted.
    Bus(gst::Message, u64),
    Stop,
}

pub(super) fn spawn(shared: Arc<Shared>, parts: Parts) -> io::Result<mpsc::Sender<Cmd>> {
    let (tx, rx) = mpsc::channel();
    let flushed = Arc::new(AtomicU64::new(u64::MAX));
    install_bus(&parts, &tx, &flushed);
    let mut control = Control {
        shared,
        parts,
        tx: tx.clone(),
        flushed,
        target: None,
        moved_from: None,
    };
    std::thread::Builder::new()
        .name(THREAD_NAME.into())
        .spawn(move || control.run(rx))?;
    Ok(tx)
}

fn install_bus(parts: &Parts, tx: &mpsc::Sender<Cmd>, flushed: &Arc<AtomicU64>) {
    let Some(bus) = parts.pipeline.bus() else {
        return;
    };
    let (tx, flushed) = (tx.clone(), Arc::clone(flushed));
    bus.set_sync_handler(move |_, message| {
        use gst::MessageView::{AsyncDone, Eos, Error};
        if matches!(message.view(), AsyncDone(_) | Eos(_) | Error(_)) {
            let tag = flushed.load(Ordering::SeqCst);
            let _ = tx.send(Cmd::Bus(message.clone(), tag));
        }
        gst::BusSyncReply::Drop
    });
}

struct Control {
    shared: Arc<Shared>,
    parts: Parts,
    tx: mpsc::Sender<Cmd>,
    flushed: Arc<AtomicU64>,
    /// The state last asked of the pipeline.
    target: Option<gst::State>,
    /// Where the clock stood when playing last started, until it first moves: a device still
    /// starting isn't a stall.
    moved_from: Option<Duration>,
}

impl Control {
    fn run(&mut self, rx: mpsc::Receiver<Cmd>) {
        self.start(0);
        loop {
            let next = if self.watching() {
                rx.recv_timeout(TICK)
            } else {
                rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            };
            match next {
                Ok(Cmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(cmd) => self.handle(cmd),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            self.watch();
        }
        self.close();
    }

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Settle => {}
            Cmd::Flush(generation) => self.flush(generation),
            Cmd::Rebuild(generation) => self.rebuild(generation),
            Cmd::Bus(message, tag) => self.bus(&message, tag),
            Cmd::Stop => unreachable!("handled by run"),
        }
        self.settle();
    }

    /// Pauses a fresh pipeline (it prerolls on the first PCM) and lets `generation`'s PCM in.
    fn start(&mut self, generation: u64) {
        self.target = None;
        if self.set_state(gst::State::Paused) {
            self.flushed.store(generation, Ordering::SeqCst);
            self.apply(generation);
        }
    }

    fn flush(&mut self, generation: u64) {
        if lock(&self.shared.core).generation != generation {
            return; // A later seek follows in the queue.
        }
        self.set_state(gst::State::Paused);
        self.parts.src.send_event(gst::event::FlushStart::new());
        // reset_time: the new generation's PCM starts at running time 0.
        self.parts.src.send_event(gst::event::FlushStop::new(true));
        self.flushed.store(generation, Ordering::SeqCst);
        self.apply(generation);
    }

    fn rebuild(&mut self, generation: u64) {
        if lock(&self.shared.core).generation != generation {
            return;
        }
        self.teardown();
        let parts = match Parts::build(&self.shared.format, &self.shared.options) {
            Ok(parts) => parts,
            Err(e) => {
                return self.fail(SinkEvent::Error {
                    message: e.to_string(),
                });
            }
        };
        {
            let mut core = lock(&self.shared.core);
            parts.apply_gain(core.volume, core.muted);
            core.parts = Some(parts.clone());
        }
        install_bus(&parts, &self.tx, &self.flushed);
        self.parts = parts;
        self.start(generation);
    }

    /// Moves the PCM pushed meanwhile into `appsrc`, in order, then the end of the stream if
    /// it came.
    fn apply(&self, generation: u64) {
        let mut core = lock(&self.shared.core);
        if core.generation != generation {
            return;
        }
        core.applied = Some(generation);
        for buffer in std::mem::take(&mut core.pending) {
            let _ = self.parts.src.push_buffer(buffer);
        }
        if core.finished {
            send_eos(&mut core);
        }
    }

    fn bus(&mut self, message: &gst::Message, tag: u64) {
        // A message from a pipeline a rebuild replaced.
        if !message
            .src()
            .is_some_and(|src| src.has_as_ancestor(&self.parts.pipeline))
        {
            return;
        }
        let mut core = lock(&self.shared.core);
        let current = tag == core.generation;
        match message.view() {
            gst::MessageView::AsyncDone(_) => {
                let loading = core.phase == Phase::Loading;
                if current
                    && matches!(core.phase, Phase::Loading | Phase::Seeking)
                    && core.applied == Some(core.generation)
                    && (core.pushed > 0 || core.finished)
                {
                    core.phase = Phase::Running;
                    let at = core.base;
                    drop(core);
                    self.shared.emit(if loading {
                        SinkEvent::Ready
                    } else {
                        SinkEvent::SeekLanded { at }
                    });
                }
            }
            gst::MessageView::Eos(_) => {
                if current && core.phase != Phase::Failed {
                    core.phase = Phase::Ended;
                    let at = core.base + self.shared.format.duration_of(core.pushed);
                    drop(core);
                    self.shared.emit(SinkEvent::Ended { at });
                }
            }
            gst::MessageView::Error(error) => {
                drop(core);
                let lost = self.parts.posted_by_output(message)
                    && error.error().is::<gst::ResourceError>();
                let text = error.error().to_string();
                let text = match error.debug() {
                    Some(debug) if !debug.is_empty() => format!("{text} ({debug})"),
                    _ => text,
                };
                self.fail(if lost {
                    SinkEvent::DeviceLost { message: text }
                } else {
                    SinkEvent::Error { message: text }
                });
            }
            _ => {}
        }
    }

    fn fail(&mut self, event: SinkEvent) {
        lock(&self.shared.core).phase = Phase::Failed;
        self.shared.emit(event);
    }

    /// Plays only when ready and wanted; otherwise holds paused.
    fn settle(&mut self) {
        let desired = {
            let core = lock(&self.shared.core);
            match core.phase {
                Phase::Failed => return,
                Phase::Running if core.want_play => gst::State::Playing,
                _ => gst::State::Paused,
            }
        };
        if self.target != Some(desired) {
            if desired == gst::State::Playing {
                self.moved_from = Some(self.shared.relative().1);
                let mut core = lock(&self.shared.core);
                core.still = None;
                // The running time went on past the PCM before the watchdog paused it: start
                // again where the PCM stopped, or what comes next plays late.
                if let Some(from) = core.resume_from.take() {
                    self.parts.pipeline.set_start_time(super::clock_time(from));
                }
            }
            self.set_state(desired);
        }
    }

    fn set_state(&mut self, state: gst::State) -> bool {
        self.target = Some(state);
        match self.parts.pipeline.set_state(state) {
            Ok(_) => true,
            Err(e) => {
                self.fail(SinkEvent::Error {
                    message: format!("the audio output didn't change state ({e})"),
                });
                false
            }
        }
    }

    fn watching(&self) -> bool {
        let core = lock(&self.shared.core);
        match core.phase {
            Phase::Running => core.want_play && !core.finished,
            Phase::Underrun => true,
            _ => false,
        }
    }

    /// Sees the PCM run out (pausing the pipeline, so PCM pushed later doesn't play late),
    /// enough of it come back, and a device that stopped taking it.
    fn watch(&mut self) {
        if !self.watching() {
            return;
        }
        let (generation, relative) = self.shared.relative();
        let mut core = lock(&self.shared.core);
        if core.generation != generation {
            return;
        }
        let ahead = self
            .shared
            .format
            .duration_of(core.pushed)
            .saturating_sub(relative);
        let at = core.base + relative;
        match core.phase {
            Phase::Running if core.want_play && !core.finished => {
                if ahead <= UNDERRUN_MARGIN {
                    core.phase = Phase::Underrun;
                    core.still = None;
                    core.resume_from = Some(self.shared.format.duration_of(core.pushed));
                    drop(core);
                    self.shared.emit(SinkEvent::Underrun { at });
                    self.settle();
                    return;
                }
                if self.moved_from.is_some_and(|from| from == relative) {
                    return;
                }
                self.moved_from = None;
                let stall_after = self.shared.options.stall_after;
                match &mut core.still {
                    Some((still_at, since, reported)) if *still_at == relative => {
                        if !*reported && since.elapsed() >= stall_after {
                            *reported = true;
                            drop(core);
                            self.shared.emit(SinkEvent::Stalled { at });
                        }
                    }
                    still => *still = Some((relative, Instant::now(), false)),
                }
            }
            Phase::Underrun => {
                let lead = RESUME_LEAD.min(self.shared.options.lookahead / 2);
                if core.finished || ahead >= lead {
                    core.phase = Phase::Running;
                    drop(core);
                    self.shared.emit(SinkEvent::Resumed { at });
                    self.settle();
                }
            }
            _ => {}
        }
    }

    fn teardown(&mut self) {
        let _ = self.parts.pipeline.set_state(gst::State::Null);
        if let Some(bus) = self.parts.pipeline.bus() {
            bus.unset_sync_handler();
        }
        self.target = None;
    }

    fn close(&mut self) {
        self.teardown();
        lock(&self.shared.core).parts = None;
    }
}
