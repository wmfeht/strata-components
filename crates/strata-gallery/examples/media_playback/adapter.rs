//! The glue between strata-platform's media clock (P7) and C13's `PlaybackClock`: an app copies
//! it as it is. strata-components never depends on strata-platform, so it lives here.
//!
//! - [`ClockAdapter`] is the `PlaybackClock` C13's `MediaState::tick` reads. It holds one
//!   [`Reading`] per frame ([`ClockAdapter::refresh`]), so the position and the state the
//!   player sees agree, and the clip's length, which the decoder knows and the sink doesn't.
//! - [`apply`] carries the player's intents to the sink ([`Transport`]) and says where the
//!   decoder must restart.
//!
//! States map one to one, except two:
//!
//! - **Underrun → Loading** (OWNER ruling): the timeline stops asking for a wake each frame,
//!   and the sink wakes the app when playing resumes (`SinkEvent::Resumed`).
//! - **Failed → Paused**: C13 has no failed state; the host shows the error
//!   (`MediaState::set_error`) and reopens the output.

use std::time::Duration;

use strata_components::media_player::{MediaIntent, PlaybackClock, PlaybackState};
use strata_platform::media::{AudioSink, ClockState, MediaClock, Reading};

/// What the player's play, pause, seek and volume reach: the audio sink, or a test's record.
pub trait Transport {
    fn play(&self);
    fn pause(&self);
    fn seek(&self, at: Duration);
    fn set_volume(&self, volume: f64, muted: bool);
}

impl Transport for AudioSink {
    fn play(&self) {
        AudioSink::play(self);
    }

    fn pause(&self) {
        AudioSink::pause(self);
    }

    fn seek(&self, at: Duration) {
        AudioSink::seek(self, at);
    }

    fn set_volume(&self, volume: f64, muted: bool) {
        AudioSink::set_volume(self, volume, muted);
    }
}

/// C13's state for one of P7's.
pub fn state(state: ClockState) -> PlaybackState {
    match state {
        ClockState::Loading | ClockState::Underrun => PlaybackState::Loading,
        ClockState::Paused | ClockState::Failed => PlaybackState::Paused,
        ClockState::Playing => PlaybackState::Playing,
        ClockState::Seeking => PlaybackState::Seeking,
        ClockState::Ended => PlaybackState::Ended,
    }
}

/// P7's clock as C13 reads it.
#[derive(Clone, Copy, Debug)]
pub struct ClockAdapter {
    reading: Reading,
    duration: Option<Duration>,
}

impl ClockAdapter {
    pub fn new(duration: Option<Duration>) -> ClockAdapter {
        ClockAdapter {
            reading: Reading {
                position: Duration::ZERO,
                state: ClockState::Loading,
                generation: 0,
            },
            duration,
        }
    }

    /// Reads the clock once, before `MediaState::tick`.
    pub fn refresh(&mut self, clock: &impl MediaClock) {
        self.reading = clock.reading();
    }

    pub fn reading(&self) -> Reading {
        self.reading
    }
}

impl PlaybackClock for ClockAdapter {
    fn position(&self) -> Duration {
        self.duration
            .map_or(self.reading.position, |d| self.reading.position.min(d))
    }

    fn duration(&self) -> Option<Duration> {
        self.duration
    }

    fn state(&self) -> PlaybackState {
        state(self.reading.state)
    }
}

/// Applies a player intent to the sink. `Some(at)`: the decoder restarts at `at` (a seek, or
/// a play from the end, which starts again from the beginning); its PCM and frames go from
/// there, after `FramePacer::flush`. The other intents (steps, hand-off, decode size,
/// storyboard) are the host's and do nothing here.
pub fn apply(
    transport: &impl Transport,
    clock: &ClockAdapter,
    intent: &MediaIntent,
) -> Option<Duration> {
    match *intent {
        MediaIntent::Play if clock.reading.state == ClockState::Ended => {
            transport.seek(Duration::ZERO);
            transport.play();
            Some(Duration::ZERO)
        }
        MediaIntent::Play => {
            transport.play();
            None
        }
        MediaIntent::Pause => {
            transport.pause();
            None
        }
        MediaIntent::Seek(at) => {
            transport.seek(at);
            Some(at)
        }
        MediaIntent::SetVolume { volume, muted } => {
            transport.set_volume(f64::from(volume), muted);
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
