//! How entries pack into Rune's virtual lines.
//!
//! Rune virtualizes vertical rows only, so a grid (C3) packs several entries into each virtual
//! line. The list and each Columns column put one entry on each line ([`OnePerLine`]).
//!
//! # Lines that aren't entries (planned)
//!
//! ListTable's grouped mode (C4) needs lines that hold no entry: a group's header. That is a
//! planned, additive extension of this trait, so that C4 never edits the element:
//!
//! ```ignore
//! /// A run of lines: a header, or entries.
//! #[non_exhaustive]
//! pub enum Span { Header { key: u64, height: f32 }, Entries { range: Range<usize> } }
//!
//! trait Lines {
//!     // …as now, plus, with a default of `None` (no headers):
//!     fn spans(&self, len: usize) -> Option<Rc<[Span]>> { None }
//! }
//! // and on the builder: `.header_line(Slot-like Fn(&S, key) -> El)`.
//! ```
//!
//! With spans, the element keys a header line `head-{key}`, maps lines to entries through
//! prefix sums over the spans (O(log spans) a lookup, nothing per entry), and gives the
//! geometry the same spans so presses, the marquee and Page Up and Down skip headers. Rune
//! keeps a sticky header only inside its own virtual row, so the header that stays at the top
//! is an overlay (`Collection::overlay`) the host fills from the group of the first visible
//! entry (`on_visible`).

use crate::contract::{CollectionGeometry, UniformRows};

/// How many entries each virtual line holds, and the geometry they make.
///
/// Every line is the element's row height tall, so the geometry's rects must match what the
/// element draws: line `l` spans `[l·h, (l+1)·h)`, and its entries share the width equally,
/// in index order.
///
/// Used by: C1e ([`OnePerLine`]), C3 (a grid of N per line).
pub trait Lines {
    /// How many entries each line holds in a viewport `width` points wide: at least 1, the
    /// same for every line. The first build has no width yet (0): return 1 or a guess; the
    /// element builds again once the viewport's width gives another count.
    fn per_line(&self, width: f32) -> usize;

    /// The geometry of `len` entries in lines `row_height` tall, in a viewport `width` by
    /// `viewport_height` points, in content coordinates.
    fn geometry(
        &self,
        len: usize,
        row_height: f32,
        width: f32,
        viewport_height: f32,
    ) -> Box<dyn CollectionGeometry>;
}

/// One entry per line: the list and each Columns column.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePerLine;

impl Lines for OnePerLine {
    fn per_line(&self, _width: f32) -> usize {
        1
    }

    fn geometry(
        &self,
        len: usize,
        row_height: f32,
        width: f32,
        viewport_height: f32,
    ) -> Box<dyn CollectionGeometry> {
        Box::new(UniformRows::new(len, row_height, width).viewport_height(viewport_height))
    }
}

/// How many lines hold `len` entries, `per_line` to a line.
pub(super) fn line_count(len: usize, per_line: usize) -> usize {
    len.div_ceil(per_line.max(1))
}
