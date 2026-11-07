//! The keys the element gives its parts, so hosts, scripts and motion effects (C23) can name a
//! row, its icon and its label, and scroll requests can name the viewport.
//!
//! Keys are one namespace in Rune, and sibling keys must be unique. Rows are keyed by entry id
//! (`row-{id}`), which is unique inside one collection; two collections showing the same
//! folder (two panes) tell their rows apart by path: [`row_path`] starts at the collection's
//! own key.

use crate::contract::EntryId;

/// The scroll viewport's key, under the collection's key.
pub const VIEWPORT: &str = "viewport";

/// The virtual rows stack's key, under the viewport.
pub const ROWS: &str = "rows";

/// The row icon's key, under its row.
pub const ICON: &str = "icon";

/// The row label's key, under its row.
pub const LABEL: &str = "label";

/// The live status line's key, under the collection's key.
pub const STATUS: &str = "status";

/// The key of the row (or grid cell) that shows entry `id`.
///
/// Used by: C1e, C5 (the row to keep on screen), C23 (flight sources), scripts.
pub fn row_key(id: EntryId) -> String {
    format!("row-{}", id.get())
}

/// The key of the virtual line whose first entry is `first`: the row itself when each line
/// holds one entry, else `line-{id}`.
pub(super) fn line_key(first: EntryId, per_line: usize) -> String {
    if per_line <= 1 {
        row_key(first)
    } else {
        format!("line-{}", first.get())
    }
}

/// The full key path of the viewport in the collection keyed `scope`, for scroll requests.
pub fn viewport_path(scope: &str) -> String {
    format!("{scope}/{VIEWPORT}")
}

/// The full key path of entry `id`'s row in the collection keyed `scope`, when each line holds
/// one entry. It names the row even when another collection keys a row alike.
pub fn row_path(scope: &str, id: EntryId) -> String {
    format!("{scope}/{VIEWPORT}/{ROWS}/{}", row_key(id))
}

/// The full key path of a part (`ICON`, `LABEL`) of entry `id`'s row in the collection keyed
/// `scope`: what a motion effect flies from (C23).
pub fn part_path(scope: &str, id: EntryId, part: &str) -> String {
    format!("{}/{part}", row_path(scope, id))
}
