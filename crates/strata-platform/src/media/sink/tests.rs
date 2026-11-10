//! The sink on `Output::Null` (fakesink, sync on), driven by a manual GStreamer clock: no sound
//! server is opened, and time moves only when a test moves it. Each generation's PCM carries
//! its own sample value, so the fakesink's handoffs say which PCM was rendered and when.

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;

use super::control::Cmd;
use super::pipeline::Parts;
use super::{AudioSink, Output, PushError, SinkEvent, SinkOptions, lock};
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

mod refusing {
    use gst::glib;
    use gst::subclass::prelude::*;
    use gstreamer as gst;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// An element that refuses to leave PLAYING until allowed: in a playing bin, it makes the
    /// bin's pause fail, as a dead device's sink can. (A bin skips children already below the
    /// state it goes to, so the element has to be playing.)
    #[derive(Default)]
    pub struct Imp {
        allowed: AtomicBool,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Imp {
        const NAME: &'static str = "StrataPlatformMediaRefusing";
        type Type = Refusing;
        type ParentType = gst::Element;
    }

    impl ObjectImpl for Imp {}
    impl GstObjectImpl for Imp {}

    impl ElementImpl for Imp {
        fn change_state(
            &self,
            transition: gst::StateChange,
        ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
            if transition == gst::StateChange::PlayingToPaused
                && !self.allowed.load(Ordering::SeqCst)
            {
                return Err(gst::StateChangeError);
            }
            self.parent_change_state(transition)
        }
    }

    glib::wrapper! {
        pub struct Refusing(ObjectSubclass<Imp>) @extends gst::Element, gst::Object;
    }

    impl Refusing {
        pub fn new() -> Refusing {
            glib::Object::new()
        }

        pub fn allow(&self) {
            self.imp().allowed.store(true, Ordering::SeqCst);
        }
    }
}
use refusing::Refusing;

/// Holds the one thread that passes it once armed, until the test releases it: the control
/// thread inside a wake, or a reader inside a clock read.
#[derive(Default)]
struct Gate {
    /// Armed, and whether a thread is held.
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}

impl Gate {
    fn arm(&self) {
        self.state.lock().unwrap().0 = true;
    }

    fn pass(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.0 {
            return;
        }
        *state = (false, true);
        self.changed.notify_all();
        while state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn wait_held(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut state = self.state.lock().unwrap();
        while !state.1 {
            assert!(Instant::now() < deadline, "nothing reached the gate");
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(10))
                .unwrap()
                .0;
        }
    }

