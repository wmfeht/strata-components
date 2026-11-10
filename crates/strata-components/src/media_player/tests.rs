use std::time::Duration;

use rune::{Chord, ChordKey, Key, KeyEvent, KeyPlatform, Mods};

use super::audio::layout::{ART_MIN, plan};
use super::audio::spectrum::{Ballistics, bar_count};
use super::badges::{BadgeKind, VideoDetails, badges};
use super::time::{
    clock, decode_request, hand_off_at, last_frame, needs_restart, snapped_volume, stem,
    track_caption,
};
use super::*;
use crate::viewers::layout::Position;

fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

fn key(k: Key, mods: Mods) -> KeyEvent {
    KeyEvent { key: k, mods }
}

fn text(t: &str, shift: bool) -> KeyEvent {
    key(
        Key::Text(t.into()),
        Mods {
            shift,
            ..Mods::default()
        },
    )
}

fn focused(keys: &MediaKeys, e: &KeyEvent) -> Option<MediaCommand> {
    keys.command_on(KeyScope::Focused, e, KeyPlatform::Linux)
}

fn window(keys: &MediaKeys, e: &KeyEvent) -> Option<MediaCommand> {
    keys.command_on(KeyScope::Window, e, KeyPlatform::Linux)
}

const CTRL_ALT: Mods = Mods {
    shift: false,
    cmd: true,
    alt: true,
    ctrl: true,
    logo: false,
};

// Strata's arithmetic.

#[test]
fn the_clock_reads_minutes_then_hours() {
    assert_eq!(clock(secs(65)), "1:05");
    assert_eq!(clock(secs(3725)), "1:02:05");
    assert_eq!(clock(Duration::from_millis(999)), "0:00");
}

#[test]
fn seeks_stop_on_the_last_tick_before_the_end() {
    // 200 s at 30 ticks a second: tick 5999, so 199.9667 s.
    let last = last_frame(secs(200));
    assert!(last < secs(200) && secs(200) - last <= Duration::from_micros(33_334));
    assert_eq!(last, Duration::from_micros(5999 * 1_000_000 / 30));
}

#[test]
fn stepped_volumes_land_on_the_grid_so_repeated_steps_reach_zero() {
    let mut v = 0.05_f32;
    for _ in 0..3 {
        v = snapped_volume(v - 0.1);
    }
    assert_eq!(v, 0.0);
    assert_eq!(snapped_volume(0.64), 0.6);
    assert_eq!(snapped_volume(1.3), 1.0);
}

// video.rs and playback.rs: the decode size.

#[test]
fn the_decode_size_is_the_frame_times_the_scale_within_16_and_1280() {
    assert_eq!(decode_request([400.0, 225.0], 2.0), [800, 450]);
    assert_eq!(decode_request([900.0, 600.0], 2.0), [1280, 1200]);
    assert_eq!(decode_request([4.0, 4.0], 1.0), [16, 16]);
}

#[test]
fn decoding_restarts_only_on_growth_of_eight_percent_and_never_on_a_shrink() {
    let decoded = [640, 360];
    assert!(
        !needs_restart(decoded, decoded, [680, 382]),
        "6 % is too little"
    );
    assert!(needs_restart(decoded, decoded, [700, 394]), "9 % restarts");
    assert!(
        !needs_restart(decoded, decoded, [320, 180]),
        "a shrink never restarts"
    );
    // A 640 × 360 source asked for 1280 × 720 decodes at its own size: the same request grown
    // still can't enlarge it.
    assert!(!needs_restart(decoded, [1280, 720], [1280, 720]));
    assert!(!needs_restart(decoded, [1280, 720], [1400, 788]));
    assert!(!needs_restart([0, 0], [0, 0], [1280, 720]));
}

