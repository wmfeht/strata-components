//! What assistive technology hears: the roles, and the live status line that reads out
//! selection changes a row's own state doesn't say.
//!
//! The cursor's row is the container's active descendant (focus stays on the container). Rune
//! gives each mounted row of a virtual list its place in the set, and the rows group the set's
//! size; grid cells state their own. A row's selected state comes from `focus_selected`.

use rune::{El, LiveRegion, el, text};

use super::keys::STATUS;

/// The roles a collection and its rows take.
///
/// Used by: C1e (the list and columns), C3 ([`Roles::GRID_CELLS`]), C4 ([`Roles::GRID_ROWS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Roles {
    /// The container's role.
    pub container: &'static str,
    /// A virtual line's role, when a line holds several entries.
    pub line: Option<&'static str>,
    /// Each entry's role.
    pub item: &'static str,
}

impl Roles {
    /// A listbox of options: the list and each Columns column.
    pub const LISTBOX: Roles = Roles {
        container: "listbox",
        line: None,
        item: "option",
    };

    /// A grid of rows, whose cells the content slot draws (C4).
    pub const GRID_ROWS: Roles = Roles {
        container: "grid",
        line: None,
        item: "row",
    };

    /// A grid of rows of cells, one entry per cell (C3).
    pub const GRID_CELLS: Roles = Roles {
        container: "grid",
        line: Some("row"),
        item: "gridcell",
    };
}

impl Default for Roles {
    fn default() -> Self {
        Self::LISTBOX
    }
}

/// What changed, for the status line.
///
/// Fields may be added.
///
/// Used by: hosts (their wording, through the element's `announce`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Announcement {
    /// How many entries are selected now.
    pub selected: usize,
    /// How many entries there are.
    pub total: usize,
}

/// The default wording: a count once more than one entry is selected (one selected entry
/// says so itself, through its row's selected state), else nothing.
pub fn default_announcement(a: &Announcement) -> String {
    if a.selected > 1 {
        format!("{} of {} items selected", a.selected, a.total)
    } else {
        String::new()
    }
}

/// The status line: a polite live region, kept in the layout but one point square, so it
/// draws nothing a person sees.
pub(super) fn status(line: &str) -> El {
    el().pin(Some(0.0), Some(0.0), None, None)
        .w(1.0)
        .h(1.0)
        .clip_soft(0.0)
        .child(
            text(line.to_owned())
                .key(STATUS)
                .live_region(LiveRegion::Polite),
        )
}
