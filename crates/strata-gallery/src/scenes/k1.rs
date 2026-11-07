//! The gallery page for K1 Theme mapping: its scenes, demo state, view and key and poll hooks.
//! Scene names start with `k1-` and end in `-light` or `-dark`.
//!
//! Work package: `docs/work/phase-1-leaves/K1-theme-mapping.md`.

use rune::{Cx, El, KeyEvent, Tx};

use crate::{Gallery, Scene};

/// K1's demo state, held in `Gallery::k1`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("k1", dark)`, then fills in `g.k1`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "k1")
}

/// Keys no focused element took while this page is open: shortcuts, and keys with nothing
/// focused. True consumes the key; false lets the gallery's Escape go back to the index.
pub fn on_key(_g: &mut Gallery, _key: &KeyEvent, _tx: &mut Tx) -> bool {
    false
}

/// Runs at the start of every frame while this page is open, with the clock in seconds.
/// Deliver fake async results here; a worker wakes the loop with `Gallery::waker`. True when
/// state changed.
pub fn poll(_g: &mut Gallery, _t: f64, _tx: &mut Tx) -> bool {
    false
}