#[test]
fn hand_off_keeps_the_position_only_away_from_either_end() {
    let d = Some(secs(100));
    assert_eq!(hand_off_at(secs(50), d), Some(secs(50)));
    assert_eq!(hand_off_at(Duration::from_millis(500), d), None);
    assert_eq!(hand_off_at(Duration::from_millis(99_500), d), None);
    assert_eq!(hand_off_at(secs(1), d), None, "1 s itself is the margin");
    assert_eq!(hand_off_at(secs(50), None), Some(secs(50)));
}

#[test]
fn captions_prefer_the_tagged_track_then_the_listing() {
    let p = Some(Position {
        index: 3,
        count: 12,
        results: false,
    });
    assert_eq!(
        track_caption(Some(3), Some(12), p).as_deref(),
        Some("Track 3 of 12")
    );
    assert_eq!(track_caption(Some(3), None, p).as_deref(), Some("Track 3"));
    assert_eq!(
        track_caption(None, Some(9), p).as_deref(),
        Some("3 of 12 in folder")
    );
    assert_eq!(track_caption(None, None, None), None);
    assert_eq!(stem("01 Low Tide.flac"), "01 Low Tide");
    assert_eq!(stem(".hidden"), ".hidden");
}

// The key map.

#[test]
fn the_default_timeline_keys_are_stratas() {
    let k = MediaKeys::default_map();
    let none = Mods::default();
    let shift = Mods {
        shift: true,
        ..none
    };
    let five = secs(5);
    let thirty = secs(30);
    let cases = [
        (key(Key::Left, none), MediaCommand::Seek(Seek::Back(five))),
        (
            key(Key::Right, none),
            MediaCommand::Seek(Seek::Forward(five)),
        ),
        (
            key(Key::Left, shift),
            MediaCommand::Seek(Seek::Back(thirty)),
        ),
        (
            key(Key::Right, shift),
            MediaCommand::Seek(Seek::Forward(thirty)),
        ),
        (
            key(Key::PageDown, none),
            MediaCommand::Seek(Seek::Back(thirty)),
        ),
        (
            key(Key::PageUp, none),
            MediaCommand::Seek(Seek::Forward(thirty)),
        ),
        (key(Key::Home, none), MediaCommand::Seek(Seek::Start)),
        (text("0", false), MediaCommand::Seek(Seek::Start)),
        // US Shift+0 types ")"; AZERTY types 0 with Shift.
        (text(")", true), MediaCommand::Seek(Seek::Start)),
        (text("0", true), MediaCommand::Seek(Seek::Start)),
        (key(Key::End, none), MediaCommand::Seek(Seek::End)),
        (text("G", true), MediaCommand::Seek(Seek::End)),
        (text("+", true), MediaCommand::VolumeUp),
        (text("+", false), MediaCommand::VolumeUp),
        (text("=", false), MediaCommand::VolumeUp),
        (text("-", false), MediaCommand::VolumeDown),
        (text(" ", false), MediaCommand::PlayPause),
        (key(Key::Up, none), MediaCommand::VolumeUp),
        (key(Key::Down, none), MediaCommand::VolumeDown),
        (key(Key::Enter, none), MediaCommand::HandOff),
    ];
    for (e, want) in cases {
        assert_eq!(focused(&k, &e), Some(want), "{e:?}");
    }
}

#[test]
fn caps_lock_g_without_shift_does_not_seek() {
    // Caps Lock: plain g types "G" with no Shift held.
    let k = MediaKeys::default_map();
    assert_eq!(focused(&k, &text("G", false)), None);
    assert_eq!(focused(&k, &text("g", false)), None);
}

#[test]
fn paging_jump_and_volume_keys_have_no_ctrl_alt_form() {
    let k = MediaKeys::default_map();
    for name in [Key::PageUp, Key::PageDown, Key::Home, Key::End] {
        let e = key(name.clone(), CTRL_ALT);
        assert_eq!(focused(&k, &e), None, "{name:?}");
        assert_eq!(window(&k, &e), None, "{name:?}");
    }
    for t in ["+", "-", "0"] {
        assert_eq!(window(&k, &key(Key::Text(t.into()), CTRL_ALT)), None, "{t}");
    }
}

