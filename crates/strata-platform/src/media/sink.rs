//! The audio sink: interleaved PCM into GStreamer's `appsrc`, and the clock it plays on.

use std::fmt;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;
use rune::Waker;

use super::clock::{ClockState, MediaClock, Reading};
use super::format::{FormatError, PcmFormat};

mod control;
mod pipeline;
#[cfg(test)]
mod tests;

use control::Cmd;
use pipeline::Parts;

/// The control thread's name.
pub const THREAD_NAME: &str = "media-audio";

/// The clock counts as run dry when it is this close to the end of the PCM pushed.
const UNDERRUN_MARGIN: Duration = Duration::from_millis(10);
/// After an underrun, playback resumes once this much PCM is queued ahead (or the stream is
/// finished), so it doesn't stutter through one chunk at a time.
const RESUME_LEAD: Duration = Duration::from_millis(200);

/// Where the sound goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Output {
    /// The desktop's default output: GStreamer's `autoaudiosink`, which picks PipeWire,
    /// PulseAudio or ALSA.
    #[default]
    Default,
    /// Nowhere, at the real rate (`fakesink sync=true`): for headless runs and tests. It opens
    /// no audio server.
    Null,
}

/// How an [`AudioSink`] opens.
#[derive(Clone, Debug)]
pub struct SinkOptions {
    output: Output,
    lookahead: Duration,
    stall_after: Duration,
    volume: f64,
    muted: bool,
    /// Tests drive the pipeline from a manual clock.
    #[cfg(test)]
    clock: Option<gst::Clock>,
}

impl Default for SinkOptions {
    fn default() -> SinkOptions {
        SinkOptions {
            output: Output::Default,
            lookahead: Duration::from_millis(2_200),
            stall_after: Duration::from_secs(3),
            volume: 1.0,
            muted: false,
            #[cfg(test)]
            clock: None,
        }
    }
}

impl SinkOptions {
    pub fn output(mut self, output: Output) -> SinkOptions {
        self.output = output;
        self
    }

    /// How far ahead of the clock [`AudioSink::push`] may queue PCM. 2.2 s by default, which
    /// covers a decoder that sends 2 s ahead of its video.
    pub fn lookahead(mut self, lookahead: Duration) -> SinkOptions {
        self.lookahead = lookahead.max(Duration::from_millis(50));
        self
    }

    /// How long the clock may stand still while playing with PCM queued before
    /// [`SinkEvent::Stalled`]. 3 s by default.
    pub fn stall_after(mut self, stall_after: Duration) -> SinkOptions {
        self.stall_after = stall_after;
        self
    }

    /// The volume it opens at ([`AudioSink::set_volume`]).
    pub fn volume(mut self, volume: f64, muted: bool) -> SinkOptions {
        self.volume = clamp_volume(volume);
        self.muted = muted;
        self
    }

    #[cfg(test)]
    pub(crate) fn clock(mut self, clock: gst::Clock) -> SinkOptions {
        self.clock = Some(clock);
        self
    }
}

/// Something the clock did on its own. Each one wakes the app (the waker given to
/// [`AudioSink::open`]) once; [`AudioSink::drain_events`] takes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SinkEvent {
    /// The first PCM is ready: [`ClockState::Loading`] ends.
    Ready,
    /// A seek, flush or reopen landed at `at`: [`ClockState::Seeking`] ends.
    SeekLanded { at: Duration },
    /// The PCM ran out at `at` before the end: [`ClockState::Underrun`] until enough arrives.
    Underrun { at: Duration },
    /// Enough PCM arrived after an underrun: playing again from `at`.
    Resumed { at: Duration },
    /// The clock stood still at `at` for the stall time while PCM was queued: the device
    /// stopped taking it. The sink leaves the pipeline as it is; a restart (a flush) is the
    /// app's call.
    Stalled { at: Duration },
    /// The last PCM played: [`ClockState::Ended`] at `at`.
    Ended { at: Duration },
    /// The output device failed or went away: [`ClockState::Failed`] until
    /// [`AudioSink::reopen`] (or a seek) opens a new one.
    DeviceLost { message: String },
    /// Something else failed in the pipeline: [`ClockState::Failed`], as for a lost device.
    Error { message: String },
}

