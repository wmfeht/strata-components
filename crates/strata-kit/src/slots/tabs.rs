//! The `tabs.*` slot tokens (for example `tabs.tab`) that BrowserTabs reads.
//!
//! Work package: C7, `docs/work/phase-1-leaves/C7-browser-tabs.md`.
//!
//! W2 seeded `tabs.tab.height` so K4 has a slot to override before C7 lands. C7 owns this
//! file: add tokens beside it, following `slots/mod.rs`. Keep the seed's name, since slot
//! names are API.

use crate::look::Look;
use crate::tokens::Token;

/// BrowserTabs' slot tokens. `None` inherits.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Tabs {
    pub tab_height: Option<f32>,
}

/// The tab strip's height: `tabs.tab.height`, else a bar's height plus the frame inset above
/// and below, as Strata's tab strip grows with the header.
pub fn tab_height(look: &Look) -> f32 {
    look.slots
        .tabs
        .tab_height
        .unwrap_or(look.frame.bar + 2.0 * look.frame.inset)
}

pub const TOKENS: &[Token] = &[Token::length("tabs.tab.height", tab_height, |l, v| {
    l.slots.tabs.tab_height = Some(v)
})];