#[test]
fn the_window_keys_are_ctrl_alt() {
    let k = MediaKeys::default_map();
    let shift_ca = Mods {
        shift: true,
        ..CTRL_ALT
    };
    let cases = [
        (
            key(Key::Text(" ".into()), CTRL_ALT),
            MediaCommand::PlayPause,
        ),
        (
            key(Key::Left, CTRL_ALT),
            MediaCommand::Seek(Seek::Back(secs(5))),
        ),
        (
            key(Key::Right, CTRL_ALT),
            MediaCommand::Seek(Seek::Forward(secs(5))),
        ),
        (key(Key::Up, CTRL_ALT), MediaCommand::VolumeUp),
        (key(Key::Down, CTRL_ALT), MediaCommand::VolumeDown),
        (key(Key::Text("m".into()), CTRL_ALT), MediaCommand::Mute),
        (key(Key::Text("<".into()), shift_ca), MediaCommand::Previous),
        (key(Key::Text(">".into()), shift_ca), MediaCommand::Next),
    ];
    for (e, want) in cases {
        assert_eq!(window(&k, &e), Some(want), "{e:?}");
    }
    assert_eq!(window(&k, &text(">", true)), None, "plain > is 10xer's");
    assert_eq!(focused(&k, &text("m", false)), None, "plain m is 10xer's");
}

#[test]
fn tenxer_adds_plain_mute_and_steps() {
    let k = MediaKeys::tenxer();
    assert_eq!(focused(&k, &text("m", false)), Some(MediaCommand::Mute));
    assert_eq!(focused(&k, &text("<", true)), Some(MediaCommand::Previous));
    assert_eq!(focused(&k, &text(">", true)), Some(MediaCommand::Next));
    assert_eq!(window(&k, &text(">", true)), Some(MediaCommand::Next));
    assert_eq!(
        window(&k, &key(Key::Text(" ".into()), CTRL_ALT)),
        Some(MediaCommand::PlayPause)
    );
}

#[test]
fn an_added_key_seeks_to_the_end_and_removing_plus_leaves_equals() {
    let mut k = MediaKeys::default_map();
    k.bind(
        KeyScope::Focused,
        Chord::new(ChordKey::Function(9)),
        MediaCommand::Seek(Seek::End),
    );
    k.unbind_chord(KeyScope::Focused, Chord::new(ChordKey::Char('+')));
    let none = Mods::default();
    assert_eq!(
        focused(&k, &key(Key::Function(9), none)),
        Some(MediaCommand::Seek(Seek::End))
    );
    assert_eq!(
        focused(&k, &text("G", true)),
        Some(MediaCommand::Seek(Seek::End))
    );
    assert_eq!(focused(&k, &text("=", false)), Some(MediaCommand::VolumeUp));
    assert_eq!(focused(&k, &text("+", false)), None);
}

#[test]
fn tooltips_name_the_first_window_chord() {
    let k = MediaKeys::default_map();
    assert_eq!(
        k.label(KeyScope::Window, MediaCommand::PlayPause)
            .as_deref(),
        Some("Ctrl+Alt+Space")
    );
    assert_eq!(
        k.label(KeyScope::Window, MediaCommand::Mute).as_deref(),
        Some("Ctrl+Alt+M")
    );
    assert_eq!(
        k.label(KeyScope::Window, MediaCommand::Next).as_deref(),
        Some("Ctrl+Alt+>")
    );
}

// Badges.

