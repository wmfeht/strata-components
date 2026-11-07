//! The gallery page for C20c Tabbed form and settings search (`docs/work/phase-1-leaves/C20c-tabbed-form-settings-search.md`): its scenes, its demo state
//! and its view. Scene names start with `c20c-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C20c's demo state, held in `Gallery::c20c`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c20c", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c20c")
}
