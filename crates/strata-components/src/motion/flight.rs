//! The trash flight: what flies, from where to where, with Strata's timings, and the flyers'
//! elements on each side of the matched swap.

use std::cell::Cell;

use rune::{Beat, Easing, El, Icon, Sequence, Spring, el, icon};
use strata_kit::look::Look;

use super::model::{EffectId, Event, Pulses};
use super::pulse::PulseKind;

/// Travel into the target (Strata's `TRAVEL`).
pub const TRAVEL: f64 = 0.36;
/// Travel out of the target onto restored rows (Strata's `RESTORE_TRAVEL`).
pub const RESTORE_TRAVEL: f64 = 0.24;
/// Travel up off rows restored in the target's own view (Strata's `RELEASE_TRAVEL`).
pub const RELEASE_TRAVEL: f64 = 0.22;
/// Each flyer after the first lands this much later (Strata's `STAGGER_MS`)…
pub const STAGGER: f64 = 0.012;
/// …up to this much (Strata's `MAX_STAGGER_MS`).
pub const MAX_STAGGER: f64 = 0.036;
/// At most this many icons fly, picked evenly from the legs (Strata's `MAX_FLYERS`).
pub const MAX_FLYERS: usize = 7;

/// Which way a flight goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// From rows into the target, which then pulses (Move to Trash).
    Inbound,
    /// From the target onto rows, which pulses as they leave (a restore into a folder).
    Outbound,
    /// Up off rows, while the target shakes (a restore while viewing the target).
    Release,
}

impl Mode {
    pub(super) fn travel(self) -> f64 {
        match self {
            Mode::Inbound => TRAVEL,
            Mode::Outbound => RESTORE_TRAVEL,
            Mode::Release => RELEASE_TRAVEL,
        }
    }

    /// The target's pulse, and whether it starts at landing (else at launch).
    pub(super) fn pulse(self) -> (PulseKind, bool) {
        match self {
            Mode::Inbound => (PulseKind::Impact, true),
            Mode::Outbound => (PulseKind::Release, false),
            Mode::Release => (PulseKind::Shake, false),
        }
    }

    fn beats(self) -> &'static [Beat] {
        match self {
            Mode::Inbound => INBOUND,
            Mode::Outbound => OUTBOUND,
            Mode::Release => RELEASE,
        }
    }
}

// Strata draws each flyer per frame: opacity `1 − p³` into the target and shrinking to 0.42
// from p = 0.58; out of it fading in (`1 − (1 − p)³`) and growing from 0.42 at p = 0.08; up
// off rows banked at 1.25, fading from p = 0.65 and spreading to 1.35 from p = 0.82.
const INBOUND: &[Beat] = &[
    Beat::new(0.0),
    Beat::new(0.209).opacity(0.8),
    Beat::new(0.30)
        .opacity(0.42)
        .scale(0.42)
        .easing(Easing::EASE_OUT),
    Beat::new(TRAVEL)
        .opacity(0.0)
        .scale(0.42)
        .easing(Easing::EASE_IN),
];
const OUTBOUND: &[Beat] = &[
    Beat::new(0.0).opacity(0.0).scale(0.42),
    Beat::new(0.06)
        .opacity(0.58)
        .scale(0.8)
        .easing(Easing::EASE_OUT),
    Beat::new(0.12).opacity(0.875),
    Beat::new(RESTORE_TRAVEL),
];
const RELEASE: &[Beat] = &[
    Beat::new(0.0).scale(0.85),
    Beat::new(0.035).scale(1.25).easing(Easing::EASE_OUT),
    Beat::new(0.143).scale(1.25),
    Beat::new(0.18).opacity(0.6).scale(1.35),
    Beat::new(RELEASE_TRAVEL)
        .opacity(0.0)
        .scale(1.35)
        .easing(Easing::EASE_IN),
];

