//! The target's pulses (Strata's `trash-receiving`, `trash-impact`, `trash-release` and
//! `trash-rebel-shudder`): a scale on the target itself, through [`dress_target`], and an
//! accent ring over it in the motion layer.

use rune::{Beat, Easing, El, Sequence, el};
use strata_kit::look::Look;

use super::model::Effects;

/// How long the impact and release pulses play (Strata's `BOUNCE`).
pub const BOUNCE: f64 = 0.28;
/// How long the shake plays (Strata's `trash-rebel-shudder`).
pub const SHAKE: f64 = 0.24;
/// How long the receiving ring takes to light while icons fly in (`trash-receiving`).
pub const RECEIVING: f64 = 0.42;

/// What the target plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseKind {
    /// Icons landed in it: it swells and settles, and a ring spreads and fades.
    Impact,
    /// Icons left it: it dips and swells, and a softer ring spreads.
    Release,
    /// Icons lifted off rows in its own view: it shudders (in scale only: U-C23-2).
    Shake,
}

impl PulseKind {
    pub(super) fn duration(self) -> f64 {
        match self {
            PulseKind::Impact | PulseKind::Release => BOUNCE,
            PulseKind::Shake => SHAKE,
        }
    }

    fn target_beats(self) -> &'static [Beat] {
        match self {
            PulseKind::Impact => IMPACT,
            PulseKind::Release => RELEASE,
            PulseKind::Shake => SHUDDER,
        }
    }

    fn ring_beats(self) -> Option<&'static [Beat]> {
        match self {
            PulseKind::Impact => Some(IMPACT_RING),
            PulseKind::Release => Some(RELEASE_RING),
            PulseKind::Shake => None,
        }
    }
}

const IMPACT: &[Beat] = &[
    Beat::new(0.0).scale(0.96),
    Beat::new(0.095).scale(1.14),
    Beat::new(0.19).scale(0.97),
    Beat::new(BOUNCE),
];
const RELEASE: &[Beat] = &[
    Beat::new(0.0).scale(0.94),
    Beat::new(0.126).scale(1.08),
    Beat::new(BOUNCE),
];
// Strata's shudder also turns ±8°; a Beat has no rotation (U-C23-2).
const SHUDDER: &[Beat] = &[
    Beat::new(0.0),
    Beat::new(0.043).scale(0.92),
    Beat::new(0.091).scale(1.14),
    Beat::new(0.139).scale(0.95),
    Beat::new(0.187).scale(1.05),
    Beat::new(SHAKE),
];
const IMPACT_RING: &[Beat] = &[
    Beat::new(0.0),
    Beat::new(BOUNCE)
        .opacity(0.0)
        .scale(1.45)
        .easing(Easing::EASE_OUT),
];
const RELEASE_RING: &[Beat] = &[
    Beat::new(0.0).opacity(0.8),
    Beat::new(BOUNCE)
        .opacity(0.0)
        .scale(1.3)
        .easing(Easing::EASE_OUT),
];
const RECEIVING_RING: &[Beat] = &[
    Beat::new(0.0).opacity(0.0),
    Beat::new(0.23),
    Beat::new(RECEIVING).opacity(0.7),
];

/// How many pulses each target has started, and the last one's kind: a new count is a new
/// key, which replays a sequence.
#[derive(Debug, Default)]
pub(super) struct Pulses(Vec<(String, u32, PulseKind)>);

impl Pulses {
    pub fn start(&mut self, target: &str, kind: PulseKind) {
        match self.0.iter_mut().find(|(t, ..)| t == target) {
            Some(p) => {
                p.1 += 1;
                p.2 = kind;
            }
            None => self.0.push((target.to_owned(), 1, kind)),
        }
    }

    pub fn last(&self, target: &str) -> Option<(u32, PulseKind)> {
        self.0
            .iter()
            .find(|(t, ..)| t == target)
            .map(|&(_, n, k)| (n, k))
    }
}

/// The target element `el`, keyed `key`, dressed for its pulses: pass C8's Trash icon box
/// through it (`PlacesSidebar::trash_icon(|el| dress_target(&fx, &key, el))`).
///
/// Until its first pulse it is `el` itself. From then on it sits in a wrapper keyed by its
/// pulse count, so each pulse mounts it afresh and replays the scale (the wrapper's sequence
/// holds its last pose, its rest, between pulses).
pub fn dress_target(fx: &Effects, key: &str, el: El) -> El {
    match fx.pulses.last(key) {
        None => el,
        Some((n, kind)) => rune::el()
            .key(format!("c23-pulse-{n}"))
            .sequence(Sequence::new(kind.target_beats()))
            .child(el),
    }
}

/// The ring's key over the target keyed `target` for its pulse `n`: one per pulse, so each
/// replays.
pub fn ring_key(target: &str, n: u32) -> String {
    format!("c23-ring-{}-{n}", target.replace('/', "_"))
}

fn ring(look: &Look, key: String, rect: [f32; 4], beats: &'static [Beat]) -> El {
    let k = &look.kit;
    let mut e = el()
        .key(key)
        .absolute(rect[0], rect[1])
        .size(rect[2], rect[3])
        .radius(k.radius.md)
        .border(1.5, k.c.icon_accent)
        .sequence(Sequence::new(beats));
    if look.glow.on {
        e = e.shadow(0.0, 12.0, look.glow.color.alpha(0.22));
    }
    e
}

/// The ring over a target at `rect` that lights while icons fly in.
pub(super) fn receiving(look: &Look, flight: u64, rect: [f32; 4]) -> El {
    ring(
        look,
        format!("c23-receiving-{flight}"),
        rect,
        RECEIVING_RING,
    )
}

/// The ring of the target's latest pulse, if that pulse has one.
pub(super) fn pulse_ring(look: &Look, fx: &Effects, target: &str, rect: [f32; 4]) -> Option<El> {
    let (n, kind) = fx.pulses.last(target)?;
    Some(ring(look, ring_key(target, n), rect, kind.ring_beats()?))
}
