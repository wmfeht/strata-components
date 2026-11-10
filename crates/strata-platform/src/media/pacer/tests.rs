use std::cell::Cell;
use std::time::Duration;

use rune::Pixels;

use super::{FramePacer, Presented, Refused, VideoFrame};
use crate::media::clock::{ClockState, MediaClock, Reading};

/// A clock the test moves by hand.
struct FakeClock(Cell<Duration>, Cell<ClockState>);

impl FakeClock {
    fn at(&self, ms: u64) -> &FakeClock {
        self.0.set(Duration::from_millis(ms));
        self
    }
}

impl MediaClock for FakeClock {
    fn reading(&self) -> Reading {
        Reading {
            position: self.0.get(),
            state: self.1.get(),
            generation: 0,
        }
    }
}

fn clock() -> FakeClock {
    FakeClock(Cell::new(Duration::ZERO), Cell::new(ClockState::Playing))
}

/// A frame whose width tells it apart on screen: `Pixels::size` names the frame showing.
fn frame(ms: u64, width: u32) -> VideoFrame {
    VideoFrame {
        pts: Duration::from_millis(ms),
        width,
        height: 1,
        rgba: vec![0; width as usize * 4],
    }
}

fn pacer(frames: &[(u64, u32)]) -> FramePacer {
    let mut pacer = FramePacer::new(Pixels::new(), 8);
    for &(ms, width) in frames {
        pacer.push(frame(ms, width)).expect("queued");
    }
    pacer
}

fn on_screen(pacer: &FramePacer) -> u32 {
    pacer.pixels().size().0
}

#[test]
fn the_latest_due_frame_shows_and_the_frames_before_it_drop() {
    let mut pacer = pacer(&[(0, 1), (33, 2), (66, 3), (100, 4)]);
    let clock = clock();
    assert_eq!(
        pacer.present(clock.at(0)),
        Presented::Shown {
            pts: Duration::ZERO,
            dropped: 0
        }
    );
    // The clock jumped past two frames' times: the later shows, the earlier never does.
    assert_eq!(
        pacer.present(clock.at(70)),
        Presented::Shown {
            pts: Duration::from_millis(66),
            dropped: 1
        }
    );
    assert_eq!(on_screen(&pacer), 3);
    assert_eq!(pacer.stats().dropped, 1);
    assert_eq!(pacer.next_due(), Some(Duration::from_millis(100)));
}

#[test]
fn a_frame_holds_until_the_next_is_due_and_repeats_when_the_next_is_late() {
    let mut pacer = pacer(&[(0, 1), (33, 2)]);
    let clock = clock();
    pacer.present(clock.at(0));
    assert_eq!(pacer.present(clock.at(20)), Presented::Held);
    assert_eq!(on_screen(&pacer), 1, "frame 0 stays until 33 ms");
    assert!(matches!(
        pacer.present(clock.at(40)),
        Presented::Shown { .. }
    ));
    assert_eq!(on_screen(&pacer), 2);
    // Frame 33 lasts until 66 (the step between the last two); nothing came after it.
    assert_eq!(pacer.present(clock.at(60)), Presented::Held);
    assert_eq!(pacer.present(clock.at(70)), Presented::Repeated);
    assert_eq!(pacer.present(clock.at(90)), Presented::Repeated);
    assert_eq!(pacer.stats().repeated, 2);
    // The late frame arrives: it shows, and the repeats stop.
    pacer.push(frame(66, 3)).expect("queued");
    assert!(matches!(
        pacer.present(clock.at(91)),
        Presented::Shown { .. }
    ));
    assert_eq!(on_screen(&pacer), 3);
    assert_eq!(
        pacer.stats(),
        super::PacerStats {
            shown: 3,
            dropped: 0,
            repeated: 2
        }
    );
}

#[test]
fn after_a_flush_the_first_frame_shows_at_once_and_stale_frames_never_do() {
    let mut pacer = pacer(&[(0, 1), (1_000, 2), (1_033, 3)]);
    let clock = clock();
    pacer.present(clock.at(0));
    assert!(matches!(
        pacer.present(clock.at(1_000)),
        Presented::Shown { .. }
    ));
    // A seek to 5 s: the frames queued before it go, and the clock hasn't reported the seek.
    pacer.flush();
    assert_eq!(pacer.present(clock.at(1_010)), Presented::Empty);
    assert_eq!(
        on_screen(&pacer),
        2,
        "the old picture stays until a new frame"
    );
    pacer.push(frame(5_000, 4)).expect("queued");
    pacer.push(frame(5_033, 5)).expect("queued");
    assert_eq!(
        pacer.present(clock.at(1_020)),
        Presented::Shown {
            pts: Duration::from_secs(5),
            dropped: 0
        }
    );
    assert_eq!(on_screen(&pacer), 4);
    // After that first frame, the clock paces again.
    assert_eq!(pacer.present(clock.at(5_000)), Presented::Held);
    assert!(matches!(
        pacer.present(clock.at(5_040)),
        Presented::Shown { .. }
    ));
    assert_eq!(on_screen(&pacer), 5);
}

#[test]
fn frames_come_in_order_whole_and_within_the_queue() {
    let mut pacer = FramePacer::new(Pixels::new(), 2);
    pacer.push(frame(0, 1)).expect("queued");
    pacer.push(frame(33, 1)).expect("queued");
    assert!(!pacer.has_room());
    assert_eq!(pacer.push(frame(66, 1)).unwrap_err().1, Refused::Full);
    pacer.present(clock().at(0));
    assert_eq!(pacer.push(frame(10, 1)).unwrap_err().1, Refused::OutOfOrder);
    let mut short = frame(66, 2);
    short.rgba.pop();
    assert_eq!(pacer.push(short).unwrap_err().1, Refused::BadSize);
    assert!(pacer.push(frame(66, 1)).is_ok());
}

#[test]
fn after_the_last_frame_the_picture_holds_while_the_sound_plays_on() {
    let mut pacer = pacer(&[(0, 1), (33, 2)]);
    pacer.finish();
    let clock = clock();
    pacer.present(clock.at(0));
    assert!(matches!(
        pacer.present(clock.at(40)),
        Presented::Shown { .. }
    ));
    // The clip's video ended at 66 ms; its sound goes on.
    assert_eq!(pacer.present(clock.at(70)), Presented::Held);
    assert_eq!(pacer.present(clock.at(500)), Presented::Held);
    assert_eq!(pacer.stats().repeated, 0);
    // A flush starts a new run of frames: a late one repeats again.
    pacer.flush();
    pacer.push(frame(1_000, 3)).expect("queued");
    pacer.push(frame(1_033, 4)).expect("queued");
    pacer.present(clock.at(1_000));
    pacer.present(clock.at(1_040));
    assert_eq!(pacer.present(clock.at(1_070)), Presented::Repeated);
}
