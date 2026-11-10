//! The sink on `Output::Null` (fakesink, sync on), driven by a manual GStreamer clock: no sound
//! server is opened, and time moves only when a test moves it. Each generation's PCM carries
//! its own sample value, so the fakesink's handoffs say which PCM was rendered and when.

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;

use super::{AudioSink, Output, PushError, SinkEvent, SinkOptions};
use crate::media::clock::{ClockState, MediaClock};
use crate::media::format::PcmFormat;
use crate::media::pacer::{FramePacer, VideoFrame};

const FORMAT: PcmFormat = PcmFormat::STEREO_S16_48K;
/// One chunk as the decoder sends it: 1/30 s.
const CHUNK_MS: u64 = 1_000 / 30;
/// How far a position read may trail the clock: GStreamer reads the position at the clock's
/// time, but the manual clock moves in steps.
const SLACK: Duration = Duration::from_millis(12);

mod manual_clock {
    use gst::glib;
    use gst::subclass::prelude::*;
    use gstreamer as gst;
    use std::sync::{Condvar, Mutex};

    /// A pipeline clock that moves only when the test sets it.
    #[derive(Default)]
    pub struct Imp {
        time: Mutex<(u64, u64)>,
        changed: Condvar,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Imp {
        const NAME: &'static str = "StrataPlatformMediaManualClock";
        type Type = ManualClock;
        type ParentType = gst::Clock;
    }

    impl ObjectImpl for Imp {}
    impl GstObjectImpl for Imp {}

    impl ClockImpl for Imp {
        fn internal_time(&self) -> gst::ClockTime {
            gst::ClockTime::from_nseconds(self.time.lock().unwrap().0)
        }

        fn wait(
            &self,
            id: &gst::ClockId,
        ) -> (
            Result<gst::ClockSuccess, gst::ClockError>,
            gst::ClockTimeDiff,
        ) {
            let target = id.time().nseconds();
            let mut time = self.time.lock().unwrap();
            let generation = time.1;
            while time.0 < target && time.1 == generation {
                time = self.changed.wait(time).unwrap();
            }
            if time.1 != generation {
                return (Err(gst::ClockError::Unscheduled), 0);
            }
            (Ok(gst::ClockSuccess::Ok), time.0 as i64 - target as i64)
        }

        // The sink is the only waiter, so any unschedule releases it.
        fn unschedule(&self, _id: &gst::ClockId) {
            self.time.lock().unwrap().1 += 1;
            self.changed.notify_all();
        }
    }

    glib::wrapper! {
        pub struct ManualClock(ObjectSubclass<Imp>) @extends gst::Clock, gst::Object;
    }

    impl ManualClock {
        pub fn new() -> ManualClock {
            glib::Object::new()
        }

        pub fn ms(&self) -> u64 {
            self.imp().time.lock().unwrap().0 / 1_000_000
        }

        pub fn set_ms(&self, ms: u64) {
            self.imp().time.lock().unwrap().0 = ms * 1_000_000;
            self.imp().changed.notify_all();
        }
    }
}
use manual_clock::ManualClock;

struct Rig {
    sink: AudioSink,
    clock: ManualClock,
    wakes: Arc<AtomicUsize>,
    /// Each buffer rendered: its timestamp and its PCM's sample value.
    rendered: Arc<Mutex<Vec<(Duration, i16)>>>,
}

fn rig(options: SinkOptions) -> Rig {
    gst::init().expect("GStreamer");
    let clock = ManualClock::new();
    let wakes = Arc::new(AtomicUsize::new(0));
    let w = Arc::clone(&wakes);
    let options = options.output(Output::Null).clock(clock.clone().upcast());
    let sink = AudioSink::open(
        FORMAT,
        options,
        Arc::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        }),
    )
    .expect("a null sink opens");
    let rendered = Arc::new(Mutex::new(Vec::new()));
    let r = Arc::clone(&rendered);
    let element = sink.sink_element();
    element.set_property("signal-handoffs", true);
    element.connect("handoff", false, move |args| {
        let buffer = args[1].get::<gst::Buffer>().expect("a buffer");
        let pts = Duration::from_nanos(buffer.pts().map_or(0, |t| t.nseconds()));
        let map = buffer.map_readable().expect("readable");
        let value = i16::from_le_bytes([map[0], map[1]]);
        r.lock().unwrap().push((pts, value));
        None
    });
    Rig {
        sink,
        clock,
        wakes,
        rendered,
    }
}