    fn release(&self) {
        self.state.lock().unwrap().1 = false;
        self.changed.notify_all();
    }
}

/// The thread name the read gate holds; the control thread reads the clock too.
const READER: &str = "torn-read";

struct Rig {
    sink: AudioSink,
    clock: ManualClock,
    wakes: Arc<AtomicUsize>,
    /// Holds the control thread inside the next wake.
    wake_gate: Arc<Gate>,
    /// Holds a [`READER`] thread inside its next clock read.
    read_gate: Arc<Gate>,
    /// Each buffer rendered: its timestamp and its PCM's sample value.
    rendered: Arc<Mutex<Vec<(Duration, i16)>>>,
}

fn rig(options: SinkOptions) -> Rig {
    gst::init().expect("GStreamer");
    let clock = ManualClock::new();
    let wakes = Arc::new(AtomicUsize::new(0));
    let wake_gate = Arc::new(Gate::default());
    let read_gate = Arc::new(Gate::default());
    let (w, g, r) = (
        Arc::clone(&wakes),
        Arc::clone(&wake_gate),
        Arc::clone(&read_gate),
    );
    let options = options
        .output(Output::Null)
        .clock(clock.clone().upcast())
        .between_reads(move || {
            if std::thread::current().name() == Some(READER) {
                r.pass();
            }
        });
    let sink = AudioSink::open(
        FORMAT,
        options,
        Arc::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
            g.pass();
        }),
    )
    .expect("a null sink opens");
    let rig = Rig {
        sink,
        clock,
        wakes,
        wake_gate,
        read_gate,
        rendered: Arc::new(Mutex::new(Vec::new())),
    };
    rig.record_renders();
    rig
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

    /// Records what the current output renders (after a rebuild, the new one's).
    fn record_renders(&self) {
        let r = Arc::clone(&self.rendered);
        let element = self.sink.sink_element();
        element.set_property("signal-handoffs", true);
        element.connect("handoff", false, move |args| {
            let buffer = args[1].get::<gst::Buffer>().expect("a buffer");
            let pts = Duration::from_nanos(buffer.pts().map_or(0, |t| t.nseconds()));
            let map = buffer.map_readable().expect("readable");
            let value = i16::from_le_bytes([map[0], map[1]]);
            r.lock().unwrap().push((pts, value));
            None
        });
    }

    fn parts(&self) -> Parts {
        lock(&self.sink.shared.core)
            .parts
            .clone()
            .expect("an output")
    }

    /// Waits until the control thread has handled every command sent before.
    fn fence(&self) {
        let (tx, rx) = mpsc::channel();
        self.sink.control.send(Cmd::Fence(tx)).expect("running");
        rx.recv_timeout(Duration::from_secs(5)).expect("the fence");
    }

    /// The output loses its device: a resource error from the output element, as a device
    /// sink posts when its server goes away.
    fn lose_device(&self) {
        let element = self.sink.sink_element();
        let _ = element.post_message(self.device_error());
    }

    /// The resource error a device sink posts when its server goes away.
    fn device_error(&self) -> gst::Message {
        gst::message::Error::builder(gst::ResourceError::Write, "the device went away")
            .src(&self.sink.sink_element())
            .build()
    }

    /// What `appsrc` posts after the output fails: its streaming task stops on the error.
    fn stream_error(src: &gstreamer_app::AppSrc) {
        let _ = src.post_message(
            gst::message::Error::builder(gst::StreamError::Failed, "Internal data stream error.")
                .src(src)
                .build(),
        );
    }

    /// Holds every buffer before the output, so it never prerolls: no real AsyncDone comes.
    fn block_output(&self) {
        let pad = self.sink.sink_element().static_pad("sink").expect("a pad");
        pad.add_probe(
            gst::PadProbeType::BLOCK | gst::PadProbeType::BUFFER,
            |_, _| gst::PadProbeReturn::Ok,
        )
        .expect("a probe");
    }

    /// Sends the control thread a preroll's AsyncDone, as the bus would, tagged with the
    /// generation `appsrc` was flushed to when it was posted.
    fn async_done(&self, tag: u64) {
        let element = self.sink.sink_element();
        let message = gst::message::AsyncDone::builder().src(&element).build();
        self.sink
            .control
            .send(Cmd::Bus(message, tag))
            .expect("running");
    }

    fn applied(&self) -> Option<u64> {
        lock(&self.sink.shared.core).applied
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
    assert_eq!(rig.sink.reading().state, ClockState::Paused);
    // The pipeline pauses on the control thread: move the test's clock only after.
    rig.fence();
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
    // The clock runs on well past the PCM's end before the watchdog can see it run dry.
    rig.run_to(250);
    rig.clock.set_ms(600);
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
    let rig = rig(SinkOptions::default()
        .stall_after(ms(100))
        .start_timeout(ms(600)));
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    // The clock never moves: within the start timeout the device is still opening, not stuck,
    // and the time paused doesn't count toward it.
    std::thread::sleep(Duration::from_millis(300));
    rig.sink.pause();
    std::thread::sleep(Duration::from_millis(800));
    rig.sink.play();
    std::thread::sleep(Duration::from_millis(100));
    rig.fence();
    assert!(
        rig.sink.drain_events().is_empty(),
        "a slow start reported as a stall"
    );
}

