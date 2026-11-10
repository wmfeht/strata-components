//! Media output: an audio sink with a playback clock, and a frame pacer that shows video on
//! that clock.
//!
//! Rune lacks: any audio output on Linux. `Player` has no Linux backend (it reports
//! "unsupported"), and Rune has no API that plays PCM or reports a playback clock. Video needs
//! nothing new: `Pixels::set` replaces an image from any thread without a view commit.
//!
//! Rune issue: none of its own. Related: upstream
//! [#30](https://github.com/anomalyco/rune/issues/30),
//! [#100](https://github.com/anomalyco/rune/issues/100) and
//! [#112](https://github.com/anomalyco/rune/issues/112) (a Linux `Player`). This module is a
//! sink and a clock for PCM an app decodes itself, which `Player` would not replace.
//!
//! Work package: P7, `docs/work/phase-1-leaves/P7-media-output.md`.
//!
//! **Rune dev 5d421fd: partly.** `Pixels` (in place, any thread, no commit) and
//! `rune::Waker` cover the video and the waking; nothing covers audio. Past the pin and not
//! adopted: `Tx::clock` and `Tx::wake_at` (c650fe0c), which would let a host wake at a
//! frame's time on Rune's clock, and the headless `--realtime` flag (b726a4d1).
//!
//! # The shape
//!
//! - [`AudioSink`] takes interleaved PCM in a [`PcmFormat`] and plays it. Its clock
//!   ([`AudioSink::clock`], a [`SinkClock`] for any thread) is the device's: the position is
//!   what has been heard. It plays, pauses, seeks and flushes, and reports what it does on its
//!   own as [`SinkEvent`]s, waking the app once for each.
//! - [`WallClock`] is the same clock for clips without sound, on the system's monotonic time.
//! - [`FramePacer`] takes time-stamped RGBA [`VideoFrame`]s and, each time the app presents,
//!   writes the one the clock is at into a `rune::Pixels`, dropping frames whose time passed
//!   and holding the picture while the next isn't due.
//!
//! Both clocks implement [`MediaClock`], whose [`Reading`] is a position, a [`ClockState`] and
//! a generation that counts seeks.
//!
//! ```ignore
//! // Setup: open the sink with Rune's setup waker.
//! s.sink = AudioSink::open(PcmFormat::STEREO_S16_48K, SinkOptions::default(), waker)?;
//! s.pacer = FramePacer::new(s.frames.clone(), 3);
//!
//! // The decoder's output, as it arrives: PCM to the sink, frames to the pacer.
//! s.sink.push(pcm)?;                    // Err(Full): try again on a later poll
//! s.pacer.push(frame)?;                 // Err((frame, Full)): keep it for later
//!
//! // Controls: play, pause and seek on the sink. A seek restarts the decoder at the target.
//! s.sink.seek(to);
//! s.pacer.flush();
//!
//! // poll, each frame while playing: show the frame, handle what the clock did.
//! s.pacer.present(&s.sink.clock());
//! for event in s.sink.drain_events() { /* Ended, DeviceLost: s.sink.reopen(), … */ }
//! ```
//!
//! # Threads and waking
//!
//! The sink's calls never block on the device. GStreamer streams on its own threads, and state
//! changes, flushes and rebuilds run on the sink's control thread ([`THREAD_NAME`]), which also
//! reads the clock every 20 ms while playing to see the PCM run out. Every change the clock
//! makes on its own (the first PCM ready, a seek landing, an underrun and the resume after it,
//! a stall, the end, a failure) queues a [`SinkEvent`] and calls the waker, so an app's `poll`
//! runs and its view shows the new state even in a resting window. Changes the app asks for
//! (play, pause, the start of a seek) wake nothing: the app made them.
//!
//! The pacer runs where the app presents, normally `poll`. While playing the app polls each
//! frame anyway; `FramePacer::next_due` says when the next frame shows for an app that would
//! rather wake for it.
//!
//! # Seeking and underruns
//!
//! A seek starts a generation. The control thread pauses the pipeline, flushes `appsrc`
//! (`flush-stop` with `reset_time`), and lets in the PCM pushed since the seek, whose
//! timestamps start again at 0; the clock reads the seek's target plus the device's position
//! in the generation. A probe on GStreamer 1.28 (fakesink on a manual clock) showed the running
//! time restart at once after the flush, with no PCM from before it rendered, while playing and
//! while paused, so no seek rebuilds the pipeline. Only a failed output is rebuilt
//! ([`AudioSink::reopen`]).
//!
//! The same probe showed that PCM pushed after the sink ran dry plays late (the running time
//! went on without it), so on an underrun the sink pauses its pipeline, and plays again once
//! 200 ms is queued or the stream is finished.
//!
//! # Why GStreamer
//!
//! The choice was GStreamer's `appsrc` or a PipeWire stream; both play without GTK.
//!
//! | | GStreamer `appsrc` (chosen) | PipeWire `pw_stream` |
//! | --- | --- | --- |
//! | Crates | `gstreamer`, `gstreamer-app` 0.25 (MIT OR Apache-2.0; the `-sys` crates MIT), on the glib 0.22 `glib-bridge` already uses | `pipewire`, `libspa` 0.10 (MIT) |
//! | Build | pkg-config only | bindgen, so libclang on every build machine |
//! | System libraries | GStreamer and gst-plugins-base (LGPL-2.1-or-later, linked dynamically); gst-plugins-good for `autoaudiosink` | libpipewire-0.3 (MIT) |
//! | Packaging | Arch `gstreamer gst-plugins-base gst-plugins-good`; Ubuntu `libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev` to build, `gstreamer1.0-plugins-base gstreamer1.0-plugins-good` to run; Fedora `gstreamer1-devel gstreamer1-plugins-base-devel`; the Flatpak GNOME and Freedesktop runtimes ship it, playing through the usual `--socket=pulseaudio` | A PipeWire daemon at run time: none on a PulseAudio or plain ALSA desktop, and in Flatpak the PipeWire socket, which isn't the usual audio permission |
//! | Outputs | PipeWire, PulseAudio or ALSA, whichever `autoaudiosink` finds | PipeWire only |
//! | Latency | The device sink buffers about 200 ms, and starting takes one preroll; sync doesn't suffer, since the clock is the sink's and counts the device's delay | One quantum, about 21 ms at 48 kHz |
//! | Clock | The sink's audio clock, which follows the samples played; its position is what's heard | Exact (`pw_stream_get_time_n`), but the position and delay arithmetic is the app's |
//! | Seek and flush | Flush events on `appsrc`; see above | `pw_stream_flush` on a ring buffer of our own |
//! | Device loss | An error from the sink on the bus; PulseAudio and PipeWire sinks follow a change of default device on their own | Stream and core errors, and reconnecting is ours |
//! | Threads | GStreamer's own; state changes on a control thread | A real-time callback, which needs a lock-free ring buffer |
//! | Tests | Headless and deterministic: `fakesink` on a manual clock, no sound server, in CI | A daemon with a null sink in every test session |
//!
//! GStreamer wins on the tests (they run in CI without a sound server), on reach (every Linux
//! audio stack, and Flatpak), on the build (no libclang), and because it is what the app this
//! was written for already ships. PipeWire's lower latency doesn't help a clock that already
//! counts the delay. cpal (Apache-2.0) was also looked at: on Linux it is an ALSA client, with
//! no format conversion or resampling, and its clock would be ours to build from
//! `snd_pcm_delay`.
//!
//! What the sink needs at run time: `appsrc`, `audioconvert`, `audioresample` and `volume`
//! (gst-plugins-base) and, for [`Output::Default`], `autoaudiosink` (gst-plugins-good) with a
//! device sink (`pipewiresink`, `pulsesink` or `alsasink`). [`Output::Null`] needs only
//! `fakesink`, from GStreamer itself. A missing element is [`MediaError::MissingElement`].
//!
//! # Tests
//!
//! The pacer's unit tests run on a fake clock. The sink's run headless on [`Output::Null`]
//! driven by a manual GStreamer clock, so they open no sound server. One test plays a tone on
//! the default output and is for a person to run: it is ignored, and plays only with
//! `STRATA_PLATFORM_TONE=1` (`docs/testing.md`).

mod clock;
mod format;
mod pacer;
mod sink;

pub use clock::{ClockState, MediaClock, Reading, WallClock};
pub use format::{FormatError, PcmFormat, SampleFormat};
pub use pacer::{FramePacer, PacerStats, Presented, Refused, VideoFrame};
pub use sink::{
    AudioSink, MediaError, Output, PushError, SinkClock, SinkEvent, SinkOptions, THREAD_NAME,
};