#[test]
fn badges_follow_stratas_rules() {
    let d = VideoDetails {
        dimensions: Some((1080, 1920)),
        color_transfer: Some("arib-std-b67".into()),
        pixel_format: Some("yuv420p12le".into()),
        frame_rate: Some(59.94),
        video_codec: Some("mpeg2video".into()),
        audio_codec: Some("pcm_s16le".into()),
        channels: Some(6),
        channel_layout: Some("5.1(side)".into()),
        subtitle_tracks: 1,
        sidecar_captions: 0,
    };
    let labels: Vec<String> = badges(&d).into_iter().map(|b| b.label).collect();
    assert_eq!(
        labels,
        [
            "1080p", "HLG", "12-bit", "60 fps", "MPEG-2", "PCM 5.1", "CC"
        ]
    );
    let more = VideoDetails {
        dimensions: Some((7680, 4320)),
        channels: Some(3),
        subtitle_tracks: 2,
        sidecar_captions: 1,
        ..VideoDetails::default()
    };
    let b = badges(&more);
    assert_eq!(b[0].label, "8K");
    assert_eq!(b[1].kind, BadgeKind::Audio);
    assert_eq!(b[1].label, "3 ch");
    assert_eq!(b[2].label, "CC ×3");
    assert!(badges(&VideoDetails::default()).is_empty());
}

// Storyboard.

#[test]
fn the_nearest_loaded_cell_stands_in_earlier_first() {
    let sheet = Sheet {
        cell: [128, 72],
        count: 10,
        duration: secs(100),
    };
    assert_eq!(sheet.cell_at(secs(0)), 0);
    assert_eq!(sheet.cell_at(secs(55)), 5);
    assert_eq!(sheet.cell_at(secs(500)), 9);
    assert_eq!(sheet.cell_time(0), secs(5));
    let mut b = Storyboard::new(sheet);
    assert!(b.nearest(secs(55)).is_none());
    assert!(b.set_cell(3, rune::Pixels::new()));
    assert!(b.set_cell(7, rune::Pixels::new()));
    assert!(!b.set_cell(7, rune::Pixels::new()), "a cell lands once");
    assert!(!b.set_cell(10, rune::Pixels::new()));
    assert_eq!(
        b.nearest(secs(55)).map(|(i, _)| i),
        Some(3),
        "a tie goes earlier"
    );
    assert_eq!(b.nearest(secs(65)).map(|(i, _)| i), Some(7));
    assert_eq!(b.loaded(), 2);
}

// The waveform and the spectrum.

#[test]
fn levels_grow_in_and_land_at_once_with_motion_off() {
    let mut l = Levels::default();
    l.set(8, 0, &[0.5; 8]);
    assert!(l.moving());
    assert!(l.step(1.0 / 60.0, true));
    let first = l.bars(8)[0];
    assert!(first > 0.0 && first < 1.0, "{first}");
    let mut steps = 1;
    while l.moving() {
        l.step(1.0 / 60.0, true);
        steps += 1;
    }
    assert!(
        (30..90).contains(&steps),
        "settles in about a second: {steps} frames"
    );
    let mut instant = Levels::default();
    instant.set(8, 0, &[0.5; 8]);
    instant.step(1.0 / 60.0, false);
    assert!(!instant.moving());
}

#[test]
fn each_bar_takes_the_loudest_of_its_buckets() {
    let mut l = Levels::default();
    l.set(4, 0, &[0.1, 0.4, 0.2, 0.0]);
    l.step(0.0, false);
    assert_eq!(l.bars(2), vec![1.0, 0.5]);
    assert_eq!(l.bars(4)[3], 0.0);
}

#[test]
fn peak_caps_hold_then_fall_while_bars_fall_at_once_to_lower_targets() {
    let mut b = Ballistics::default();
    b.resize(1);
    b.update(Some(&[0.8]), 0.016);
    b.update(Some(&[0.1]), 0.1);
    assert!(
        (b.bars()[0] - 0.46).abs() < 1e-4,
        "falls 3.4 a second: {}",
        b.bars()[0]
    );
    assert_eq!(b.peaks().next(), Some(0.8), "the cap holds");
    for _ in 0..3 {
        b.update(Some(&[0.1]), 0.1);
    }
    assert_eq!(b.peaks().next(), Some(0.8), "still within 0.42 s");
    b.update(Some(&[0.1]), 0.1);
    b.update(Some(&[0.1]), 0.1);
    assert!(b.peaks().next().unwrap() < 0.8, "then falls");
    for _ in 0..40 {
        b.update(None, 0.1);
    }
    assert!(b.is_settled());
}