#[test]
fn a_device_that_never_starts_is_a_stall_once_the_start_timeout_passes() {
    let rig = rig(SinkOptions::default()
        .stall_after(ms(100))
        .start_timeout(ms(300)));
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    let playing = Instant::now();
    // The clock never moves. Pressing play again and again doesn't restart the count.
    let mut events = Vec::new();
    while events.is_empty() {
        assert!(
            playing.elapsed() < ms(1_500),
            "a device that never started went unreported"
        );
        std::thread::sleep(Duration::from_millis(100));
        rig.sink.pause();
        rig.fence();
        rig.sink.play();
        events = rig.sink.drain_events();
    }
    assert_eq!(events, [SinkEvent::Stalled { at: Duration::ZERO }]);
    assert!(
        playing.elapsed() >= ms(300),
        "reported inside the start timeout"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(rig.sink.drain_events().is_empty(), "one stall, one event");
    assert_eq!(rig.wakes(), 2);
}

#[test]
fn an_output_that_never_takes_its_first_pcm_is_a_stall_once_the_start_timeout_passes() {
    let rig = rig(SinkOptions::default().start_timeout(ms(300)));
    rig.block_output();
    push(&rig.sink, 500, 1);
    std::thread::sleep(Duration::from_millis(150));
    rig.fence();
    assert!(
        rig.sink.drain_events().is_empty(),
        "nothing inside the start timeout"
    );
    assert_eq!(rig.next_event(), SinkEvent::Stalled { at: Duration::ZERO });
    assert_eq!(rig.sink.reading().state, ClockState::Loading);
}

#[test]
fn reopening_then_seeking_in_the_same_frame_opens_a_new_output() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(300);
    rig.position_near(ms(300));
    let old = rig.parts();
    // The host handles DeviceLost (reopen), then the player's seek, before the control thread
    // has gone on from the wake.
    rig.wake_gate.arm();
    rig.lose_device();
    rig.wake_gate.wait_held();
    rig.sink.reopen();
    rig.sink.seek(ms(2_000));
    rig.wake_gate.release();
    assert!(matches!(rig.next_event(), SinkEvent::DeviceLost { .. }));
    eventually("a new output", || rig.parts().sink != old.sink);
    eventually("the old pipeline stopped", || {
        old.pipeline.current_state() == gst::State::Null
    });
    rig.take_rendered();
    rig.record_renders();
    push(&rig.sink, 500, 2);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: ms(2_000) });
    eventually("playing again", || {
        rig.sink.reading().state == ClockState::Playing
    });
    rig.run_to(500);
    rig.position_near(ms(2_200));
    assert!(rig.take_rendered().iter().all(|&(_, v)| v == 2));
}

#[test]
fn two_seeks_after_a_failure_open_a_new_output() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    let old = rig.parts();
    rig.wake_gate.arm();
    rig.lose_device();
    rig.wake_gate.wait_held();
    rig.sink.seek(ms(1_000));
    rig.sink.seek(ms(2_000));
    rig.wake_gate.release();
    assert!(matches!(rig.next_event(), SinkEvent::DeviceLost { .. }));
    eventually("a new output", || rig.parts().sink != old.sink);
    push(&rig.sink, 500, 2);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: ms(2_000) });
}

#[test]
fn one_failure_is_one_event_and_one_wake() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    // A lost device, then appsrc's own error as its streaming task stops on it.
    rig.lose_device();
    Rig::stream_error(&rig.parts().src);
    rig.fence();
    let events = rig.sink.drain_events();
    assert!(
        matches!(events.as_slice(), [SinkEvent::DeviceLost { .. }]),
        "{events:?}"
    );
    assert_eq!(rig.wakes(), 2);
    assert_eq!(rig.sink.reading().state, ClockState::Failed);
}

#[test]
fn an_error_from_before_a_reopen_leaves_the_new_output_alone() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    let src = rig.parts().src;
    rig.wake_gate.arm();
    rig.lose_device();
    rig.wake_gate.wait_held();
    // The old pipeline's second error is queued behind the first, and the host reopens.
    Rig::stream_error(&src);
    rig.sink.reopen();
    rig.wake_gate.release();
    assert!(matches!(rig.next_event(), SinkEvent::DeviceLost { .. }));
    rig.fence();
    assert_eq!(rig.sink.reading().state, ClockState::Seeking);
    push(&rig.sink, 500, 2);
    assert!(matches!(rig.next_event(), SinkEvent::SeekLanded { .. }));
    eventually("playing again", || {
        rig.sink.reading().state == ClockState::Playing
    });
}

