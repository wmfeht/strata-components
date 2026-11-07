//! The gallery page for C19 Footer and ShortcutReference (`docs/work/phase-2-composites/C19-footer-shortcut-reference.md`): its scenes, its demo state
//! and its view. Scene names start with `c19-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C19's demo state, held in `Gallery::c19`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c19", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c19")
}
