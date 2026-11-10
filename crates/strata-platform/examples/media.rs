//! Plays a generated clip, a tone and a video ramp, on the `media-audio` sink and pacer, to hear
//! and see that they keep in sync.
//!
//! The clip is 60 s. The tone is 440 Hz with an 880 Hz beep for the first 60 ms of every
//! second; the video is a scrolling ramp at 30 fps whose frame at each whole second flashes
//! white, with a bar along the bottom that fills over each second. In sync, each flash comes
//! with its beep, through pauses and seeks.
//!
//! Keys: Space plays or pauses (from the end it starts again), Left and Right seek 5 s, S seeks
//! somewhere else. The line under the picture shows the clock, its state and how far the clock
//! is past the frame on screen; closing the window prints the sync report on stderr.
//!
//! - `cargo run -p strata-platform --features media-audio --example media` plays on the
//!   default output.
//! - `--null` plays nowhere (`Output::Null`), at the real rate.
//! - `--measure` opens no window and plays nowhere: it plays, pauses and seeks on a script for
//!   about 12 s, presenting at 60 Hz, and prints the sync report.
//!
//! The report: the offset (clock position minus the shown frame's time, at each present while
//! playing) at p50, p95 and max, its minimum (below zero would be a frame shown early), and
//! the pacer's shown, dropped and repeated counts. With 30 fps video presented at 60 Hz the
//! offset spreads over one frame interval, 0 to 33 ms; the display's own latency isn't in it.

#[cfg(feature = "media-audio")]
mod demo {
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use rune::{
        App, AppSpec, Cx, El, Key, KeyEvent, LiveText, Pixels, SurfaceFit, Tx, col, el, text,
    };
    use strata_platform::media::{
        AudioSink, ClockState, FramePacer, MediaClock, Output, PcmFormat, SinkEvent, SinkOptions,
        VideoFrame,
    };

    const FORMAT: PcmFormat = PcmFormat::STEREO_S16_48K;
    const CLIP: Duration = Duration::from_secs(60);
    const FPS: u32 = 30;
    const SIZE: [u32; 2] = [320, 180];
    /// PCM goes in chunks of one video frame's time, as a decoder sends it.
    const CHUNK_FRAMES: u64 = FORMAT.rate as u64 / FPS as u64;
    const BEEP: Duration = Duration::from_millis(60);

    static NULL: AtomicBool = AtomicBool::new(false);

    /// The generated clip from a point on: the "decoder".
    struct Generator {
        /// PCM frames made since `start`.
        pcm: u64,
        /// The next video frame's index.
        frame: u64,
        start: Duration,
    }

    impl Generator {
        fn at(start: Duration) -> Generator {
            // The frame on screen at `start`: the last one at or before it.
            let frame = (start.as_nanos() * u128::from(FPS) / 1_000_000_000) as u64;
            Generator {
                pcm: 0,
                frame,
                start,
            }
        }

        fn pcm_done(&self) -> bool {
            self.start + FORMAT.duration_of(self.pcm) >= CLIP
        }

        fn next_pcm(&mut self) -> Vec<u8> {
            let start = FORMAT.frames_in(self.start);
            let end = FORMAT.frames_in(CLIP);
            let n = CHUNK_FRAMES.min(end.saturating_sub(start + self.pcm));
            let mut pcm = Vec::with_capacity(n as usize * FORMAT.frame_bytes());
            for i in 0..n {
                let k = start + self.pcm + i;
                let t = k as f64 / f64::from(FORMAT.rate);
                let beep = (k % u64::from(FORMAT.rate)) < FORMAT.frames_in(BEEP);
                let hz = if beep { 880.0 } else { 440.0 };
                let gain = if beep { 0.3 } else { 0.12 };
                let v =
                    ((t * hz * std::f64::consts::TAU).sin() * gain * f64::from(i16::MAX)) as i16;
                pcm.extend_from_slice(&v.to_le_bytes());
                pcm.extend_from_slice(&v.to_le_bytes());
            }
            self.pcm += n;
            pcm
        }

        fn frame_pts(&self) -> Duration {
            Duration::from_nanos(self.frame * 1_000_000_000 / u64::from(FPS))
        }

        fn video_done(&self) -> bool {
            self.frame_pts() >= CLIP
        }

        fn next_frame(&mut self) -> VideoFrame {
            let pts = self.frame_pts();
            let [w, h] = SIZE;
            let flash = self.frame.is_multiple_of(u64::from(FPS));
            let into_second = (self.frame % u64::from(FPS)) as f32 / FPS as f32;
            let shift = (self.frame * 4) as u32;
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let px = if flash {
                        [255, 255, 255]
                    } else if y >= h - 12 {
                        if (x as f32) < into_second * w as f32 {
                            [240, 200, 40]
                        } else {
                            [30, 30, 30]
                        }
                    } else {
                        let v = ((x + shift) % w * 255 / w) as u8;
                        [v, 255 - v, 160]
                    };
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
            self.frame += 1;
            VideoFrame {
                pts,
                width: w,
                height: h,
                rgba,
            }
        }
    }

