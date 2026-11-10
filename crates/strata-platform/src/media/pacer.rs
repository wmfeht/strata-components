//! Picks the video frame a clock is at and writes it into a `rune::Pixels`.

use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

use rune::Pixels;

use super::clock::{ClockState, MediaClock};

#[cfg(test)]
mod tests;

/// A decoded frame: straight-alpha sRGB RGBA, rows top first, shown from `pts` in the media's
/// time (the clock's [`Reading::position`](super::Reading::position)).
#[derive(Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub pts: Duration,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VideoFrame({:?}, {}x{})",
            self.pts, self.width, self.height
        )
    }
}

/// What [`FramePacer::present`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presented {
    /// This frame went to the `Pixels`, after skipping `dropped` frames whose time had passed.
    Shown { pts: Duration, dropped: u32 },
    /// The frame on screen stays: the next one isn't due yet.
    Held,
    /// The frame on screen stays past its time while the clock plays: the next one should be
    /// showing but hasn't arrived, so the picture repeats. Paused, ended, or after
    /// [`FramePacer::finish`] (the clip's last frame, its sound playing on), a frame past its
    /// time is [`Presented::Held`].
    Repeated,
    /// Nothing has shown yet and nothing is queued.
    Empty,
}

/// Counts since the pacer was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacerStats {
    pub shown: u64,
    /// Frames skipped because a later one was already due.
    pub dropped: u64,
    /// Presents that kept a frame past its time ([`Presented::Repeated`]).
    pub repeated: u64,
}

/// Why [`FramePacer::push`] gave a frame back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The queue holds its capacity: push again once a present has taken one.
    Full,
    /// The frame's time is before the last one queued; frames come in order.
    OutOfOrder,
    /// The RGBA isn't `width * height * 4` bytes.
    BadSize,
}

/// Queues time-stamped frames and, each time the host presents, writes the one the clock is
/// at into its [`Pixels`].
///
/// [`Pixels::set`] wakes only the windows that show the picture and needs no view commit, so a
/// host presents from its `poll` each frame while playing (C13's player asks for that wake) and
/// returns `false` from `poll` when nothing else changed. A frame's bytes move into the
/// `Pixels`; nothing is copied.
///
/// The first frame after [`FramePacer::new`] or [`FramePacer::flush`] shows as soon as it's
/// queued, whatever the clock says: it is the poster for a clip that hasn't started and the
/// picture for a seek that hasn't landed. After that a frame shows once the clock reaches its
/// time; when several are due, the latest shows and the rest drop.
pub struct FramePacer {
    pixels: Pixels,
    queue: VecDeque<VideoFrame>,
    capacity: usize,
    /// The next frame shows at once (nothing shown since the last flush).
    fresh: bool,
    /// The time the frame on screen shows until, when the next frame's time is known.
    shown_until: Option<Duration>,
    /// No more frames until the next flush.
    finished: bool,
    shown: Option<Duration>,
    stats: PacerStats,
}

impl FramePacer {
    /// A pacer writing into `pixels`, holding at most `capacity` frames (at least 1).
    pub fn new(pixels: Pixels, capacity: usize) -> FramePacer {
        FramePacer {
            pixels,
            queue: VecDeque::new(),
            capacity: capacity.max(1),
            fresh: true,
            shown_until: None,
            finished: false,
            shown: None,
            stats: PacerStats::default(),
        }
    }

    pub fn pixels(&self) -> &Pixels {
        &self.pixels
    }

    /// Queues a frame, or gives it back with the reason.
    pub fn push(&mut self, frame: VideoFrame) -> Result<(), (VideoFrame, Refused)> {
        let bytes = (frame.width as usize)
            .checked_mul(frame.height as usize)
            .and_then(|n| n.checked_mul(4));
        if bytes != Some(frame.rgba.len()) {
            return Err((frame, Refused::BadSize));
        }
        if self.queue.len() >= self.capacity {
            return Err((frame, Refused::Full));
        }
        let last = self.queue.back().map(|f| f.pts).or(self.shown);
        if last.is_some_and(|last| frame.pts < last) {
            return Err((frame, Refused::OutOfOrder));
        }
        self.queue.push_back(frame);
        Ok(())
    }

    /// Whether [`FramePacer::push`] has room.
    pub fn has_room(&self) -> bool {
        self.queue.len() < self.capacity
    }

    /// The frames pushed so far are the clip's last, until the next flush: once the last one
    /// shows, the picture holds ([`Presented::Held`]) however long the sound plays on, rather
    /// than counting as repeated. Frames pushed after it still show.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    /// Drops the queued frames, for a seek: the next frame pushed shows at once. The picture on
    /// screen stays until then.
    pub fn flush(&mut self) {
        self.queue.clear();
        self.fresh = true;
        self.finished = false;
        self.shown = None;
        self.shown_until = None;
    }

    /// The time of the next queued frame.
    pub fn next_due(&self) -> Option<Duration> {
        self.queue.front().map(|f| f.pts)
    }

    /// The time of the frame on screen, since the last flush.
    pub fn shown(&self) -> Option<Duration> {
        self.shown
    }

    pub fn stats(&self) -> PacerStats {
        self.stats
    }

    /// Shows the frame `clock` is at, if it isn't on screen yet.
    pub fn present(&mut self, clock: &(impl MediaClock + ?Sized)) -> Presented {
        if self.fresh {
            return match self.queue.pop_front() {
                Some(frame) => {
                    self.fresh = false;
                    self.show(frame, 0)
                }
                None => Presented::Empty,
            };
        }
        let reading = clock.reading();
        let position = reading.position;
        let mut due = None;
        let mut dropped = 0;
        while self.queue.front().is_some_and(|f| f.pts <= position) {
            if due
                .replace(self.queue.pop_front().expect("front"))
                .is_some()
            {
                dropped += 1;
            }
        }
        if let Some(frame) = due {
            return self.show(frame, dropped);
        }
        if let Some(until) = self.shown_until
            && !self.finished
            && reading.state == ClockState::Playing
            && position >= until
            && self.queue.is_empty()
        {
            self.stats.repeated += 1;
            return Presented::Repeated;
        }
        if self.shown.is_some() {
            Presented::Held
        } else {
            Presented::Empty
        }
    }

    fn show(&mut self, frame: VideoFrame, dropped: u32) -> Presented {
        let pts = frame.pts;
        // How long a frame lasts, from the step between the last two; without a step there is
        // nothing to repeat against.
        self.shown_until = self
            .queue
            .front()
            .map(|next| next.pts)
            .or_else(|| self.shown.map(|prev| pts + pts.saturating_sub(prev)));
        self.shown = Some(pts);
        self.stats.shown += 1;
        self.stats.dropped += u64::from(dropped);
        self.pixels.set(frame.width, frame.height, frame.rgba);
        Presented::Shown { pts, dropped }
    }
}
