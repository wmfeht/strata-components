//! How entries pack into Rune's virtual lines.
//!
//! Rune virtualizes vertical rows only, so a grid (C3) packs several entries into each virtual
//! line. The list and each Columns column put one entry on each line ([`OnePerLine`]).

use crate::contract::{CollectionGeometry, UniformRows};

/// How many entries each virtual line holds, and the geometry they make.
///
/// Every line is the element's row height tall, so the geometry's rects must match what the
/// element draws: line `l` spans `[l·h, (l+1)·h)`, and its entries share the width equally,
/// in index order.
///
/// Used by: C1e ([`OnePerLine`]), C3 (a grid of N per line).
pub trait Lines {
    /// How many entries each line holds: at least 1, the same for every line.
    fn per_line(&self) -> usize;

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
    fn per_line(&self) -> usize {
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
