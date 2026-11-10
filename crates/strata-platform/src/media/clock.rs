//! The playback clock video follows, and a wall clock for clips without sound.

use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// What playback is doing, as a clock reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ClockState {
    /// Nothing to play yet: the output opens, or waits for its first data.
    #[default]
    Loading,
    Paused,
    Playing,
    /// A seek or flush runs: [`Reading::position`] is its target, and it lands once the new
    /// data is ready.
    Seeking,
    /// Playback is wanted but the data ran out before the end: the clock holds where the data
    /// stopped until more arrives.
    Underrun,
    /// Playback reached the end of the stream and stopped there.
    Ended,
    /// The output failed (a device went away, say): the clock holds its last position.
    Failed,
}

/// One read of a clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    /// Where playback is, in the media's time. While seeking, the seek's target.
    pub position: Duration,
    pub state: ClockState,
    /// Counts seeks and flushes: frames from before the latest one are stale.
    pub generation: u64,
}

/// A clock playback follows: [`AudioSink`](super::AudioSink)'s [`SinkClock`](super::SinkClock)
/// for clips with sound, [`WallClock`] for clips without.
pub trait MediaClock {
    fn reading(&self) -> Reading;
}

impl<C: MediaClock + ?Sized> MediaClock for &C {
    fn reading(&self) -> Reading {
        (**self).reading()
    }
}

/// A clock on the system's monotonic time, for clips without sound.
///
/// It mirrors the sink's states: [`ClockState::Loading`] until [`WallClock::ready`] (the first
/// frame is there), [`ClockState::Seeking`] from a seek until the next `ready`, and
/// [`ClockState::Ended`] once playing reaches [`WallClock::set_end`]'s end. Each call takes
/// the time it happens at, `Instant::now()` in an app. The only change it makes on its own is
/// reaching the end, while playing, when the host is polling each frame anyway.
#[derive(Clone, Debug)]
pub struct WallClock {
    /// Where playback stood at `since`.
    at: Duration,
    since: Instant,
    playing: bool,
    /// A start or a seek waiting for its first frame.
    waiting: Option<ClockState>,
    end: Option<Duration>,
    generation: u64,
}

impl WallClock {
    /// A clock at 0, loading, paused.
    pub fn new(now: Instant) -> WallClock {
        WallClock {
            at: Duration::ZERO,
            since: now,
            playing: false,
            waiting: Some(ClockState::Loading),
            end: None,
            generation: 0,
        }
    }

    /// The first frame (of the clip, or since a seek) is there: loading or seeking ends, and
    /// a play asked for meanwhile starts now.
    pub fn ready(&mut self, now: Instant) {
        if self.waiting.take().is_some() {
            self.since = now;
        }
    }

    pub fn play(&mut self, now: Instant) {
        if self.playing || self.ended_at(now) {
            return;
        }
        self.at = self.position_at(now);
        self.since = now;
        self.playing = true;
    }

    pub fn pause(&mut self, now: Instant) {
        self.at = self.position_at(now);
        self.since = now;
        self.playing = false;
    }

    /// Moves to `at`: [`ClockState::Seeking`] there until [`WallClock::ready`], playing on
    /// afterwards if it was playing.
    pub fn seek(&mut self, at: Duration, now: Instant) {
        let at = self.end.map_or(at, |end| at.min(end));
        self.at = at;
        self.since = now;
        self.waiting = Some(ClockState::Seeking);
        self.generation += 1;
    }

    /// The clip's length, once known: playing stops there.
    pub fn set_end(&mut self, end: Option<Duration>) {
        self.end = end;
    }

    /// The reading at `now`.
    pub fn reading_at(&self, now: Instant) -> Reading {
        let state = match self.waiting {
            Some(waiting) => waiting,
            None if self.ended_at(now) => ClockState::Ended,
            None if self.playing => ClockState::Playing,
            None => ClockState::Paused,
        };
        Reading {
            position: self.position_at(now),
            state,
            generation: self.generation,
        }
    }

    fn ended_at(&self, now: Instant) -> bool {
        self.waiting.is_none()
            && self.playing
            && self.end.is_some_and(|end| self.position_at(now) >= end)
    }

    fn position_at(&self, now: Instant) -> Duration {
        let moving = self.playing && self.waiting.is_none();
        let position = if moving {
            self.at + now.saturating_duration_since(self.since)
        } else {
            self.at
        };
        self.end.map_or(position, |end| position.min(end))
    }
}

impl MediaClock for WallClock {
    fn reading(&self) -> Reading {
        self.reading_at(Instant::now())
    }
}
