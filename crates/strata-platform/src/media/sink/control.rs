//! The control thread: state changes, flushes, rebuilds, bus messages and the watchdog that
//! sees underruns and stalls.
//!
//! GStreamer's state changes can block on the device (a sound server that is slow to answer),
//! so none runs on the app's thread. The bus's sync handler runs on GStreamer's threads and only
//! forwards messages here, tagged with the generation `appsrc` was flushed to when they were
//! posted: a message from before a flush (or from a pipeline a rebuild replaced) can't land the
//! seek after it or end it. An error from before a flush still fails the pipeline that flush
//! kept; only one from a failed pipeline awaiting its rebuild is dropped.

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
    /// Restart `appsrc` for this generation: flush it, or rebuild the pipeline when one is
    /// owed (`Core::rebuild_owed`). Stale once a later seek came.
    Seek(u64),
    /// A bus message, and the generation `appsrc` was flushed to when it was posted.
    Bus(gst::Message, u64),
    Stop,
    /// Answers once the commands sent before it are handled.
    #[cfg(test)]
    Fence(mpsc::Sender<()>),
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
        starting: None,
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
    /// A fresh output (a new pipeline, or a flushed generation) until its clock first moves:
    /// a device still starting isn't a stall until the start timeout.
    starting: Option<Starting>,
}

