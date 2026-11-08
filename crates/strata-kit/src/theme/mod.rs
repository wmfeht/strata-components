//! Theme mapping: Strata's bundled themes, custom theme files and the Omarchy mapping onto
//! `rune_kit::Kit`.
//!
//! Work package: K1, `docs/work/phase-1-leaves/K1-theme-mapping.md`.

mod catalog;
mod color;
mod colors;
mod file;

pub use catalog::{DEFAULT_ID, UNKNOWN_ID_FALLBACK, bundled, bundled_theme};
pub use color::Rgba8;
pub use colors::{
    Colors, DEFAULT_DANGER, ErrorKind, Key, Location, Source, SourceColors, Theme, ThemeError,
};
pub use file::{Library, parse_custom};