    /// The sink, the pacer, the generator and the sync record: what a real host holds.
    struct Player {
        sink: AudioSink,
        pacer: FramePacer,
        generator: Generator,
        finished: bool,
        /// Clock position minus the shown frame's time, in µs, at each present while playing.
        offsets: Vec<i64>,
        seed: u64,
    }

    impl Player {
        fn open(pixels: Pixels, output: Output, waker: rune::Waker) -> Result<Player, String> {
            let sink = AudioSink::open(FORMAT, SinkOptions::default().output(output), waker)
                .map_err(|e| e.to_string())?;
            Ok(Player {
                sink,
                pacer: FramePacer::new(pixels, 3),
                generator: Generator::at(Duration::ZERO),
                finished: false,
                offsets: Vec::new(),
                seed: 0x2545_f491_4f6c_dd1d,
            })
        }

        /// Pushes what fits: PCM up to the lookahead, frames up to the pacer's queue.
        fn feed(&mut self) {
            while !self.generator.pcm_done() && self.sink.room() > FORMAT.duration_of(CHUNK_FRAMES)
            {
                if self.sink.push(self.generator.next_pcm()).is_err() {
                    break;
                }
            }
            if self.generator.pcm_done() && !self.finished {
                self.sink.finish();
                self.finished = true;
            }
            while self.pacer.has_room() && !self.generator.video_done() {
                if self.pacer.push(self.generator.next_frame()).is_err() {
                    break;
                }
            }
        }

        /// One display frame: feed, show the due frame, record the offset.
        fn present(&mut self) {
            self.feed();
            self.pacer.present(&self.sink);
            let r = self.sink.reading();
            if r.state == ClockState::Playing
                && let Some(shown) = self.pacer.shown()
            {
                self.offsets
                    .push(r.position.as_micros() as i64 - shown.as_micros() as i64);
            }
        }

        fn toggle(&mut self) {
            match self.sink.reading().state {
                ClockState::Ended => {
                    self.seek(Duration::ZERO);
                    self.sink.play();
                }
                ClockState::Paused | ClockState::Loading => self.sink.play(),
                ClockState::Seeking => self.sink.play(),
                _ => self.sink.pause(),
            }
        }

        fn seek(&mut self, to: Duration) {
            let to = to.min(CLIP - Duration::from_millis(100));
            self.sink.seek(to);
            self.pacer.flush();
            self.generator = Generator::at(to);
            self.finished = false;
        }

        fn seek_by(&mut self, secs: i64) {
            let now = self.sink.reading().position.as_secs_f64();
            self.seek(Duration::from_secs_f64((now + secs as f64).max(0.0)));
        }

        fn seek_elsewhere(&mut self) {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            self.seek(Duration::from_millis(self.seed % 55_000));
        }

        /// Handles what the sink did on its own; a lost device reopens where it stopped.
        fn events(&mut self) -> Vec<SinkEvent> {
            let events = self.sink.drain_events();
            for event in &events {
                if let SinkEvent::DeviceLost { .. } = event {
                    self.sink.reopen();
                    let at = self.sink.reading().position;
                    self.pacer.flush();
                    self.generator = Generator::at(at);
                    self.finished = false;
                }
            }
            events
        }

        fn status(&self) -> String {
            let r = self.sink.reading();
            let offset = self.offsets.last().map_or(String::new(), |us| {
                format!(", clock {:.1} ms past the frame", *us as f64 / 1000.0)
            });
            format!("{:.2} s, {:?}{offset}", r.position.as_secs_f64(), r.state)
        }

        fn report(&self) -> String {
            let stats = self.pacer.stats();
            let mut o = self.offsets.clone();
            o.sort_unstable();
            let ms = |us: i64| us as f64 / 1000.0;
            let pick = |q: f64| {
                o.get(((o.len().max(1) - 1) as f64 * q) as usize)
                    .copied()
                    .unwrap_or(0)
            };
            format!(
                "p7 sync: offset p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms, min {:.1} ms over {} presents; \
                 frames shown {}, dropped {}, repeated {}\n",
                ms(pick(0.5)),
                ms(pick(0.95)),
                ms(pick(1.0)),
                ms(pick(0.0)),
                o.len(),
                stats.shown,
                stats.dropped,
                stats.repeated,
            )
        }
    }

