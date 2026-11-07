//! `Look`: the `rune_kit::Kit` plus Strata's extension tokens, the one surface components read
//! their look from.
//!
//! Work package: W2, `docs/work/phase-0-groundwork/W2-look-contract.md`.

use rune_kit::Kit;

/// The kit components draw with until a Strata theme is loaded.
pub fn fallback(dark: bool) -> Kit {
    Kit::neutral(dark)
}
