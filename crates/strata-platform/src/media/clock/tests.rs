use std::time::{Duration, Instant};

use super::{ClockState, WallClock};

#[test]
fn the_wall_clock_loads_seeks_and_ends_as_the_sink_does() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut clock = WallClock::new(t0);
    clock.set_end(Some(Duration::from_secs(2)));
    // A play asked for while loading starts when the first frame is ready, not before.
    clock.play(at(0));
    assert_eq!(clock.reading_at(at(100)).state, ClockState::Loading);
    assert_eq!(clock.reading_at(at(100)).position, Duration::ZERO);
    clock.ready(at(100));
    let r = clock.reading_at(at(600));
    assert_eq!(
        (r.state, r.position),
        (ClockState::Playing, Duration::from_millis(500))
    );

    clock.pause(at(600));
    assert_eq!(
        clock.reading_at(at(900)).position,
        Duration::from_millis(500)
    );

    // A seek holds its target, a new generation, until ready; playing resumes if it was on.
    clock.play(at(900));
    clock.seek(Duration::from_millis(1_500), at(1_000));
    let r = clock.reading_at(at(1_200));
    assert_eq!(
        (r.state, r.position, r.generation),
        (ClockState::Seeking, Duration::from_millis(1_500), 1)
    );
    clock.ready(at(1_200));
    let r = clock.reading_at(at(2_000));
    assert_eq!(
        (r.state, r.position),
        (ClockState::Ended, Duration::from_secs(2))
    );
    // Ended stays ended: playing again is a seek's job.
    clock.play(at(2_100));
    assert_eq!(clock.reading_at(at(2_500)).state, ClockState::Ended);
}