    fn eprint(line: &str) {
        let _ = std::io::stderr().lock().write_all(line.as_bytes());
    }

    /// `--measure`: no window, no sound; a script of plays, pauses and seeks at 60 Hz.
    fn measure() {
        let mut p = match Player::open(Pixels::new(), Output::Null, Arc::new(|| {})) {
            Ok(p) => p,
            Err(e) => return eprint(&format!("media: {e}\n")),
        };
        let run = |p: &mut Player, d: Duration| {
            let until = Instant::now() + d;
            while Instant::now() < until {
                p.present();
                for event in p.events() {
                    eprint(&format!(
                        "  {:.3} s {event:?}\n",
                        p.sink.reading().position.as_secs_f64()
                    ));
                }
                std::thread::sleep(Duration::from_micros(16_667));
            }
        };
        p.sink.play();
        run(&mut p, Duration::from_secs(3));
        p.sink.pause();
        run(&mut p, Duration::from_millis(500));
        p.sink.play();
        run(&mut p, Duration::from_secs(1));
        p.seek(Duration::from_secs(20));
        run(&mut p, Duration::from_secs(2));
        p.seek_by(5);
        run(&mut p, Duration::from_secs(2));
        p.sink.pause();
        p.seek(Duration::from_secs(40));
        run(&mut p, Duration::from_millis(500));
        p.sink.play();
        run(&mut p, Duration::from_secs(2));
        p.seek(Duration::from_millis(59_000));
        run(&mut p, Duration::from_millis(1_500));
        eprint(&format!("  end: {}\n", p.status()));
        eprint(&p.report());
    }

    struct State {
        frames: Pixels,
        status: LiveText,
        player: Option<Player>,
        error: Option<String>,
    }

    fn init() -> State {
        State {
            frames: Pixels::new(),
            status: LiveText::new("loading"),
            player: None,
            error: None,
        }
    }

    fn setup(s: &mut State, waker: rune::Waker) {
        let output = if NULL.load(Ordering::Relaxed) {
            Output::Null
        } else {
            Output::Default
        };
        match Player::open(s.frames.clone(), output, waker) {
            Ok(p) => s.player = Some(p),
            Err(e) => s.error = Some(e),
        }
    }

    fn poll(s: &mut State, _now: f64, tx: &mut Tx) -> bool {
        let Some(p) = &mut s.player else {
            return false;
        };
        p.present();
        for event in p.events() {
            if let SinkEvent::Ended { .. } = event {
                eprint(&p.report());
            }
        }
        s.status.set(p.status());
        if !matches!(
            p.sink.reading().state,
            ClockState::Paused | ClockState::Ended | ClockState::Failed
        ) {
            tx.wake_after(Duration::from_millis(8));
        }
        // The picture and the status line are live: nothing to commit.
        false
    }

    fn on_key(s: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
        let Some(p) = &mut s.player else {
            return false;
        };
        match &key.key {
            Key::Text(t) if t == " " => p.toggle(),
            Key::Text(t) if t.eq_ignore_ascii_case("s") => p.seek_elsewhere(),
            Key::Left => p.seek_by(-5),
            Key::Right => p.seek_by(5),
            _ => return false,
        }
        tx.wake_after(Duration::ZERO);
        true
    }

    fn view(s: &State, _: &mut Cx<State>) -> El {
        let status = match &s.error {
            Some(e) => text(format!("no audio output: {e}")),
            None => text("loading").live_text(&s.status),
        };
        col()
            .w_full()
            .h_full()
            .center()
            .gap(12.0)
            .child(
                el().w(640.0)
                    .h(360.0)
                    .pixels_fit(&s.frames, SurfaceFit::Fit),
            )
            .child(status)
            .child(text(
                "Space: play or pause · Left, Right: 5 s · S: seek elsewhere",
            ))
    }

    fn on_close(s: &mut State, _: &mut Tx) {
        if let Some(p) = &s.player {
            eprint(&p.report());
        }
    }

    pub fn main() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.iter().any(|a| a == "--measure") {
            return measure();
        }
        NULL.store(args.iter().any(|a| a == "--null"), Ordering::Relaxed);
        rune::tooling::set_app_id("strata-platform-media-example");
        let spec = AppSpec {
            setup: Some(setup),
            poll: Some(poll),
            on_key: Some(on_key),
            ..AppSpec::new("media", (720.0, 480.0), init, view)
        };
        let argv =
            std::iter::once("media".to_owned()).chain(args.into_iter().filter(|a| a != "--null"));
        App::new(spec).args(argv).on_close(on_close).run();
    }
}

#[cfg(feature = "media-audio")]
fn main() {
    demo::main();
}

#[cfg(not(feature = "media-audio"))]
fn main() {
    eprintln!("run with --features media-audio");
}
