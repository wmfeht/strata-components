//! The zoom model as plain math: levels, steps, limits, the wheel and pinch factors, zooming
//! about a point, pan clamps, the zoom report and the zoom keys.
//!
//! [`Viewport`](super::Viewport) runs on these, and C12's page column reuses them with its own
//! scroll view: a virtual column can't sit under the viewport's live pan, because Rune mounts
//! virtual rows from the scroll offset. C12 picks its own basis (a level of 1 is the fitted
//! width there, limited to 1–4) and its own wheel speed, and keeps the rest:
//!
//! ```ignore
//! let limits = ZoomLimits::new(1.0, 4.0);
//! // Strata's PDF zoom: e^0.14 a notch, and a notch is 40 points.
//! let next = limits.clamp(s.pdf.level * wheel_factor_per(e.dy, 0.14 / 40.0));
//! if let Some(ZoomKey::Reset) = ctrl_zero_key(&key) { /* back to fitted width, consumed */ }
//! ```
//!
//! # Units
//!
//! A *level* is device pixels per content pixel: 1 is 1:1, where each content pixel covers one
//! device pixel, whatever the display scale. Content sizes are in content pixels (an image's
//! own size, which isn't the size of the `Pixels` that draws it). Frame sizes, pans and points
//! are in layout points.

use rune::{Key, KeyEvent};
use strata_kit::look::Ladder;

/// Points of Ctrl+wheel per unit of magnification: Rune's own, where a trackpad pinch arrives
/// as Ctrl+wheel (Linux, Windows and the web), so a Ctrl+wheel the viewport hears itself and
/// one Rune turned into a pinch zoom alike.
pub const PINCH_WHEEL: f32 = 200.0;

/// The zoom factor for a Ctrl+wheel of `dy` points: `e^(-dy / 200)`. One 40-point mouse notch
/// is about 1.22×, and a notch back returns to the same level.
pub fn wheel_factor(dy: f32) -> f32 {
    wheel_factor_per(dy, 1.0 / PINCH_WHEEL)
}

/// The zoom factor for a Ctrl+wheel of `dy` points at `per_point` (natural log of the factor
/// per point): `e^(-dy × per_point)`.
pub fn wheel_factor_per(dy: f32, per_point: f32) -> f32 {
    (-dy * per_point).exp()
}

/// The zoom factor for a pinch of `delta` (Rune's `MagnifyEvent::delta`): `e^delta`, which
/// matches [`wheel_factor`] for a pinch Rune made from Ctrl+wheel and is symmetric, so pinching
/// out and back lands where it started.
pub fn magnify_factor(delta: f32) -> f32 {
    delta.exp()
}

/// Strata's zoom steps for `+` and `-`: from 10 % to 3200 %, with 100 % (1:1) the default.
pub fn default_steps() -> Ladder {
    let steps = vec![
        0.1,
        0.25,
        1.0 / 3.0,
        0.5,
        2.0 / 3.0,
        1.0,
        1.5,
        2.0,
        3.0,
        4.0,
        6.0,
        8.0,
        12.0,
        16.0,
        24.0,
        32.0,
    ];
    Ladder::new(steps, 1.0).unwrap_or_else(|| unreachable!("the default steps increase"))
}

/// The range a level is held in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomLimits {
    pub min: f32,
    pub max: f32,
}

impl ZoomLimits {
    /// Limits from `min` to `max`, swapped if given the wrong way round.
    pub fn new(min: f32, max: f32) -> ZoomLimits {
        ZoomLimits {
            min: min.min(max),
            max: max.max(min),
        }
    }

    /// The default steps' range: 10 % to 3200 %.
    pub fn of(steps: &Ladder) -> ZoomLimits {
        ZoomLimits::new(steps.min(), steps.max())
    }

    pub fn clamp(&self, level: f32) -> f32 {
        level.clamp(self.min, self.max)
    }

    /// These limits, with `min` lowered to `floor` when that is smaller: a viewport always lets
    /// the user zoom out to fit a content larger than the limits allow.
    pub fn lowered_to(&self, floor: f32) -> ZoomLimits {
        ZoomLimits::new(self.min.min(floor), self.max)
    }
}

