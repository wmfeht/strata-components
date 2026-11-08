//! ColumnsView (C2): Miller columns, a sideways strip of C1e collections, each its own virtual
//! list.
//!
//! Work package: C2, `docs/work/phase-3-views/C2-columns-view.md`.

#![warn(missing_docs)]

mod column;
pub mod drag_scroll;
mod fit;
mod intent;
pub mod keys;
mod pointer;
mod reveal;
mod sources;
mod state;
mod strip;
mod view;
pub mod width;

#[cfg(test)]
mod tests;

pub use intent::ColumnsIntent;
pub use sources::ColumnSources;
pub use state::{ColumnState, ColumnsState, Focus, Opening};
pub use view::{ColumnsLabels, ColumnsParts, ColumnsPartsMut, ColumnsView, HeaderCx};