/// `ms` of PCM whose every sample is `value`, in decoder-sized chunks.
fn push(sink: &AudioSink, ms: u64, value: i16) {
    let frames = FORMAT.frames_in(Duration::from_millis(ms));
    let chunk = FORMAT.frames_in(Duration::from_millis(CHUNK_MS)).max(1);
    let mut left = frames;
    while left > 0 {
        let n = left.min(chunk);
        let mut pcm = Vec::with_capacity(n as usize * FORMAT.frame_bytes());
        for _ in 0..n * u64::from(FORMAT.channels) {
            pcm.extend_from_slice(&value.to_le_bytes());
        }
        sink.push(pcm).expect("pushed");
        left -= n;
    }
}

fn eventually(what: &str, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

impl Rig {
    /// Moves the pipeline clock forward to `to` ms in small steps, letting the sink render.
    fn run_to(&self, to: u64) {
        let mut t = self.clock.ms();
        while t < to {
            t = (t + 5).min(to);
            self.clock.set_ms(t);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn next_event(&self) -> SinkEvent {
        let mut got = None;
        eventually("an event", || {
            got = self.sink.drain_events().into_iter().next();
            got.is_some()
        });
        got.expect("an event")
    }

    fn position_near(&self, want: Duration) {
        eventually(&format!("the clock at {want:?}"), || {
            let p = self.sink.reading().position;
            p <= want && want - p <= SLACK
        });
    }

    fn wakes(&self) -> usize {
        self.wakes.load(Ordering::SeqCst)
    }

    /// The sample values rendered since the last call.
    fn take_rendered(&self) -> Vec<(Duration, i16)> {
        std::mem::take(&mut *self.rendered.lock().unwrap())
    }
}

#[test]
fn loading_ends_with_one_wake_when_the_first_pcm_is_ready() {
    let rig = rig(SinkOptions::default());
    let r = rig.sink.reading();
    assert_eq!((r.state, r.position), (ClockState::Loading, Duration::ZERO));
    push(&rig.sink, 300, 1);
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    assert_eq!(rig.wakes(), 1);
    assert_eq!(rig.sink.reading().state, ClockState::Paused);
}

#[test]
fn the_clock_follows_the_pcm_played_and_holds_while_paused() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    assert_eq!(rig.sink.reading().state, ClockState::Playing);
    rig.run_to(400);
    rig.position_near(ms(400));
    // The audio rendered is the clock's: the chunk the position is in has rendered, and none
    // after it. (The position, not the test's clock: playing starts a step after Ready.)
    let p = rig.sink.reading().position;
    eventually("the chunk at the position rendered", || {
        rig.rendered
            .lock()
            .unwrap()
            .last()
            .is_some_and(|&(pts, _)| p - pts < ms(CHUNK_MS + 1))
    });
    let last = rig.take_rendered().last().copied().expect("rendered").0;
    assert!(
        last <= p,
        "audio from {last:?} rendered with the clock at {p:?}"
    );

    rig.sink.pause();
    eventually("paused", || rig.sink.reading().state == ClockState::Paused);
    rig.run_to(700);
    std::thread::sleep(Duration::from_millis(30));
    let held = rig.sink.reading().position;
    assert!(
        held <= ms(400) && ms(400) - held <= SLACK,
        "held at {held:?}"
    );

    rig.sink.play();
    rig.run_to(900);
    rig.position_near(ms(600));
    assert_eq!(
        rig.wakes(),
        1,
        "play and pause are the app's: only Ready woke it"
    );
}

#[test]
fn a_seek_holds_its_target_until_its_pcm_is_ready_and_the_old_pcm_never_plays() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(300);
    rig.position_near(ms(300));

    rig.sink.seek(ms(5_000));
    let r = rig.sink.reading();
    assert_eq!(
        (r.state, r.position, r.generation),
        (ClockState::Seeking, ms(5_000), 1)
    );
    // The decoder takes a while to restart: the clock holds the target meanwhile, and nothing
    // the pipeline did before its PCM lands the seek.
    rig.run_to(400);
    std::thread::sleep(Duration::from_millis(50));
    let r = rig.sink.reading();
    assert_eq!((r.state, r.position), (ClockState::Seeking, ms(5_000)));
    assert!(
        rig.sink.drain_events().is_empty(),
        "the seek landed before its PCM"
    );
    rig.take_rendered();
    push(&rig.sink, 1_000, 2);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: ms(5_000) });
    assert_eq!(rig.wakes(), 2);
    eventually("playing again", || {
        rig.sink.reading().state == ClockState::Playing
    });
    rig.run_to(600);
    rig.position_near(ms(5_200));
    let rendered = rig.take_rendered();
    assert!(!rendered.is_empty());
    assert!(
        rendered.iter().all(|&(_, value)| value == 2),
        "PCM from before the seek played after it"
    );
    assert_eq!(
        rendered[0].0,
        Duration::ZERO,
        "the seek's PCM plays from its start"
    );
}

