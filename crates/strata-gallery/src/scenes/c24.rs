//! The gallery page for C24 Scroll extras (`docs/work/phase-1-leaves/C24-scroll-extras.md`): its scenes, its demo state
//! and its view. Scene names start with `c24-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C24's demo state, held in `Gallery::c24`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c24", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c24")
}
