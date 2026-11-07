//! The gallery page for C21 Context menus (`docs/work/phase-1-leaves/C21-context-menus.md`): its scenes, its demo state
//! and its view. Scene names start with `c21-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C21's demo state, held in `Gallery::c21`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c21", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c21")
}