/// A flight to start with [`Effects::fly`](super::Effects::fly).
///
/// Each leg names a row side by key (a key, or a key path such as C1e's
/// `part_path(scope, id, ICON)`), with the icon that flies. A key whose rect is wider than tall
/// is taken as a row: the flyer meets it at its leading square, where a row's icon sits.
#[derive(Clone, Debug)]
pub struct Flight {
    pub(super) mode: Mode,
    pub(super) target: String,
    pub(super) target_in: Option<String>,
    pub(super) rows_in: Option<String>,
    pub(super) legs: Vec<(String, &'static Icon)>,
}

impl Flight {
    fn new(
        mode: Mode,
        target: impl Into<String>,
        legs: impl IntoIterator<Item = (String, &'static Icon)>,
    ) -> Self {
        Self {
            mode,
            target: target.into(),
            target_in: None,
            rows_in: None,
            legs: legs.into_iter().collect(),
        }
    }

    /// Icons fly from `sources` into `target` (C8's `trash_target_key`), which then pulses.
    pub fn into_target(
        target: impl Into<String>,
        sources: impl IntoIterator<Item = (String, &'static Icon)>,
    ) -> Self {
        Self::new(Mode::Inbound, target, sources)
    }

    /// Icons fly out of `target` onto the rows `destinations` name, which pulses as they leave.
    /// The rows must be mounted when the flight starts: keep them hidden
    /// (`RowMarks::vanished`) until [`Event::Landed`].
    pub fn out_of_target(
        target: impl Into<String>,
        destinations: impl IntoIterator<Item = (String, &'static Icon)>,
    ) -> Self {
        Self::new(Mode::Outbound, target, destinations)
    }

    /// Icons lift up off the rows `sources` name and fade, while `target` shakes.
    pub fn release(
        target: impl Into<String>,
        sources: impl IntoIterator<Item = (String, &'static Icon)>,
    ) -> Self {
        Self::new(Mode::Release, target, sources)
    }

    /// The scroll viewport the target shows in (C8's sidebar list): a target scrolled out of
    /// it flies nothing.
    pub fn target_in(mut self, viewport: impl Into<String>) -> Self {
        self.target_in = Some(viewport.into());
        self
    }

    /// The scroll viewport the rows show in (C1e's `viewport_path`): rows outside it fly
    /// nothing, as rows outside the window never do.
    pub fn rows_in(mut self, viewport: impl Into<String>) -> Self {
        self.rows_in = Some(viewport.into());
        self
    }
}

/// One leg of a flight that is playing.
pub(super) struct LegRun {
    pub row: String,
    pub icon: &'static Icon,
    /// The flyer's rect as the launch side drew it: set by the view while the flight mounts
    /// (a view reads layout and the tick doesn't), `None` for a leg that couldn't launch.
    pub seen: Cell<Option<[f32; 4]>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    /// The launch side is mounted; the flight flips at the second tick, a frame after its
    /// launch side has surely been laid out and drawn.
    Mount { ticks: u8 },
    /// Flying since `t0`: the landing side is mounted until it lands (at the tick in
    /// `landed`), then the target's pulse plays out.
    Fly { t0: f64, landed: Option<f64> },
}

pub(super) struct FlightRun {
    pub id: EffectId,
    pub mode: Mode,
    pub target: String,
    pub target_in: Option<String>,
    pub rows_in: Option<String>,
    pub legs: Vec<LegRun>,
    pub phase: Phase,
    /// The legs that launched, in order: a flyer's place here is its stagger.
    pub flying: Vec<usize>,
}

impl FlightRun {
    pub fn new(id: EffectId, f: Flight) -> Self {
        let count = f.legs.len().min(MAX_FLYERS);
        let picked = (0..count).map(|i| i * f.legs.len() / count);
        let legs = picked
            .map(|i| LegRun {
                row: f.legs[i].0.clone(),
                icon: f.legs[i].1,
                seen: Cell::new(None),
            })
            .collect();
        Self {
            id,
            mode: f.mode,
            target: f.target,
            target_in: f.target_in,
            rows_in: f.rows_in,
            legs,
            phase: Phase::Mount { ticks: 0 },
            flying: Vec::new(),
        }
    }

    fn land_at(&self, t0: f64) -> f64 {
        t0 + self.mode.travel() + stagger(self.flying.len().saturating_sub(1))
    }

    fn done_at(&self, t0: f64, landed_at: f64) -> f64 {
        let (kind, at_landing) = self.mode.pulse();
        let from = if at_landing { landed_at } else { t0 };
        landed_at.max(from + kind.duration())
    }

    /// One tick at `now`: pushes this flight's events, lowers `next` to its next deadline,
    /// and says whether it plays on.
    pub fn step(
        &mut self,
        now: f64,
        events: &mut Vec<Event>,
        next: &mut Option<f64>,
        pulses: &mut Pulses,
    ) -> bool {
        const EPS: f64 = 1e-6;
        let due = |next: &mut Option<f64>, at: f64| *next = Some(next.map_or(at, |n| n.min(at)));
        match self.phase {
            Phase::Mount { ticks: 0 } => {
                self.phase = Phase::Mount { ticks: 1 };
                due(next, now);
                true
            }
            Phase::Mount { .. } => {
                events.push(Event::Launched(self.id));
                self.flying = (0..self.legs.len())
                    .filter(|&i| self.legs[i].seen.get().is_some())
                    .collect();
                if self.flying.is_empty() {
                    events.push(Event::Done(self.id));
                    return false;
                }
                self.phase = Phase::Fly {
                    t0: now,
                    landed: None,
                };
                let (kind, at_landing) = self.mode.pulse();
                if !at_landing {
                    pulses.start(&self.target, kind);
                }
                due(next, self.land_at(now));
                true
            }
            Phase::Fly { t0, landed } => {
                let land = self.land_at(t0);
                let landed = match landed {
                    Some(at) => at,
                    None if now + EPS >= land => {
                        events.push(Event::Landed(self.id));
                        let (kind, at_landing) = self.mode.pulse();
                        if at_landing {
                            pulses.start(&self.target, kind);
                        }
                        self.phase = Phase::Fly {
                            t0,
                            landed: Some(now),
                        };
                        now
                    }
                    None => {
                        due(next, land);
                        return true;
                    }
                };
                let done = self.done_at(t0, landed);
                if now + EPS >= done {
                    events.push(Event::Done(self.id));
                    return false;
                }
                due(next, done);
                true
            }
        }
    }
}

/// The landing delay of the flyer at place `k`.
pub(super) fn stagger(k: usize) -> f64 {
    (k as f64 * STAGGER).min(MAX_STAGGER)
}

/// The spring flyer `k` travels on: longer by its stagger, so the icons land in turn.
pub(super) fn spring(mode: Mode, k: usize) -> Spring {
    Spring::new((mode.travel() + stagger(k)) as f32, 0.0)
}

/// Where a flyer of size `size` meets `rect`: its middle, or for a row (wider than tall) its
/// leading square.
pub(super) fn meet(rect: [f32; 4], size: [f32; 2]) -> [f32; 4] {
    let [x, y, w, h] = rect;
    let (cx, cy) = if w > h * 1.5 {
        (x + h / 2.0, y + h / 2.0)
    } else {
        (x + w / 2.0, y + h / 2.0)
    };
    [cx - size[0] / 2.0, cy - size[1] / 2.0, size[0], size[1]]
}

/// Where release flyer `k` of `n` heads from `from`: up and spread (Strata's `release_end`).
pub(super) fn release_end(from: [f32; 4], k: usize, n: usize) -> [f32; 4] {
    let drift = (k as f32 - n.saturating_sub(1) as f32 / 2.0) * 44.0;
    let centre_y = from[1] + from[3] / 2.0;
    let rise = (centre_y - 28.0).clamp(90.0, 240.0);
    [from[0] + drift, from[1] - rise, from[2], from[3]]
}

/// The matched id both sides of flyer `i` share.
pub(super) fn matched_id(id: EffectId, i: usize) -> String {
    format!("c23-fly-{}-{i}", id.0)
}

/// The launch side's key.
pub fn launch_key(id: EffectId, i: usize) -> String {
    format!("c23-launch-{}-{i}", id.0)
}

/// The landing side's key: what flies.
pub fn flyer_key(id: EffectId, i: usize) -> String {
    format!("c23-flyer-{}-{i}", id.0)
}

fn glyph(look: &Look, i: &'static Icon, rect: [f32; 4]) -> El {
    el().w_full()
        .h_full()
        .center()
        .child(icon(i, look.icon_size.md.min(rect[2]).min(rect[3])).color(look.kit.c.icon_base))
}

/// The launch side of leg `i`: the icon, drawn where it starts.
pub(super) fn launch(id: EffectId, i: usize, look: &Look, ic: &'static Icon, rect: [f32; 4]) -> El {
    el().key(launch_key(id, i))
        .matched(matched_id(id, i))
        .absolute(rect[0], rect[1])
        .size(rect[2], rect[3])
        .child(glyph(look, ic, rect))
}

/// The landing side of leg `i`, flyer `k` of the flight: the icon where it lands, which Rune
/// flies there from the launch side, fading and shrinking as Strata's do.
pub(super) fn flyer(
    id: EffectId,
    i: usize,
    k: usize,
    mode: Mode,
    look: &Look,
    ic: &'static Icon,
    rect: [f32; 4],
) -> El {
    el().key(flyer_key(id, i))
        .matched(matched_id(id, i))
        .motion(spring(mode, k))
        .absolute(rect[0], rect[1])
        .size(rect[2], rect[3])
        .child(glyph(look, ic, rect).sequence(Sequence::new(mode.beats()).delay(stagger(k))))
}