#[test]
fn when_the_pcm_runs_out_the_clock_holds_there_and_resumes_when_more_arrives() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 300, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(600);
    let SinkEvent::Underrun { at } = rig.next_event() else {
        panic!("not an underrun");
    };
    assert!(at <= ms(300) && ms(300) - at <= SLACK, "ran dry at {at:?}");
    assert_eq!(rig.sink.reading().state, ClockState::Underrun);
    assert_eq!(rig.wakes(), 2);

    // More PCM comes later; the clock went on to 600 meanwhile, but the PCM must not play late.
    rig.run_to(800);
    push(&rig.sink, 500, 1);
    let SinkEvent::Resumed { .. } = rig.next_event() else {
        panic!("not resumed");
    };
    assert_eq!(rig.wakes(), 3);
    rig.run_to(900);
    rig.position_near(ms(400));
}

#[test]
fn finishing_ends_the_clock_once_the_last_pcm_plays() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 200, 1);
    rig.sink.finish();
    assert_eq!(rig.sink.push(vec![0; 4]), Err(PushError::Finished));
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(400);
    let end = FORMAT.duration_of(FORMAT.frames_in(ms(200)));
    assert_eq!(rig.next_event(), SinkEvent::Ended { at: end });
    let r = rig.sink.reading();
    assert_eq!((r.state, r.position), (ClockState::Ended, end));
}

#[test]
fn a_lost_device_fails_the_clock_and_reopening_plays_on_from_where_it_stopped() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(300);
    rig.position_near(ms(300));
    let element = rig.sink.sink_element();
    let _ = element.post_message(
        gst::message::Error::builder(gst::ResourceError::Write, "the device went away")
            .src(&element)
            .build(),
    );
    let SinkEvent::DeviceLost { message } = rig.next_event() else {
        panic!("not a lost device");
    };
    assert!(message.contains("the device went away"), "{message}");
    let r = rig.sink.reading();
    assert_eq!(r.state, ClockState::Failed);
    assert_eq!(rig.sink.push(vec![0; 4]), Err(PushError::Failed));

    rig.sink.reopen();
    let r = rig.sink.reading();
    assert_eq!(r.state, ClockState::Seeking);
    assert!(
        ms(300) - r.position <= SLACK,
        "reopens where it stopped: {:?}",
        r.position
    );
    push(&rig.sink, 500, 2);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: r.position });
    eventually("playing again", || {
        rig.sink.reading().state == ClockState::Playing
    });
    rig.run_to(500);
    rig.position_near(r.position + ms(200));
}

#[test]
fn a_device_that_stops_taking_pcm_is_a_stall_reported_once() {
    let rig = rig(SinkOptions::default().stall_after(ms(150)));
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(100);
    rig.position_near(ms(100));
    // The clock stops while PCM is queued: the device took no more.
    let SinkEvent::Stalled { at } = rig.next_event() else {
        panic!("not a stall");
    };
    assert!(ms(100) - at <= SLACK);
    std::thread::sleep(Duration::from_millis(300));
    assert!(rig.sink.drain_events().is_empty(), "one stall, one event");
    assert_eq!(rig.sink.reading().state, ClockState::Playing);
}

#[test]
fn a_device_that_has_not_started_yet_is_not_a_stall() {
    let rig = rig(SinkOptions::default().stall_after(ms(100)));
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    // The clock never moves: the device is still opening, not stuck.
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        rig.sink.drain_events().is_empty(),
        "a slow start reported as a stall"
    );
}

