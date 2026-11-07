//! The `list.*` slot tokens (for example `list.row`) that ListTable reads.
//!
//! Work package: C4, `docs/work/phase-3-views/C4-list-table.md`.
//!
//! W2 seeded `list.row.height` so K4 has a slot to override before C4 lands. C4 owns this
//! file: add tokens beside it, following `slots/mod.rs`. Keep the seed's name, since slot
//! names are API.

use crate::look::Look;
use crate::tokens::Token;

/// ListTable's slot tokens. `None` inherits.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct List {
    pub row_height: Option<f32>,
}

/// A ListTable row's height: `list.row.height`, else `rows.list`.
pub fn row_height(look: &Look) -> f32 {
    look.slots.list.row_height.unwrap_or(look.rows.list)
}

pub const TOKENS: &[Token] = &[Token::length("list.row.height", row_height, |l, v| {
    l.slots.list.row_height = Some(v)
})];