impl Default for ZoomLimits {
    fn default() -> ZoomLimits {
        ZoomLimits::of(&default_steps())
    }
}

/// How the content sits in the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The whole content, as large as fits, letterboxed.
    #[default]
    Fit,
    /// As [`Mode::Fit`], but never larger than 1:1 (G-code thumbnails).
    ScaleDown,
    /// Covers the frame; the overflow is cropped and can be panned.
    Fill,
    /// 1:1: one content pixel per device pixel.
    Actual,
    /// A level the user zoomed to.
    Free,
}

impl Mode {
    /// Whether the level follows the frame's size.
    pub fn follows_frame(self) -> bool {
        matches!(self, Mode::Fit | Mode::ScaleDown | Mode::Fill)
    }
}

/// The level `mode` shows `content` at in a `frame` at display `scale`, or `free` for
/// [`Mode::Free`]. A zero-sized frame or content gives 1.
pub fn mode_level(mode: Mode, frame: [f32; 2], content: [f32; 2], scale: f32, free: f32) -> f32 {
    let ratio = |pick: fn(f32, f32) -> f32| {
        if content[0] <= 0.0 || content[1] <= 0.0 || frame[0] <= 0.0 || frame[1] <= 0.0 {
            1.0
        } else {
            pick(frame[0] / content[0], frame[1] / content[1]) * scale
        }
    };
    match mode {
        Mode::Fit => ratio(f32::min),
        Mode::ScaleDown => ratio(f32::min).min(1.0),
        Mode::Fill => ratio(f32::max),
        Mode::Actual => 1.0,
        Mode::Free => free,
    }
}

/// The size `content` draws at, in points, at `level` and display `scale`.
pub fn drawn_size(content: [f32; 2], level: f32, scale: f32) -> [f32; 2] {
    let k = level / scale.max(f32::EPSILON);
    [content[0] * k, content[1] * k]
}

/// `pan` held so the drawn content never leaves a gap it could fill: on each axis the content
/// overflowing the frame pans at most to its edges, and a content that fits is centred (pan 0).
/// Pans are offsets from centred, in points.
pub fn clamp_pan(pan: [f32; 2], drawn: [f32; 2], frame: [f32; 2]) -> [f32; 2] {
    let axis = |p: f32, d: f32, f: f32| {
        let reach = ((d - f) / 2.0).max(0.0);
        p.clamp(-reach, reach)
    };
    [
        axis(pan[0], drawn[0], frame[0]),
        axis(pan[1], drawn[1], frame[1]),
    ]
}

/// Whether the drawn content overflows the frame, per axis (beyond a hundredth of a point).
pub fn overflows(drawn: [f32; 2], frame: [f32; 2]) -> [bool; 2] {
    [drawn[0] > frame[0] + 0.01, drawn[1] > frame[1] + 0.01]
}

/// The pan that keeps the content point under `about` there when the level changes by `k`.
/// `about` and the pan are measured from the frame's centre. Clamp the result.
pub fn zoom_about(pan: [f32; 2], about: [f32; 2], k: f32) -> [f32; 2] {
    [
        about[0] - (about[0] - pan[0]) * k,
        about[1] - (about[1] - pan[1]) * k,
    ]
}

/// What changed when the level did: where the content now draws, so a host can render it
/// again at the new scale (C12's pages, an SVG rasterised for its size, a downsampled photo).
///
/// The viewport sends one on every change of level: Ctrl+wheel, pinch, the keys, a mode change,
/// and a resize in a mode that follows the frame. Debounce in the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomReport {
    /// Device pixels per content pixel.
    pub level: f32,
    pub mode: Mode,
    /// The content's size as drawn, in device pixels.
    pub drawn_px: [f32; 2],
    /// The content's own size, in content pixels.
    pub content: [f32; 2],
}

impl ZoomReport {
    /// A report for `content` at `level` in `mode`.
    pub fn new(level: f32, mode: Mode, content: [f32; 2]) -> ZoomReport {
        ZoomReport {
            level,
            mode,
            drawn_px: [content[0] * level, content[1] * level],
            content,
        }
    }