/// Why [`AudioSink::open`] failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaError {
    /// GStreamer didn't start.
    Init(String),
    /// A GStreamer element isn't installed: `appsrc`, `audioconvert`, `audioresample` and
    /// `volume` are in gst-plugins-base, `autoaudiosink` in gst-plugins-good.
    MissingElement(&'static str),
    Format(FormatError),
    /// The elements wouldn't link or take the format.
    Pipeline(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaError::Init(e) => write!(f, "GStreamer didn't start: {e}"),
            MediaError::MissingElement(e) => write!(f, "the GStreamer element {e} isn't installed"),
            MediaError::Format(e) => e.fmt(f),
            MediaError::Pipeline(e) => write!(f, "the audio pipeline didn't build: {e}"),
        }
    }
}

impl std::error::Error for MediaError {}

/// Why [`AudioSink::push`] refused PCM. Nothing was queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushError {
    Empty,
    /// The bytes aren't a whole number of frames.
    PartialFrame,
    /// The lookahead is full: push again once the clock moves ([`AudioSink::room`]).
    Full,
    /// [`AudioSink::finish`] came first: seek to push again.
    Finished,
    /// The output failed ([`ClockState::Failed`]).
    Failed,
}

impl fmt::Display for PushError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PushError::Empty => "no PCM",
            PushError::PartialFrame => "PCM that isn't a whole number of frames",
            PushError::Full => "the PCM lookahead is full",
            PushError::Finished => "the stream is finished",
            PushError::Failed => "the audio output failed",
        })
    }
}

impl std::error::Error for PushError {}

/// Plays interleaved PCM and keeps the clock video follows.
///
/// Each seek starts a generation: its PCM's first frame plays at the seek's target, and the
/// clock reads the target plus how far the device has played into it. The device's own clock
/// drives it, so what the clock says is what is heard.
///
/// No call blocks on the device: state changes run on a control thread ([`THREAD_NAME`]). The
/// handle is `Send`; [`AudioSink::clock`] gives a reader for any thread. Dropping the sink stops
/// the sound, and its clock then holds where it was, paused.
pub struct AudioSink {
    shared: Arc<Shared>,
    control: mpsc::Sender<Cmd>,
}

/// A reader of an [`AudioSink`]'s clock, for any thread.
#[derive(Clone)]
pub struct SinkClock {
    shared: Arc<Shared>,
}

struct Shared {
    core: Mutex<Core>,
    events: Mutex<Vec<SinkEvent>>,
    waker: Waker,
    format: PcmFormat,
    options: SinkOptions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Before the first generation's PCM is ready.
    Loading,
    /// After a seek, before its PCM is ready.
    Seeking,
    /// Ready: playing or paused, as `want_play` says.
    Running,
    Underrun,
    Ended,
    Failed,
}