/// How long a fresh output has been starting.
#[derive(Default)]
struct Starting {
    /// The time counted so far: loading or seeking with PCM queued, or playing.
    waited: Duration,
    /// When the watchdog last counted, while it counts.
    since: Option<Instant>,
    reported: bool,
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
            Cmd::Seek(generation) => self.seek(generation),
            Cmd::Bus(message, tag) => self.bus(&message, tag),
            Cmd::Stop => unreachable!("handled by run"),
            #[cfg(test)]
            Cmd::Fence(done) => {
                let _ = done.send(());
            }
        }
        self.settle();
    }

    /// Pauses a fresh pipeline (it prerolls on the first PCM) and lets `generation`'s PCM in.
    fn start(&mut self, generation: u64) {
        self.target = None;
        self.starting = Some(Starting::default());
        // Before the state change: a device that fails to open posts its error during it.
        // (Untested: no seam posts during the state change.)
        self.flushed.store(generation, Ordering::SeqCst);
        if self.set_state(gst::State::Paused) {
            self.apply(generation);
        }
    }

    fn seek(&mut self, generation: u64) {
        let rebuild = {
            let mut core = lock(&self.shared.core);
            if core.generation != generation {
                return; // A later seek follows in the queue.
            }
            std::mem::take(&mut core.rebuild_owed)
        };
        if rebuild {
            self.rebuild(generation);
        } else {
            self.flush(generation);
        }
    }

    fn flush(&mut self, generation: u64) {
        self.starting = Some(Starting::default());
        self.set_state(gst::State::Paused);
        self.parts.src.send_event(gst::event::FlushStart::new());
        // reset_time: the new generation's PCM starts at running time 0.
        self.parts.src.send_event(gst::event::FlushStop::new(true));
        // After the flush, unlike `start`: pausing a playing pipeline can preroll the old
        // generation's PCM, and that AsyncDone mustn't land this seek
        // (`a_reading_never_mixes_two_generations` fails about half its runs with the store
        // first). An error the output posts during the pause carries the generation before,
        // and still fails this output (see `bus`).
        self.flushed.store(generation, Ordering::SeqCst);
        self.apply(generation);
    }

    fn rebuild(&mut self, generation: u64) {
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
        // The flush tag is load-bearing: open, push and seek at once, and the first
        // generation's AsyncDone can arrive behind the seek's flush with the seek's own PCM
        // already in, where nothing else tells the two apart.
        let current = tag == core.generation;
        match message.view() {
            gst::MessageView::AsyncDone(_) => {
                let loading = core.phase == Phase::Loading;
                if current
                    && matches!(core.phase, Phase::Loading | Phase::Seeking)
                    && core.applied == Some(core.generation)
                    // Defensive: an output that finishes going to PAUSED without a preroll
                    // has nothing ready to play.
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
                // One from before a seek that owes a rebuild leaves the new output alone: the
                // old pipeline's errors are the failure already reported. (Once the rebuild
                // has run, the ancestor check above drops them.) A plain seek flushes and
                // keeps this pipeline, so an error from before it is this output's: it fails
                // the generation now playing. While failed, `fail` keeps the first failure.
                if !current && core.rebuild_owed {
                    return;
                }
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

    /// Fails the clock and reports it, once: what fails while failed is the same failure
    /// (appsrc stopping on the lost device, say), and adds nothing.
    fn fail(&mut self, event: SinkEvent) {
        {
            let mut core = lock(&self.shared.core);
            if core.phase == Phase::Failed {
                return;
            }
            core.phase = Phase::Failed;
        }
        self.shared.emit(event);
    }

    /// Plays only when ready and wanted; otherwise holds paused. While a rebuild is owed the
    /// pipeline is the failed one: only the rebuild touches it, so a refused state change
    /// there can't fail the generation that rebuild starts.
    fn settle(&mut self) {
        let desired = {
            let core = lock(&self.shared.core);
            if core.rebuild_owed {
                return;
            }
            match core.phase {
                Phase::Failed => return,
                Phase::Running if core.want_play => gst::State::Playing,
                _ => gst::State::Paused,
            }
        };
        if self.target != Some(desired) {
            if desired == gst::State::Playing {
                let mut core = lock(&self.shared.core);
                core.still = None;
                // After an underrun the pipeline paused at the running time it had reached.
                // That can be past the PCM's end (it ran on between two watchdog reads): start
                // again at the end, or what comes next plays late. Short of the end, it plays
                // on from where it paused, so the last PCM before the underrun still plays.
                if let Some(end) = core.resume_from.take()
                    && self
                        .parts
                        .pipeline
                        .start_time()
                        .is_none_or(|paused_at| paused_at > super::clock_time(end))
                {
                    self.parts.pipeline.set_start_time(super::clock_time(end));
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
            Phase::Running => core.want_play,
            Phase::Underrun => true,
            // Waiting for a preroll with PCM in: the start timeout runs.
            Phase::Loading | Phase::Seeking => {
                core.applied == Some(core.generation)
                    && (core.pushed > 0 || core.finished)
                    && self.starting.as_ref().is_some_and(|s| !s.reported)
            }
            Phase::Ended | Phase::Failed => false,
        }
    }

    /// Sees the PCM run out (pausing the pipeline, so PCM pushed later doesn't play late),
    /// enough of it come back, a fresh output that doesn't start, and a device that stopped
    /// taking PCM.
    fn watch(&mut self) {
        if !self.watching() {
            // Time paused doesn't count toward the start timeout.
            if let Some(starting) = &mut self.starting {
                starting.since = None;
            }
            return;
        }
        let mut core = self.shared.settled();
        let relative = core.held;
        let ahead = self
            .shared
            .format
            .duration_of(core.pushed)
            .saturating_sub(relative);
        let at = core.base + relative;
        match core.phase {
            Phase::Loading | Phase::Seeking => {
                drop(core);
                self.count_start(at);
            }
            Phase::Running => {
                if ahead <= UNDERRUN_MARGIN {
                    // Finished: the end comes as EOS.
                    if !core.finished {
                        core.phase = Phase::Underrun;
                        core.still = None;
                        core.resume_from = Some(self.shared.format.duration_of(core.pushed));
                        drop(core);
                        self.shared.emit(SinkEvent::Underrun { at });
                        self.settle();
                    }
                    return;
                }
                // A new generation's clock starts at 0: it moved once it is past it.
                if self.starting.is_some() {
                    if relative.is_zero() {
                        drop(core);
                        self.count_start(at);
                        return;
                    }
                    self.starting = None;
                }
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
            Phase::Ended | Phase::Failed => {}
        }
    }

    /// Counts a watchdog tick toward the start timeout, and reports a start that ran out.
    fn count_start(&mut self, at: Duration) {
        let Some(starting) = &mut self.starting else {
            return;
        };
        let now = Instant::now();
        if let Some(since) = starting.since {
            starting.waited += now - since;
        }
        starting.since = Some(now);
        if !starting.reported && starting.waited >= self.shared.options.start_timeout {
            starting.reported = true;
            self.shared.emit(SinkEvent::Stalled { at });
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
