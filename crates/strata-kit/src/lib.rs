//! Strata's look on top of `rune_kit::Kit`: theme mapping, icon catalogs, density and text
//! size. Components read these tokens, never a theme by name.

use rune_kit::Kit;

/// The kit components draw with until a Strata theme is loaded.
pub fn fallback(dark: bool) -> Kit {
    Kit::neutral(dark)
}