#[test]
fn a_device_error_read_after_a_seek_fails_the_output_that_seek_flushes() {
    let rig = rig(SinkOptions::default());
    rig.sink.play();
    let old = rig.parts();
    rig.wake_gate.arm();
    push(&rig.sink, 1_000, 1);
    rig.wake_gate.wait_held();
    // The device goes away and the app seeks while the control thread is inside Ready's wake:
    // it reads the error, tagged with the generation before the seek, after the seek. The
    // seek flushes this same pipeline, so the error is this output's.
    rig.lose_device();
    rig.sink.seek(ms(2_000));
    rig.wake_gate.release();
    rig.fence();
    let events = rig.sink.drain_events();
    assert!(
        matches!(
            events.as_slice(),
            [SinkEvent::Ready, SinkEvent::DeviceLost { .. }]
        ),
        "{events:?}"
    );
    assert_eq!(rig.sink.reading().state, ClockState::Failed);
    // The host reopens: a new output, where the seek went.
    rig.sink.reopen();
    eventually("a new output", || rig.parts().sink != old.sink);
    push(&rig.sink, 500, 2);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: ms(2_000) });
}

#[test]
fn a_device_error_tagged_before_a_flush_fails_the_flushed_output() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.sink.seek(ms(2_000));
    rig.fence();
    // The output the flush kept posted an error before the flush moved the tag on.
    rig.sink
        .control
        .send(Cmd::Bus(rig.device_error(), 0))
        .expect("running");
    assert!(matches!(rig.next_event(), SinkEvent::DeviceLost { .. }));
    assert_eq!(rig.sink.reading().state, ClockState::Failed);
}

#[test]
fn a_seek_out_of_a_failure_leaves_the_failed_pipeline_alone() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    let old = rig.parts();
    rig.wake_gate.arm();
    rig.lose_device();
    rig.wake_gate.wait_held();
    // The dead output refuses any state change, and the app seeks before the control thread
    // goes on from the wake: only the rebuild the seek owes may touch the old pipeline.
    let refusing = Refusing::new();
    old.pipeline.add(&refusing).expect("added");
    refusing.sync_state_with_parent().expect("playing");
    rig.sink.seek(ms(2_000));
    rig.wake_gate.release();
    assert!(matches!(rig.next_event(), SinkEvent::DeviceLost { .. }));
    eventually("a new output", || rig.parts().sink != old.sink);
    rig.fence();
    let events = rig.sink.drain_events();
    assert!(
        events.is_empty(),
        "the old pipeline failed the seek: {events:?}"
    );
    assert_eq!(rig.sink.reading().state, ClockState::Seeking);
    push(&rig.sink, 500, 2);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: ms(2_000) });
    // The rebuild's teardown was refused too: stop the old pipeline here.
    refusing.allow();
    let _ = old.pipeline.set_state(gst::State::Null);
}

#[test]
fn a_reading_never_mixes_two_generations() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(300);
    rig.position_near(ms(300));
    // A read on another thread, parked between the device's position and the reading, while
    // a seek and its first PCM come in between. The output takes nothing more, so the seek
    // stays in flight.
    rig.block_output();
    rig.read_gate.arm();
    let clock = rig.sink.clock();
    let reader = std::thread::Builder::new()
        .name(READER.into())
        .spawn(move || clock.reading())
        .expect("a reader");
    rig.read_gate.wait_held();
    rig.sink.seek(ms(5_000));
    push(&rig.sink, 500, 2);
    rig.read_gate.release();
    let r = reader.join().expect("read");
    assert_eq!(
        (r.state, r.position, r.generation),
        (ClockState::Seeking, ms(5_000), 1)
    );
}

#[test]
fn volume_and_mute_reach_the_pcm_played_and_survive_a_new_output() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_500, 1_000);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(100);
    // A buffer already past the volume element may still render at the old gain.
    let settles_at = |want: i16| {
        let rendered = rig.take_rendered();
        let after: Vec<i16> = rendered
            .iter()
            .map(|&(_, v)| v)
            .skip_while(|&v| v != want)
            .collect();
        assert!(
            !after.is_empty() && after.iter().all(|&v| v == want),
            "rendered {rendered:?}, not settling at {want}"
        );
    };
    rig.sink.set_volume(0.5, false);
    rig.take_rendered();
    rig.run_to(400);
    settles_at(500);
    rig.sink.set_volume(0.8, true);
    rig.run_to(700);
    settles_at(0);

    rig.sink.reopen();
    eventually("seeking", || {
        rig.sink.reading().state == ClockState::Seeking
    });
    rig.fence();
    rig.take_rendered();
    rig.record_renders();
    push(&rig.sink, 500, 1_000);
    assert!(matches!(rig.next_event(), SinkEvent::SeekLanded { .. }));
    rig.run_to(900);
    let rendered = rig.take_rendered();
    assert!(!rendered.is_empty());
    assert!(
        rendered.iter().all(|&(_, v)| v == 0),
        "mute lost on the new output: {rendered:?}"
    );
}

