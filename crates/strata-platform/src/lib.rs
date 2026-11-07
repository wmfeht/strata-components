//! Linux platform shims for [Rune](https://github.com/anomalyco/rune) apps.
//!
//! Each module fills one gap in Rune's Linux support, sits behind its own cargo feature, and
//! links the Rune issue that tracks the upstream fix. A module is deprecated for one release
//! once Rune ships its feature, then removed in the next minor release.
//!
//! This crate is generic: it depends on `rune` alone and carries no Strata names in its API.