struct Core {
    generation: u64,
    /// The media time this generation's first frame plays at.
    base: Duration,
    /// Frames pushed in this generation.
    pushed: u64,
    /// The last position read into this generation, so the clock never steps back.
    held: Duration,
    phase: Phase,
    want_play: bool,
    finished: bool,
    eos_sent: bool,
    /// The generation `appsrc` is flushed to. Until it is this one, PCM waits in `pending`.
    applied: Option<u64>,
    pending: Vec<gst::Buffer>,
    parts: Option<Parts>,
    volume: f64,
    muted: bool,
    /// Where the clock stood still, since when, and whether it was reported.
    still: Option<(Duration, Instant, bool)>,
    /// After an underrun, where the PCM ran out: playing resumes at that running time.
    resume_from: Option<Duration>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn clamp_volume(volume: f64) -> f64 {
    if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

impl AudioSink {
    /// Opens a sink for `format`, paused and loading. `waker` is called once for each
    /// [`SinkEvent`]: pass Rune's setup waker (or one that calls `AppHandle::wake`).
    pub fn open(
        format: PcmFormat,
        options: SinkOptions,
        waker: Waker,
    ) -> Result<AudioSink, MediaError> {
        format.validate().map_err(MediaError::Format)?;
        gst::init().map_err(|e| MediaError::Init(e.to_string()))?;
        let parts = Parts::build(&format, &options)?;
        parts.apply_gain(options.volume, options.muted);
        let shared = Arc::new(Shared {
            core: Mutex::new(Core {
                generation: 0,
                base: Duration::ZERO,
                pushed: 0,
                held: Duration::ZERO,
                phase: Phase::Loading,
                want_play: false,
                finished: false,
                eos_sent: false,
                applied: None,
                pending: Vec::new(),
                parts: Some(parts.clone()),
                volume: options.volume,
                muted: options.muted,
                still: None,
                resume_from: None,
            }),
            events: Mutex::new(Vec::new()),
            waker,
            format,
            options,
        });
        let control = control::spawn(Arc::clone(&shared), parts)
            .map_err(|e| MediaError::Pipeline(e.to_string()))?;
        Ok(AudioSink { shared, control })
    }

    pub fn format(&self) -> PcmFormat {
        self.shared.format
    }

    pub fn clock(&self) -> SinkClock {
        SinkClock {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Queues interleaved PCM after what was pushed before: since the last seek, the PCM
    /// plays from the seek's target on, frame after frame.
    pub fn push(&self, pcm: Vec<u8>) -> Result<(), PushError> {
        let format = self.shared.format;
        if pcm.is_empty() {
            return Err(PushError::Empty);
        }
        if !pcm.len().is_multiple_of(format.frame_bytes()) {
            return Err(PushError::PartialFrame);
        }
        if self.shared.queued() >= self.shared.options.lookahead {
            return Err(PushError::Full);
        }
        let frames = (pcm.len() / format.frame_bytes()) as u64;
        let mut core = lock(&self.shared.core);
        if core.phase == Phase::Failed {
            return Err(PushError::Failed);
        }
        if core.finished {
            return Err(PushError::Finished);
        }
        let start = format.duration_of(core.pushed);
        let end = format.duration_of(core.pushed + frames);
        let mut buffer = gst::Buffer::from_mut_slice(pcm);
        {
            let b = buffer.get_mut().expect("a new buffer is writable");
            b.set_pts(clock_time(start));
            b.set_duration(clock_time(end - start));
        }
        core.pushed += frames;
        if core.applied == Some(core.generation)
            && let Some(parts) = &core.parts
        {
            // A refusal here means a flush is starting, which drops this PCM anyway.
            let _ = parts.src.push_buffer(buffer);
        } else {
            core.pending.push(buffer);
        }
        Ok(())
    }

    /// How much more PCM fits in the lookahead now. It grows only while playing.
    pub fn room(&self) -> Duration {
        self.shared
            .options
            .lookahead
            .saturating_sub(self.shared.queued())
    }

    /// No more PCM in this generation: once the last pushed plays, the clock is
    /// [`ClockState::Ended`] ([`SinkEvent::Ended`]).
    pub fn finish(&self) {
        let mut core = lock(&self.shared.core);
        if core.finished {
            return;
        }
        core.finished = true;
        if core.applied == Some(core.generation) {
            send_eos(&mut core);
        }
    }

    /// Plays: at once when ready, or once loading or a seek lands. Ended stays ended until a
    /// seek.
    pub fn play(&self) {
        lock(&self.shared.core).want_play = true;
        let _ = self.control.send(Cmd::Settle);
    }

    pub fn pause(&self) {
        {
            let mut core = lock(&self.shared.core);
            core.want_play = false;
            if core.phase == Phase::Underrun {
                core.phase = Phase::Running;
            }
        }
        let _ = self.control.send(Cmd::Settle);
    }

    /// Drops the PCM queued and starts a generation at `at`: [`ClockState::Seeking`] there
    /// until the PCM pushed next is ready ([`SinkEvent::SeekLanded`]). Playing or paused stays
    /// as it was. After a failure this opens a new output, as [`AudioSink::reopen`] does.
    pub fn seek(&self, at: Duration) {
        let mut core = lock(&self.shared.core);
        let rebuild = core.phase == Phase::Failed;
        core.generation += 1;
        core.base = at;
        core.pushed = 0;
        core.held = Duration::ZERO;
        core.phase = Phase::Seeking;
        core.finished = false;
        core.eos_sent = false;
        core.applied = None;
        core.pending.clear();
        core.still = None;
        core.resume_from = None;
        let generation = core.generation;
        drop(core);
        let _ = self.control.send(if rebuild {
            Cmd::Rebuild(generation)
        } else {
            Cmd::Flush(generation)
        });
    }

    /// Drops the PCM queued and starts again where the clock is: a seek to its position, for
    /// a decoder that restarts there.
    pub fn flush(&self) {
        self.seek(self.shared.reading().position);
    }

    /// Opens a new output where the clock stopped, after [`SinkEvent::DeviceLost`]: a seek to
    /// its position on a fresh pipeline. Push from that position on.
    pub fn reopen(&self) {
        let at = self.shared.reading().position;
        lock(&self.shared.core).phase = Phase::Failed;
        self.seek(at);
    }

    /// Sets the volume (0.0 to 1.0) and mute, at once.
    pub fn set_volume(&self, volume: f64, muted: bool) {
        let mut core = lock(&self.shared.core);
        core.volume = clamp_volume(volume);
        core.muted = muted;
        if let Some(parts) = &core.parts {
            parts.apply_gain(core.volume, muted);
        }
    }

    /// The events since the last call, oldest first.
    pub fn drain_events(&self) -> Vec<SinkEvent> {
        std::mem::take(&mut *lock(&self.shared.events))
    }

    #[cfg(test)]
    fn sink_element(&self) -> gst::Element {
        lock(&self.shared.core)
            .parts
            .as_ref()
            .expect("an output")
            .sink
            .clone()
    }
}

impl Drop for AudioSink {
    fn drop(&mut self) {
        lock(&self.shared.core).want_play = false;
        let _ = self.control.send(Cmd::Stop);
    }
}

impl fmt::Debug for AudioSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioSink")
            .field("format", &self.shared.format)
            .field("reading", &self.shared.reading())
            .finish()
    }
}

impl MediaClock for SinkClock {
    fn reading(&self) -> Reading {
        self.shared.reading()
    }
}

impl MediaClock for AudioSink {
    fn reading(&self) -> Reading {
        self.shared.reading()
    }
}

impl fmt::Debug for SinkClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SinkClock({:?})", self.shared.reading())
    }
}