#[test]
fn dropping_the_sink_while_playing_ends_its_thread_and_stops_its_pipeline() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    rig.run_to(300);
    rig.position_near(ms(300));
    let pipeline = rig.parts().pipeline;
    let clock = rig.sink.clock();
    drop(rig.sink);
    // Only this reader holds the shared state once the control thread has returned.
    eventually("the control thread ended", || {
        Arc::strong_count(&clock.shared) == 1
    });
    assert_eq!(pipeline.current_state(), gst::State::Null);
    let r = clock.reading();
    assert_eq!(r.state, ClockState::Paused);
    assert!(ms(300) - r.position <= SLACK, "held at {:?}", r.position);
}

#[test]
fn dropping_the_sink_while_seeking_ends_its_thread_and_stops_its_pipeline() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 1_000, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    let pipeline = rig.parts().pipeline;
    let clock = rig.sink.clock();
    rig.sink.seek(ms(5_000));
    drop(rig.sink);
    eventually("the control thread ended", || {
        Arc::strong_count(&clock.shared) == 1
    });
    assert_eq!(pipeline.current_state(), gst::State::Null);
    let r = clock.reading();
    assert_eq!((r.state, r.position), (ClockState::Seeking, ms(5_000)));
}

#[test]
fn an_async_done_from_before_a_seek_never_lands_it() {
    let rig = rig(SinkOptions::default());
    rig.block_output();
    // Open, push and seek at once: the first generation's preroll can finish behind the
    // seek's flush.
    push(&rig.sink, 300, 1);
    rig.sink.seek(ms(5_000));
    push(&rig.sink, 300, 2);
    eventually("the seek's flush", || rig.applied() == Some(1));
    rig.async_done(0);
    rig.fence();
    assert!(
        rig.sink.drain_events().is_empty(),
        "a stale preroll landed the seek"
    );
    assert_eq!(rig.sink.reading().state, ClockState::Seeking);
    rig.async_done(1);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: ms(5_000) });
}

#[test]
fn an_async_done_before_any_pcm_never_lands_a_seek() {
    let rig = rig(SinkOptions::default());
    rig.block_output();
    rig.sink.seek(ms(5_000));
    eventually("the seek's flush", || rig.applied() == Some(1));
    rig.async_done(1);
    rig.fence();
    assert!(rig.sink.drain_events().is_empty(), "landed with no PCM");
    assert_eq!(rig.sink.reading().state, ClockState::Seeking);
    push(&rig.sink, 300, 2);
    rig.async_done(1);
    assert_eq!(rig.next_event(), SinkEvent::SeekLanded { at: ms(5_000) });
}

#[test]
fn after_an_underrun_the_clock_plays_on_from_where_it_held() {
    let rig = rig(SinkOptions::default());
    push(&rig.sink, 300, 1);
    rig.sink.play();
    assert_eq!(rig.next_event(), SinkEvent::Ready);
    // Stop the clock a few ms short of the PCM's end: inside the underrun margin.
    rig.run_to(295);
    let SinkEvent::Underrun { .. } = rig.next_event() else {
        panic!("not an underrun");
    };
    // Where the pipeline paused (the clock may have stepped on since the watchdog's read).
    rig.fence();
    let held = rig.sink.reading().position;
    assert!(held < ms(298), "held at {held:?}, past the margin");
    rig.run_to(500);
    push(&rig.sink, 500, 1);
    assert_eq!(rig.next_event(), SinkEvent::Resumed { at: held });
    // The PCM left between there and the end plays: the clock goes on from where it held.
    rig.run_to(600);
    rig.position_near(held + ms(100));
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
