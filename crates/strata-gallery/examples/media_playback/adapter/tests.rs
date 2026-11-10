use std::cell::RefCell;
use std::time::Duration;

use strata_components::media_player::{MediaIntent, PlaybackClock, PlaybackState};
use strata_platform::media::{ClockState, MediaClock, Reading, SinkEvent};

use super::{ClockAdapter, Recovered, Recovery, Transport, apply};

/// Where a test's reopen says the clock stopped.
const STOPPED: Duration = Duration::from_millis(1_500);

/// The sink calls an intent made, in order.
#[derive(Default)]
struct Calls(RefCell<Vec<String>>);

impl Transport for Calls {
    fn play(&self) {
        self.0.borrow_mut().push("play".into());
    }
    fn pause(&self) {
        self.0.borrow_mut().push("pause".into());
    }
    fn seek(&self, at: Duration) {
        self.0.borrow_mut().push(format!("seek {}", at.as_millis()));
    }
    fn set_volume(&self, volume: f64, muted: bool) {
        self.0.borrow_mut().push(format!("volume {volume} {muted}"));
    }
    fn reopen(&self) -> Duration {
        self.0.borrow_mut().push("reopen".into());
        STOPPED
    }
}

struct Fixed(Reading);

impl MediaClock for Fixed {
    fn reading(&self) -> Reading {
        self.0
    }
}

fn adapter(state: ClockState, ms: u64) -> ClockAdapter {
    let mut a = ClockAdapter::new(Some(Duration::from_secs(10)));
    a.refresh(&Fixed(Reading {
        position: Duration::from_millis(ms),
        state,
        generation: 0,
    }));
    a
}

#[test]
fn an_underrun_reads_as_loading_and_a_failure_as_paused() {
    // Underrun shows as Loading (OWNER ruling): the sink wakes the app when it resumes.
    assert_eq!(
        adapter(ClockState::Underrun, 500).state(),
        PlaybackState::Loading
    );
    assert_eq!(
        adapter(ClockState::Failed, 500).state(),
        PlaybackState::Paused
    );
    assert_eq!(
        adapter(ClockState::Seeking, 500).state(),
        PlaybackState::Seeking
    );
    // The decoder's length caps the position: a sink that read a little past it reads the end.
    let a = adapter(ClockState::Ended, 10_020);
    assert_eq!(
        (a.position(), a.state()),
        (Duration::from_secs(10), PlaybackState::Ended)
    );
    assert!(a.seekable());
}

#[test]
fn intents_reach_the_sink_and_seeks_restart_the_decoder() {
    let calls = Calls::default();
    let playing = adapter(ClockState::Playing, 2_000);
    assert_eq!(
        apply(&calls, &playing, &MediaIntent::Seek(Duration::from_secs(7))),
        Some(Duration::from_secs(7))
    );
    assert_eq!(apply(&calls, &playing, &MediaIntent::Pause), None);
    assert_eq!(
        apply(
            &calls,
            &playing,
            &MediaIntent::SetVolume {
                volume: 0.5,
                muted: true
            }
        ),
        None
    );
    // The host's own intents reach no sink.
    assert_eq!(apply(&calls, &playing, &MediaIntent::Step(1)), None);
    assert_eq!(
        apply(&calls, &playing, &MediaIntent::HandOff { at: None }),
        None
    );
    // A play from the end starts again from the beginning.
    let ended = adapter(ClockState::Ended, 10_000);
    assert_eq!(
        apply(&calls, &ended, &MediaIntent::Play),
        Some(Duration::ZERO)
    );
    assert_eq!(apply(&calls, &playing, &MediaIntent::Play), None);
    assert_eq!(
        calls.0.into_inner(),
        [
            "seek 7000",
            "pause",
            "volume 0.5 true",
            "seek 0",
            "play",
            "play"
        ]
    );
}

fn reading(state: ClockState, position: Duration) -> Reading {
    Reading {
        position,
        state,
        generation: 1,
    }
}

#[test]
fn a_lost_device_an_error_and_a_stall_reopen_three_times_in_a_row_then_show_the_failure() {
    let calls = Calls::default();
    let mut recovery = Recovery::default();
    let reopened = Some(Recovered::Reopened { at: STOPPED });
    assert_eq!(
        recovery.handle(&calls, &SinkEvent::Underrun { at: STOPPED }),
        None
    );
    let lost = SinkEvent::DeviceLost {
        message: "gone".into(),
    };
    assert_eq!(recovery.handle(&calls, &lost), reopened);
    // A stall: the device stopped taking PCM, or a fresh output never started.
    assert_eq!(
        recovery.handle(&calls, &SinkEvent::Stalled { at: STOPPED }),
        reopened
    );
    let error = SinkEvent::Error {
        message: "broken".into(),
    };
    assert_eq!(recovery.handle(&calls, &error), reopened);
    assert_eq!(
        recovery.handle(&calls, &error),
        Some(Recovered::GaveUp {
            detail: "broken".into()
        })
    );
    assert_eq!(calls.0.into_inner(), ["reopen", "reopen", "reopen"]);
}

#[test]
fn playing_on_past_a_reopen_clears_the_count() {
    let calls = Calls::default();
    let mut recovery = Recovery::default();
    let stalled = SinkEvent::Stalled { at: STOPPED };
    for _ in 0..3 {
        assert!(matches!(
            recovery.handle(&calls, &stalled),
            Some(Recovered::Reopened { .. })
        ));
        // Ready again where it stopped, or moved on while not playing: not yet recovered.
        recovery.observe(reading(ClockState::Playing, STOPPED));
        recovery.observe(reading(ClockState::Seeking, STOPPED * 2));
    }
    recovery.observe(reading(
        ClockState::Playing,
        STOPPED + Duration::from_millis(20),
    ));
    assert!(matches!(
        recovery.handle(&calls, &stalled),
        Some(Recovered::Reopened { .. })
    ));
}