impl Shared {
    fn emit(&self, event: SinkEvent) {
        lock(&self.events).push(event);
        (self.waker)();
    }

    /// How far into the current generation the device has played, read now. The query runs
    /// outside the lock, since a device's position can take its own locks.
    fn relative(&self) -> (u64, Duration) {
        let (generation, sink) = {
            let core = lock(&self.core);
            let sink = match core.phase {
                Phase::Running | Phase::Underrun => core.parts.as_ref().map(|p| p.sink.clone()),
                _ => None,
            };
            (core.generation, sink)
        };
        let queried = sink.and_then(|s| s.query_position::<gst::ClockTime>());
        let mut core = lock(&self.core);
        let end = self.format.duration_of(core.pushed);
        if core.generation == generation
            && let Some(t) = queried
        {
            let t = Duration::from_nanos(t.nseconds()).min(end);
            core.held = core.held.max(t);
        }
        if core.phase == Phase::Ended {
            core.held = end;
        }
        (core.generation, core.held)
    }

    fn reading(&self) -> Reading {
        let (_, relative) = self.relative();
        let core = lock(&self.core);
        let state = match core.phase {
            Phase::Loading => ClockState::Loading,
            Phase::Seeking => ClockState::Seeking,
            Phase::Running if core.want_play => ClockState::Playing,
            Phase::Running => ClockState::Paused,
            Phase::Underrun if core.want_play => ClockState::Underrun,
            Phase::Underrun => ClockState::Paused,
            Phase::Ended => ClockState::Ended,
            Phase::Failed => ClockState::Failed,
        };
        Reading {
            position: core.base + relative,
            state,
            generation: core.generation,
        }
    }

    /// PCM pushed but not yet played.
    fn queued(&self) -> Duration {
        let (_, relative) = self.relative();
        let core = lock(&self.core);
        self.format
            .duration_of(core.pushed)
            .saturating_sub(relative)
    }
}

/// Ends the stream in `appsrc`, after the PCM pushed.
fn send_eos(core: &mut Core) {
    if core.eos_sent {
        return;
    }
    if let Some(parts) = &core.parts {
        let _ = parts.src.end_of_stream();
        core.eos_sent = true;
    }
}

fn clock_time(d: Duration) -> gst::ClockTime {
    gst::ClockTime::from_nseconds(u64::try_from(d.as_nanos()).unwrap_or(u64::MAX - 1))
}
