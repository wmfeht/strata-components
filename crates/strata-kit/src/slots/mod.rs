//! Component slots: one file per component that reads named tokens (`list.row`,
//! `sidebar.row`, `tabs.tab`, …). Slots are API (plan K4).
//!
//! Work package: W2, `docs/work/phase-0-groundwork/W2-look-contract.md`. Each component's work
//! package fills in its own slot file. W0 declared the slot files the work package docs name,
//! so their owners never edit this file; a new slot file is declared here in its own PR.
//!
//! # A slot file
//!
//! A slot file holds three things, all in that file:
//!
//! 1. A struct of the slot's tokens, held in [`Slots`]. Each field is an `Option`: `None`
//!    inherits a value from the rest of the [`Look`], as CSS inherits.
//! 2. One reader per token, `fn(&Look) -> T`, which the component calls. It returns the set
//!    value, or else the inherited one.
//! 3. `TOKENS`, the slot's entries in the registry ([`crate::tokens`]): one line per token,
//!    naming it `<slot>.<part>.<property>` (`list.row.height`).
//!
//! ```ignore
//! #[derive(Clone, Debug, Default)]
//! pub struct List {
//!     pub row_height: Option<f32>,
//! }
//!
//! /// A ListTable row's height: `list.row.height`, else `rows.list`.
//! pub fn row_height(look: &Look) -> f32 {
//!     look.slots.list.row_height.unwrap_or(look.rows.list)
//! }
//!
//! pub const TOKENS: &[Token] = &[Token::length("list.row.height", row_height, |l, v| {
//!     l.slots.list.row_height = Some(v)
//! })];
//! ```
//!
//! K4's `[list.row]` table then sets `list.row.height`, and the component reads it through
//! `row_height`. A slot token nothing set follows its source, so a user override of
//! `rows.list` moves the list rows too.
//!
//! Renaming or removing a slot token breaks users' override files: record it in the changelog
//! (plan K4).
//!
//! # A new slot file
//!
//! Its own small PR edits this file in three places: the `mod` line, the [`Slots`] field and
//! the [`TOKENS`] entry.

use crate::tokens::Token;

#[cfg(doc)]
use crate::look::Look;

pub mod icons;
pub mod list;
pub mod sidebar;
pub mod tabs;

#[cfg(test)]
mod tests;

/// Every slot's tokens, held in `Look::slots`.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Slots {
    /// IconsGrid (C3).
    pub icons: icons::Icons,
    /// ListTable (C4).
    pub list: list::List,
    /// PlacesSidebar (C8).
    pub sidebar: sidebar::Sidebar,
    /// BrowserTabs (C7).
    pub tabs: tabs::Tabs,
}

/// Every slot's registry entries, which `tokens::all` lists after the core tokens.
pub const TOKENS: &[&[Token]] = &[icons::TOKENS, list::TOKENS, sidebar::TOKENS, tabs::TOKENS];
