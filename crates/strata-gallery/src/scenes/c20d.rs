//! The gallery page for C20d IconPicker (`docs/work/phase-4-integration/C20d-icon-picker.md`): its scenes, its demo state
//! and its view. Scene names start with `c20d-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C20d's demo state, held in `Gallery::c20d`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c20d", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c20d")
}
