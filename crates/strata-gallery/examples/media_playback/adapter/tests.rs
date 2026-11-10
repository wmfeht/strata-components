use std::cell::RefCell;
use std::time::Duration;

use strata_components::media_player::{MediaIntent, PlaybackClock, PlaybackState};
use strata_platform::media::{ClockState, MediaClock, Reading};

use super::{ClockAdapter, Transport, apply};

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