    /// The pixel size worth rendering the content at: its drawn size, never more than its own,
    /// and scaled down (keeping its shape) so neither side passes `cap`. Rune draws no image
    /// over 8192 pixels on a side, so pass at most that.
    pub fn wanted_px(&self, cap: u32) -> [u32; 2] {
        let w = self.drawn_px[0].min(self.content[0]).max(0.0);
        let h = self.drawn_px[1].min(self.content[1]).max(0.0);
        let cap = cap.max(1) as f32;
        let k = (cap / w.max(h).max(f32::EPSILON)).min(1.0);
        [
            ((w * k).ceil() as u32).min(cap as u32),
            ((h * k).ceil() as u32).min(cap as u32),
        ]
    }
}

/// What a key asks of the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZoomKey {
    /// One step up the zoom steps.
    In,
    /// One step down.
    Out,
    /// Back to the default mode. Consumed even when already there, so a reset meant for the
    /// content never reaches the app's own reset (Strata's text size).
    Reset,
    /// A pan by one arrow step along (x, y), each -1, 0 or 1. Taken only on an axis that
    /// overflows; otherwise it passes on (media seek, archive navigation).
    Pan(i8, i8),
}

/// The viewport's default keys: `+` or `=` zooms in, `-` zooms out, `0` (and keypad 0) resets,
/// as does Ctrl+0 ([`ctrl_zero_key`]), and the arrows pan. Ctrl++ and Ctrl+− are left alone:
/// they are Strata's interface text size.
///
/// Open item: Strata's preview and 10xer keymaps haven't been checked for plain `+`, `-` and
/// `0`. A host whose keymap takes them passes its own set to `Viewport::zoom_keys`.
pub fn zoom_key(key: &KeyEvent) -> Option<ZoomKey> {
    if let Some(reset) = ctrl_zero_key(key) {
        return Some(reset);
    }
    let m = key.mods;
    if m.cmd || m.ctrl || m.alt || m.logo {
        return None;
    }
    match &key.key {
        Key::Text(t) => match t.as_str() {
            "+" | "=" => Some(ZoomKey::In),
            "-" => Some(ZoomKey::Out),
            // Shift+0 types ")" on most layouts; a "0" with Shift is a keypad 0 at most.
            "0" => Some(ZoomKey::Reset),
            _ => None,
        },
        Key::Left if !m.shift => Some(ZoomKey::Pan(-1, 0)),
        Key::Right if !m.shift => Some(ZoomKey::Pan(1, 0)),
        Key::Up if !m.shift => Some(ZoomKey::Pan(0, -1)),
        Key::Down if !m.shift => Some(ZoomKey::Pan(0, 1)),
        _ => None,
    }
}

/// Ctrl+0, including keypad 0 with Num Lock on: Strata's PDF zoom reset
/// (`preview/preview-panel/pdf`). With Alt or Super also held it is not a reset, so
/// Ctrl+Alt+0 and Ctrl+Super+0 pass on. On macOS the primary modifier (⌘) counts as Ctrl.
pub fn ctrl_zero_key(key: &KeyEvent) -> Option<ZoomKey> {
    let m = key.mods;
    let zero = match &key.key {
        // Rune sends a character typed with the primary modifier as `Command`.
        Key::Command(c) => *c == '0',
        Key::Text(t) => t == "0",
        _ => false,
    };
    // Off macOS the primary modifier is Ctrl, which sets both; on macOS ⌘ sets `cmd` and
    // `logo`, and a literal Control sets `ctrl`. Super with Ctrl is `ctrl && logo`.
    let primary = m.cmd || m.ctrl;
    let super_with_ctrl = m.ctrl && m.logo;
    (zero && primary && !m.alt && !m.shift && !super_with_ctrl).then_some(ZoomKey::Reset)
}

/// No keys at all: for a host that routes every key itself.
pub fn no_keys(_key: &KeyEvent) -> Option<ZoomKey> {
    None
}
