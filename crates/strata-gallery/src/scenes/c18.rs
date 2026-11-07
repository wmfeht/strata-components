//! The gallery page for C18 FilterBar and FooterPrompt (`docs/work/phase-2-composites/C18-filter-bar-footer-prompt.md`): its scenes, its demo state
//! and its view. Scene names start with `c18-` and end in `-light` or `-dark`.

use rune::{Cx, El};

use crate::{Gallery, Scene};

/// C18's demo state, held in `Gallery::c18`.
#[derive(Default)]
pub struct Demo {}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c18", dark)`.
pub const SCENES: &[Scene] = &[];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    super::placeholder(g, "c18")
}
