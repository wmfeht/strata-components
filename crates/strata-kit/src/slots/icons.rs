//! The `icons.*` slot tokens (for example `icons.tile`) that IconsGrid reads.
//!
//! Work package: C3, `docs/work/phase-3-views/C3-icons-grid.md`.
//!
//! Empty until C3 adds its tokens here, following `slots/mod.rs`.

use crate::tokens::Token;

/// IconsGrid's slot tokens. `None` inherits.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Icons {}

pub const TOKENS: &[Token] = &[];
