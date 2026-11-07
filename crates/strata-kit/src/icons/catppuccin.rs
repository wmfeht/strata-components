//! The Catppuccin catalog: multi-colour icons, drawn in their own colours, generated at build
//! time behind the semantic `IconName` set. Licence: MIT.
//!
//! Work package: K3, `docs/work/phase-1-leaves/K3-icon-catalogs.md`.

use super::{IconSet, PLACEHOLDER};

/// The Catppuccin catalog. W2's placeholder until K3 generates the real one: K3 replaces
/// this line, and only the goldens move.
pub const SET: IconSet = PLACEHOLDER.renamed("catppuccin");
