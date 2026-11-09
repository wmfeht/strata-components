//! Context menus (C21): where Strata's menus open and how they move, on rune-kit's menus.
//!
//! Work package: C21, `docs/work/phase-1-leaves/C21-context-menus.md`.

mod anchor;
mod field;
mod menu;
mod model;
mod state;
mod view;

#[cfg(test)]
mod tests;

pub use anchor::{AnchorEdge, KeyboardAnchor, MenuKeys, keyboard_anchor, menu_chord};
pub use field::{Clipboard, FieldCommand, FieldLabels, FieldMenu, FieldMenuState};
pub use menu::ContextMenu;
pub use model::{ActionId, ChooserMinimum, Item, MenuModel, Section, chooser_minimum};
pub use state::{ContextMenuState, MenuIntent, MenuTarget};
