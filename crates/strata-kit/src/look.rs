//! `Look`: the `rune_kit::Kit` plus Strata's extension tokens, the one surface components read
//! their look from.
//!
//! Work package: W2, `docs/work/phase-0-groundwork/W2-look-contract.md`.

use rune_kit::Kit;

/// The kit components draw with until a Strata theme is loaded.
///
/// `lib.rs` re-exports this as `strata_kit::fallback`, and the gallery calls it. W2 keeps the
/// name here and makes it forward to `Look::fallback`, so neither `lib.rs` nor the gallery
/// changes. Renaming or removing it means editing `lib.rs` and `Gallery::kit` in the same PR.
pub fn fallback(dark: bool) -> Kit {
    Kit::neutral(dark)
}
