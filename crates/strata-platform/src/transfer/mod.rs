//! The transfer substrate: protocol bindings on winit's display, multi-MIME offers and
//! sources, and X11 selections. Internal: `dnd`, `clipboard` and `foreign-parent` build on it.
//!
//! Rune lacks: (filled in by P0).
//! Rune issue: (filed by S5).
//!
//! Work package: P0, `docs/work/phase-1-leaves/P0-transfer-substrate.md`.
//!
//! Nothing calls this module until P1, P2 or P4 lands, so it is dead code under `_transfer`
//! alone. P0 adds `#![expect(dead_code, reason = "used by dnd, clipboard and foreign-parent
//! once P1, P2 and P4 land")]` here and removes it when the first caller arrives.