#[test]
fn the_spectrum_holds_12_to_96_bars() {
    assert_eq!(bar_count(10.0, 5.0, 2.0), 12);
    assert_eq!(bar_count(348.0, 5.0, 2.0), 50);
    assert_eq!(bar_count(2000.0, 5.0, 2.0), 96);
}

// The audio layout.

#[test]
fn wide_panes_put_art_beside_with_hysteresis() {
    let (fixed, gap) = (200.0, 16.0);
    assert!(plan(600.0, 500.0, fixed, gap, false).wide, "1.2 enters");
    assert!(!plan(540.0, 500.0, fixed, gap, false).wide, "1.08 doesn't");
    assert!(plan(500.0, 500.0, fixed, gap, true).wide, "1.0 stays");
    assert!(!plan(470.0, 500.0, fixed, gap, true).wide, "0.94 leaves");
}

#[test]
fn a_short_stacked_pane_drops_the_spectrum_then_the_art() {
    let (fixed, gap) = (200.0, 16.0);
    let tall = plan(400.0, 700.0, fixed, gap, false);
    assert!(tall.art.is_some() && tall.spectrum.is_some());
    let short = plan(400.0, 360.0, fixed, gap, false);
    assert!(short.art.is_some() && short.spectrum.is_none(), "{short:?}");
    let tiny = plan(300.0, 200.0 + ART_MIN, fixed, gap, false);
    assert!(tiny.art.is_none() && tiny.spectrum.is_none(), "{tiny:?}");
}

// The fake clock.

#[test]
fn the_fake_clock_plays_seeks_and_ends() {
    let mut c = FakeClock::new(Some(secs(10))).latency(0.0, 0.2);
    c.advance(1.0);
    c.apply(&MediaIntent::Play);
    c.advance(3.0);
    assert_eq!(c.position(), secs(2));
    c.apply(&MediaIntent::Seek(secs(8)));
    assert_eq!(c.state(), PlaybackState::Seeking);
    assert_eq!(c.position(), secs(8), "a seek reports its target");
    assert_eq!(c.next_due(), Some(3.2));
    c.advance(3.2);
    assert_eq!(c.state(), PlaybackState::Playing, "and resumes");
    c.advance(6.0);
    assert_eq!(c.state(), PlaybackState::Ended);
    assert_eq!(c.position(), secs(10));
    c.apply(&MediaIntent::Play);
    assert_eq!(c.position(), Duration::ZERO, "play at the end starts over");
    let mut unknown = FakeClock::new(None);
    unknown.apply(&MediaIntent::Seek(secs(3)));
    assert_eq!(unknown.position(), Duration::ZERO, "no duration, no seek");
}

// The state's commands.

fn playing(kind: MediaKind) -> (MediaState, FakeClock) {
    let mut st = MediaState::new(Clip {
        kind,
        name: "clip.mp4".into(),
        position: None,
        has_previous: false,
        has_next: true,
    });
    st.set_volume(0.3, false);
    let mut c = FakeClock::new(Some(secs(100)));
    c.set_position(secs(40));
    c.play_now();
    sync(&mut st, &c);
    (st, c)
}

/// Reads the clock as a tick would, without a `Tx`.
fn sync(st: &mut MediaState, c: &FakeClock) {
    let mut rt = rune::Runtime::new(
        (),
        |_: &(), _| rune::el(),
        (10.0, 10.0),
        rune::Env::new(false, 1.0),
    );
    rt.update(|_, tx| {
        st.tick(c, 0.0, tx);
    });
}

