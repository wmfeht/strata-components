//! The gallery page for C20a Slider and NumberField (`docs/work/phase-1-leaves/C20a-slider-number-field.md`): its scenes, its demo state
//! and its view. Scene names start with `c20a-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C20a's demo state, held in `Gallery::c20a`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c20a", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c20a")
}
