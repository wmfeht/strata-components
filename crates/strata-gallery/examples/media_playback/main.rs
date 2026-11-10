//! C13's MediaPlayer on P7's audio sink and frame pacer: the hand test C13 waited for.
//! `adapter.rs` is the glue an app copies as it is.
//!
//! ```text
//! cargo run -p strata-gallery --example media_playback [-- --null]
//! ```
//!
//! It plays a generated 60 s clip, a decoder stand-in: 440 Hz with an 880 Hz beep for the
//! first 60 ms of each second, and a 30 fps ramp that flashes white on the same seconds. The
//! player's own controls drive it: the frame or Ctrl+Alt+Space plays and pauses, the timeline
//! and Ctrl+Alt+Left and Right seek, the volume slider and mute reach the sink. In sync, each
//! flash comes with its beep. `--null` plays nowhere (`Output::Null`). Closing the window
//! prints the sync report on stderr: how far the clock was past the frame on screen.

mod adapter;

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rune::{App, AppSpec, Cx, El, KeyEvent, Pixels, Tx, col};
use strata_components::media_player::{Clip, MediaError, MediaKind, MediaPlayer, MediaState};
use strata_kit::look::Look;
use strata_platform::media::{
    AudioSink, ClockState, FramePacer, MediaClock, Output, PcmFormat, Presented, SinkEvent,
    SinkOptions, VideoFrame,
};

use adapter::ClockAdapter;

const FORMAT: PcmFormat = PcmFormat::STEREO_S16_48K;
const CLIP: Duration = Duration::from_secs(60);
const FPS: u64 = 30;
const SIZE: [u32; 2] = [320, 180];
const CHUNK: u64 = FORMAT.rate as u64 / FPS;

static NULL: AtomicBool = AtomicBool::new(false);

/// The generated clip from a point on.
struct Decoder {
    /// The next PCM frame and the next video frame, from the clip's start.
    pcm: u64,
    frame: u64,
    finished: bool,
}

impl Decoder {
    fn at(at: Duration) -> Decoder {
        Decoder {
            pcm: FORMAT.frames_in(at),
            frame: (at.as_nanos() * u128::from(FPS) / 1_000_000_000) as u64,
            finished: false,
        }
    }