#[test]
fn volume_keys_step_the_grid_and_mute_at_zero_and_unmute_upward() {
    let (mut st, _) = playing(MediaKind::Audio);
    for _ in 0..3 {
        st.command(MediaCommand::VolumeDown);
    }
    assert!(st.muted());
    assert!(
        (st.volume() - 0.1).abs() < 1e-6,
        "keeps the volume to restore"
    );
    st.command(MediaCommand::VolumeUp);
    assert!(!st.muted());
    assert!((st.volume() - 0.1).abs() < 1e-6);
    let intents = st.drain_intents();
    assert_eq!(intents.len(), 4);
    assert!(
        matches!(intents[2], MediaIntent::SetVolume { muted: true, volume } if (volume - 0.1).abs() < 1e-6)
    );
    st.set_volume(0.7, true);
    st.command(MediaCommand::VolumeUp);
    assert!(
        (st.volume() - 0.1).abs() < 1e-6,
        "a muted volume steps from 0"
    );
}

#[test]
fn seeks_clamp_to_the_start_and_the_last_frame() {
    let (mut st, _) = playing(MediaKind::Video);
    st.command(MediaCommand::Seek(Seek::Back(secs(60))));
    st.command(MediaCommand::Seek(Seek::Forward(secs(600))));
    st.command(MediaCommand::Seek(Seek::End));
    assert_eq!(
        st.drain_intents(),
        vec![
            MediaIntent::Seek(Duration::ZERO),
            MediaIntent::Seek(last_frame(secs(100))),
            MediaIntent::Seek(last_frame(secs(100))),
        ]
    );
}

#[test]
fn a_clip_without_a_duration_takes_no_seek_nor_drag() {
    let mut st = MediaState::default();
    st.command(MediaCommand::Seek(Seek::Forward(secs(5))));
    assert!(!st.drag_at(Some(10.0), false));
    assert!(st.drain_intents().is_empty());
}

#[test]
fn hand_off_pauses_a_playing_video_then_asks_for_the_player() {
    let (mut st, _) = playing(MediaKind::Video);
    assert!(st.command(MediaCommand::HandOff));
    assert_eq!(
        st.drain_intents(),
        vec![
            MediaIntent::Pause,
            MediaIntent::HandOff { at: Some(secs(40)) }
        ]
    );
    let (mut audio, _) = playing(MediaKind::Audio);
    assert!(
        !audio.command(MediaCommand::HandOff),
        "Return goes on over audio"
    );
    assert!(audio.drain_intents().is_empty());
}

#[test]
fn steps_follow_the_listings_previous_and_next() {
    let (mut st, _) = playing(MediaKind::Audio);
    st.command(MediaCommand::Previous);
    st.command(MediaCommand::Next);
    assert_eq!(st.drain_intents(), vec![MediaIntent::Step(1)]);
}

#[test]
fn the_wheel_seeks_a_step_a_notch_and_ignores_jumps_under_a_tick() {
    let (mut st, _) = playing(MediaKind::Plain);
    assert!(st.wheel(40.0, false));
    assert_eq!(st.drain_intents(), vec![MediaIntent::Seek(secs(45))]);
    assert!(!st.wheel(0.1, true), "0.0125 s is under a tick");
    assert!(st.drain_intents().is_empty());
    // The small move carries over into the next: a step back less the 0.0025 kept.
    assert!(st.wheel(-40.0, true));
    match st.drain_intents()[..] {
        [MediaIntent::Seek(t)] => {
            assert!((t.as_secs_f64() - 40.0125).abs() < 1e-3, "{t:?}")
        }
        ref other => panic!("{other:?}"),
    }
}

#[test]
fn an_error_turns_play_off() {
    let (mut st, _) = playing(MediaKind::Video);
    st.set_error(Some(MediaError {
        title: "No".into(),
        detail: "No.".into(),
    }));
    st.toggle();
    assert!(st.drain_intents().is_empty());
}
