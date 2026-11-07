//! The gallery page for K1 Theme mapping (`docs/work/phase-1-leaves/K1-theme-mapping.md`): its scenes, its demo state
//! and its view. Scene names start with `k1-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// K1's demo state, held in `Gallery::k1`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("k1", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "k1")
}