    fn feed(&mut self, sink: &AudioSink, pacer: &mut FramePacer) {
        let end = FORMAT.frames_in(CLIP);
        while self.pcm < end && sink.room() > FORMAT.duration_of(CHUNK) {
            let n = CHUNK.min(end - self.pcm);
            let mut pcm = Vec::with_capacity(n as usize * FORMAT.frame_bytes());
            for k in self.pcm..self.pcm + n {
                let beep = k % u64::from(FORMAT.rate) < FORMAT.frames_in(Duration::from_millis(60));
                let (hz, gain) = if beep { (880.0, 0.3) } else { (440.0, 0.12) };
                let t = k as f64 / f64::from(FORMAT.rate);
                let v =
                    ((t * hz * std::f64::consts::TAU).sin() * gain * f64::from(i16::MAX)) as i16;
                pcm.extend_from_slice(&v.to_le_bytes());
                pcm.extend_from_slice(&v.to_le_bytes());
            }
            if sink.push(pcm).is_err() {
                break;
            }
            self.pcm += n;
        }
        if self.pcm >= end && !self.finished {
            sink.finish();
            self.finished = true;
        }
        while pacer.has_room() && self.frame * 1_000 / FPS < CLIP.as_millis() as u64 {
            let pts = Duration::from_nanos(self.frame * 1_000_000_000 / FPS);
            let flash = self.frame.is_multiple_of(FPS);
            let [w, h] = SIZE;
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for _y in 0..h {
                for x in 0..w {
                    let v = ((x + self.frame as u32 * 4) % w * 255 / w) as u8;
                    let px = if flash {
                        [255, 255, 255]
                    } else {
                        [v, 255 - v, 160]
                    };
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
            if pacer
                .push(VideoFrame {
                    pts,
                    width: w,
                    height: h,
                    rgba,
                })
                .is_err()
            {
                break;
            }
            self.frame += 1;
        }
    }
}

struct Host {
    look: Look,
    media: MediaState,
    frames: Pixels,
    clock: ClockAdapter,
    sink: Option<AudioSink>,
    pacer: FramePacer,
    decoder: Decoder,
    /// Clock position minus the shown frame's time while playing, in µs.
    offsets: Vec<i64>,
}

fn init() -> Host {
    let frames = Pixels::new();
    let mut media = MediaState::default();
    media.set_clip(Clip {
        kind: MediaKind::Video,
        name: "tone-and-ramp.mkv".into(),
        position: None,
        has_previous: false,
        has_next: false,
    });
    media.set_poster(None, Some(16.0 / 9.0));
    // The waveform: the tone's level, louder at each second's beep (Strata's 1024 buckets).
    let levels: Vec<f32> = (0..1024)
        .map(|i| if i * 60 % 1024 < 60 { 0.9 } else { 0.4 })
        .collect();
    media.set_levels(1024, 0, &levels);
    media.settle_levels();
    Host {
        look: Look::fallback(true),
        media,
        pacer: FramePacer::new(frames.clone(), 3),
        frames,
        clock: ClockAdapter::new(Some(CLIP)),
        sink: None,
        decoder: Decoder::at(Duration::ZERO),
        offsets: Vec::new(),
    }
}

impl Host {
    fn apply(&mut self, intent: strata_components::media_player::MediaIntent) {
        let Some(sink) = &self.sink else { return };
        if let Some(at) = adapter::apply(sink, &self.clock, &intent) {
            self.pacer.flush();
            self.decoder = Decoder::at(at);
        }
    }

    fn report(&self) -> String {
        let mut o = self.offsets.clone();
        o.sort_unstable();
        let pick = |q: f64| {
            o.get(((o.len().max(1) - 1) as f64 * q) as usize)
                .map_or(0.0, |us| *us as f64 / 1000.0)
        };
        let s = self.pacer.stats();
        format!(
            "c13 on p7: offset p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms, min {:.1} ms over {} presents; frames shown {}, dropped {}, repeated {}\n",
            pick(0.5),
            pick(0.95),
            pick(1.0),
            pick(0.0),
            o.len(),
            s.shown,
            s.dropped,
            s.repeated
        )
    }
}

fn setup(h: &mut Host, waker: rune::Waker) {
    let output = if NULL.load(Ordering::Relaxed) {
        Output::Null
    } else {
        Output::Default
    };
    match AudioSink::open(FORMAT, SinkOptions::default().output(output), waker) {
        Ok(sink) => h.sink = Some(sink),
        Err(e) => h.media.set_error(Some(MediaError {
            title: "No audio output".into(),
            detail: e.to_string(),
        })),
    }
}

/// The host's frame, in C13's order: the clock, the tick, then the intents.
fn poll(h: &mut Host, now: f64, tx: &mut Tx) -> bool {
    let Some(sink) = &h.sink else { return false };
    let mut changed = false;
    for event in sink.drain_events() {
        match event {
            SinkEvent::DeviceLost { .. } => {
                sink.reopen();
                let at = sink.reading().position;
                h.pacer.flush();
                h.decoder = Decoder::at(at);
            }
            SinkEvent::Error { message } => {
                h.media.set_error(Some(MediaError {
                    title: "Playback failed".into(),
                    detail: message,
                }));
                changed = true;
            }
            _ => {}
        }
    }
    h.decoder.feed(sink, &mut h.pacer);
    // The first frame shown ends the stand-in.
    if let Presented::Shown { .. } = h.pacer.present(sink)
        && !h.media.has_frame()
    {
        h.media.set_frame(Some(SIZE));
        changed = true;
    }
    h.clock.refresh(sink);
    let r = h.clock.reading();
    if r.state == ClockState::Playing
        && let Some(shown) = h.pacer.shown()
    {
        h.offsets
            .push(r.position.as_micros() as i64 - shown.as_micros() as i64);
    }
    changed |= h.media.tick(&h.clock, now, tx);
    for intent in h.media.drain_intents() {
        h.apply(intent);
    }
    changed
}

fn on_key(h: &mut Host, key: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(command) = h.media.keys().window_command(key, tx) else {
        return false;
    };
    h.media.command(command);
    for intent in h.media.drain_intents() {
        h.apply(intent);
    }
    tx.wake_after(Duration::ZERO);
    true
}

fn view(h: &Host, cx: &mut Cx<Host>) -> El {
    let player = MediaPlayer::new(
        &h.media,
        |h: &mut Host| &mut h.media,
        |h: &mut Host| &mut h.media.viewport,
    )
    .key("media")
    .video(&h.frames)
    .on_intent(|h: &mut Host, intent, _tx| h.apply(intent))
    .build(&h.look, cx);
    col()
        .w_full()
        .h_full()
        .fill(h.look.kit.c.bg_base)
        .child(player)
}

fn main() {
    let args = rune::tooling::extra_args();
    NULL.store(args.iter().any(|a| a == "--null"), Ordering::Relaxed);
    rune::tooling::set_app_id("strata-media-playback");
    let spec = AppSpec {
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("media playback", (760.0, 620.0), init, view)
    };
    App::new(spec)
        .on_close(|h, _| {
            let _ = std::io::stderr().lock().write_all(h.report().as_bytes());
        })
        .run();
}
