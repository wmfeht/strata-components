//! `Look`: the `rune_kit::Kit` plus Strata's extension tokens, the one surface components read
//! their look from.
//!
//! Work package: W2, `docs/work/phase-0-groundwork/W2-look-contract.md`.

use rune_kit::Kit;

/// The kit components draw with until a Strata theme is loaded.
///
/// `lib.rs` re-exports this as `strata_kit::fallback`, and the gallery calls it. W2 keeps the
/// name here and makes it forward to `Look::fallback`, so `lib.rs` doesn't change. Renaming or
/// removing it means editing `lib.rs` and `Gallery::kit` in the same PR.
///
/// The gallery: W2's PR is the one sanctioned edit to strata-gallery's `src/lib.rs`. It adds a
/// `look: Look` field to `Gallery` with the accessor components take (`fn(&Gallery) -> &Look`),
/// and makes `Gallery::open` rebuild it for `dark`. From then on, K1, K2 and K4 change `g.look`
/// only from their own `scenes/<id>.rs`.
pub fn fallback(dark: bool) -> Kit {
    Kit::neutral(dark)
}
