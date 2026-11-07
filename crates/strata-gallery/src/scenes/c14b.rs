//! The gallery page for C14b Viewers and archive preview (`docs/work/phase-3-views/C14b-viewers.md`): its scenes, its demo state
//! and its view. Scene names start with `c14b-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C14b's demo state, held in `Gallery::c14b`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c14b", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c14b")
}