/// Video on the sink's clock against the audio the sink renders, through play, pause, seek and
/// a paused seek. The offset is how far the clock is past the frame on screen: within a frame
/// interval, and never negative (no frame shows early).
#[test]
fn video_paced_on_the_sink_clock_stays_within_a_frame_of_the_audio() {
    const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 30);
    let rig = rig(SinkOptions::default());
    let mut pacer = FramePacer::new(rune::Pixels::new(), 3);
    let mut offsets = Vec::new();
    // The decoder: a clip of 2 s from `start`, PCM and frames pushed as there's room.
    struct Decoder {
        start: Duration,
        frame: u32,
        pcm_ms: u64,
        value: i16,
    }
    let feed = |dec: &mut Decoder, pacer: &mut FramePacer| {
        while dec.pcm_ms < 2_000 && rig.sink.room() > ms(CHUNK_MS) {
            push(&rig.sink, CHUNK_MS, dec.value);
            dec.pcm_ms += CHUNK_MS;
        }
        while pacer.has_room() && dec.frame < 60 {
            let pts = dec.start + FRAME * dec.frame;
            pacer
                .push(VideoFrame {
                    pts,
                    width: 2,
                    height: 2,
                    rgba: vec![0; 16],
                })
                .expect("room");
            dec.frame += 1;
        }
    };
    let step = |dec: &mut Decoder, pacer: &mut FramePacer, to: u64, offsets: &mut Vec<Duration>| {
        let mut t = rig.clock.ms();
        while t < to {
            t += 5;
            rig.clock.set_ms(t);
            std::thread::sleep(Duration::from_millis(1));
            feed(dec, pacer);
            pacer.present(&rig.sink.clock());
            let r = rig.sink.reading();
            if r.state == ClockState::Playing
                && let Some(shown) = pacer.shown()
            {
                assert!(
                    r.position >= shown,
                    "frame {shown:?} showed early at {:?}",
                    r.position
                );
                let offset = r.position - shown;
                assert!(
                    offset < FRAME + SLACK,
                    "video {offset:?} behind at {:?}",
                    r.position
                );
                offsets.push(offset);
            }
        }
    };

    let mut dec = Decoder {
        start: Duration::ZERO,
        frame: 0,
        pcm_ms: 0,
        value: 1,
    };
    feed(&mut dec, &mut pacer);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    step(&mut dec, &mut pacer, 500, &mut offsets);
    rig.sink.pause();
    step(&mut dec, &mut pacer, 700, &mut offsets);
    rig.sink.play();
    step(&mut dec, &mut pacer, 900, &mut offsets);

    // A seek while playing: the decoder restarts at 10 s.
    rig.sink.seek(ms(10_000));
    pacer.flush();
    dec = Decoder {
        start: ms(10_000),
        frame: 0,
        pcm_ms: 0,
        value: 2,
    };
    step(&mut dec, &mut pacer, 1_400, &mut offsets);
    let rendered = rig.take_rendered();
    let after: Vec<_> = rendered.iter().skip_while(|&&(_, v)| v == 1).collect();
    assert!(
        after.iter().all(|&&(_, v)| v == 2),
        "old PCM after the seek"
    );

    // A seek while paused, then play.
    rig.sink.pause();
    rig.sink.seek(ms(20_000));
    pacer.flush();
    dec = Decoder {
        start: ms(20_000),
        frame: 0,
        pcm_ms: 0,
        value: 3,
    };
    step(&mut dec, &mut pacer, 1_600, &mut offsets);
    assert_eq!(rig.sink.reading().state, ClockState::Paused);
    assert_eq!(
        pacer.shown(),
        Some(ms(20_000)),
        "the seek's first frame shows while paused"
    );
    rig.sink.play();
    step(&mut dec, &mut pacer, 2_100, &mut offsets);
    rig.position_near(ms(20_500));

    offsets.sort();
    let pick = |q: f64| offsets[((offsets.len() - 1) as f64 * q) as usize];
    let stats = pacer.stats();
    let report = format!(
        "p7 sync (headless, manual clock): offset p50 {:?} p95 {:?} max {:?} over {} presents; \
         shown {} dropped {} repeated {}\n",
        pick(0.5),
        pick(0.95),
        pick(1.0),
        offsets.len(),
        stats.shown,
        stats.dropped,
        stats.repeated,
    );
    let _ = std::io::stderr().lock().write_all(report.as_bytes());
    assert_eq!(
        stats.repeated, 0,
        "the decoder kept up, so no frame repeated"
    );
}

/// The hand test: plays two seconds of a 440 Hz tone on the default output. Run it by hand
/// with `STRATA_PLATFORM_TONE=1 cargo test -p strata-platform --features media-audio
/// plays_a_tone -- --ignored`; without the variable it plays nothing and says SKIP.
#[test]
#[ignore = "plays a tone on the default audio output; run it by hand"]
fn plays_a_tone_on_the_default_output() {
    if std::env::var_os("STRATA_PLATFORM_TONE").is_none_or(|v| v != "1") {
        let _ = std::io::stderr().lock().write_all(
            b"SKIP p7::plays_a_tone_on_the_default_output: set STRATA_PLATFORM_TONE=1 to play it\n",
        );
        return;
    }
    let sink = AudioSink::open(FORMAT, SinkOptions::default(), Arc::new(|| {}))
        .expect("the default output opens");
    let frames = FORMAT.frames_in(Duration::from_secs(2));
    let mut pcm = Vec::with_capacity(frames as usize * FORMAT.frame_bytes());
    for i in 0..frames {
        let s = (i as f64 * 440.0 * std::f64::consts::TAU / f64::from(FORMAT.rate)).sin();
        let v = ((s * 0.2) * f64::from(i16::MAX)) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    sink.push(pcm).expect("pushed");
    sink.finish();
    sink.play();
    let deadline = Instant::now() + Duration::from_secs(10);
    while sink.reading().state != ClockState::Ended {
        assert!(
            Instant::now() < deadline,
            "the tone never ended: {:?}",
            sink.reading()
        );
        assert!(
            !matches!(sink.reading().state, ClockState::Failed),
            "the output failed: {:?}",
            sink.drain_events()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
