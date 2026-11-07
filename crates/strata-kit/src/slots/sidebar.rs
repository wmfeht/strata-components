//! The `sidebar.*` slot tokens (for example `sidebar.row`) that PlacesSidebar reads.
//!
//! Work package: C8, `docs/work/phase-1-leaves/C8-places-sidebar.md`.
//!
//! W2 seeded `sidebar.row.height` so K4 has a slot to override before C8 lands. C8 owns this
//! file: add tokens beside it, following `slots/mod.rs`. Keep the seed's name, since slot
//! names are API.

use crate::look::Look;
use crate::tokens::Token;

/// PlacesSidebar's slot tokens. `None` inherits.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Sidebar {
    pub row_height: Option<f32>,
}

/// A sidebar row's height: `sidebar.row.height`, else `rows.sidebar`.
pub fn row_height(look: &Look) -> f32 {
    look.slots.sidebar.row_height.unwrap_or(look.rows.sidebar)
}

pub const TOKENS: &[Token] = &[Token::length("sidebar.row.height", row_height, |l, v| {
    l.slots.sidebar.row_height = Some(v)
})];
