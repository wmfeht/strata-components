//! FilterBar and FooterPrompt (C18).
//!
//! Work package: C18, `docs/work/phase-2-composites/C18-filter-bar-footer-prompt.md`.

pub mod chord;
mod field;
pub mod filter;
mod flash;
mod keycap;
pub mod prompt;

#[cfg(test)]
mod tests;

pub use flash::{FLASH_FOR, Flash, flash};
pub use keycap::keycap;
