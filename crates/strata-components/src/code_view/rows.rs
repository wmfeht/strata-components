//! The text's display rows: one per line, and a line over [`CHUNK`] bytes split into rows of at
//! most that, as Strata's large source preview splits it (`push_source_line_chunks` in
//! `src/ui/virtual_preview/source.rs`): on character boundaries, with the last two rows of a
//! line splitting its last 4 KiB about evenly. Rune draws a tab to the next four-column stop
//! (measured at the pin), as Strata's `tab_width(4)` does, so rows draw their text as it is.

use std::ops::Range;
use std::sync::Arc;

/// The longest row, in bytes (Strata's `PATHOLOGICAL_TEXT_UNIT_BYTES`).
pub const CHUNK: usize = 2 * 1024;

/// Tabs stop every this many columns (Strata's `tab_width(4)`, and Rune's own stops).
pub const TAB: usize = 4;

/// One display row: bytes `start..end` of the text (no line break), part of line `line`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    /// A later part of a line split at [`CHUNK`]: its gutter shows "↳", not a number.
    pub cont: bool,
}

/// The rows of a text, with where each line starts.
#[derive(Debug, Default)]
pub(crate) struct Rows {
    pub text: Arc<str>,
    rows: Vec<Row>,
    /// Each line's start in the text.
    line_starts: Vec<usize>,
    /// The most columns any row takes, tabs expanded.
    pub columns: usize,
    /// The rows that draw widest by [`width_of`]'s estimate, widest first: a fallback face
    /// (CJK, emoji) draws them wider than their columns, so the view measures these.
    pub widest: Vec<usize>,
}

/// How many of the widest rows the view measures for its unwrapped width.
const MEASURED: usize = 6;

impl Rows {
    pub fn new(text: Arc<str>) -> Rows {
        let mut rows = Vec::new();
        let mut line_starts = Vec::new();
        let mut at = 0;
        let len = text.len();
        // A text ending in a line break has no empty line after it, as Strata's large source
        // preview shows it (`split_inclusive`); an empty text is one empty line.
        while at < len || line_starts.is_empty() {
            let next = text[at..].find('\n').map_or(len, |i| at + i);
            let end = if text[..next].ends_with('\r') && next > at {
                next - 1
            } else {
                next
            };
            let line = line_starts.len();
            line_starts.push(at);
            split(&text, at, end, line, &mut rows);
            at = next + 1;
            if next == len {
                break;
            }
        }
        let mut columns = 0;
        let mut by_width: Vec<(usize, usize)> = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let s = &text[r.start..r.end];
            columns = columns.max(columns_of(s));
            let n = width_of(s);
            if by_width.len() < MEASURED || n > by_width[by_width.len() - 1].0 {
                by_width.push((n, i));
                by_width.sort_by_key(|&(n, _)| std::cmp::Reverse(n));
                by_width.truncate(MEASURED);
            }
        }
        Rows {
            text,
            rows,
            line_starts,
            columns,
            widest: by_width.into_iter().map(|(_, i)| i).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn lines(&self) -> usize {
        self.line_starts.len()
    }

    pub fn row(&self, i: usize) -> Row {
        self.rows[i]
    }

    pub fn get(&self, i: usize) -> Option<Row> {
        self.rows.get(i).copied()
    }

    /// Row `i`'s bytes.
    pub fn source(&self, i: usize) -> &str {
        let r = self.rows[i];
        &self.text[r.start..r.end]
    }

    /// Where line `line` starts in the text.
    pub fn line_start(&self, line: usize) -> usize {
        self.line_starts[line]
    }

    /// The lines rows `rows` show part of.
    pub fn lines_of(&self, rows: Range<usize>) -> Range<usize> {
        if rows.is_empty() || self.rows.is_empty() {
            return 0..0;
        }
        let last = (rows.end - 1).min(self.rows.len() - 1);
        let first = rows.start.min(last);
        self.rows[first].line..self.rows[last].line + 1
    }

    /// The row holding byte `at` of the text: a line break belongs to the row before it.
    pub fn row_at(&self, at: usize) -> usize {
        self.rows
            .partition_point(|r| r.start <= at)
            .saturating_sub(1)
    }
}

/// Splits line `start..end` into rows, as Strata's `push_source_line_chunks` does.
fn split(text: &str, start: usize, end: usize, line: usize, rows: &mut Vec<Row>) {
    let mut at = start;
    let mut cont = false;
    while end - at > CHUNK {
        let left = end - at;
        let take = if left < CHUNK * 2 {
            left.div_ceil(2)
        } else {
            CHUNK
        };
        let mut cut = at + take;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        rows.push(Row {
            start: at,
            end: cut,
            line,
            cont,
        });
        at = cut;
        cont = true;
    }
    rows.push(Row {
        start: at,
        end,
        line,
        cont,
    });
}

/// An estimate of how wide `s` draws, in columns of the mono face: East Asian wide
/// characters and emoji take two (`unicode-width`), tabs run to the next [`TAB`] stop, and
/// control characters one. It only ranks rows for measuring; the width itself is measured.
pub(crate) fn width_of(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;
    s.chars().fold(0, |col, ch| {
        if ch == '\t' {
            col + TAB - col % TAB
        } else {
            col + ch.width().unwrap_or(1)
        }
    })
}

/// The columns `s` takes, each tab running to the next [`TAB`] stop.
pub(crate) fn columns_of(s: &str) -> usize {
    s.chars().fold(0, |col, ch| {
        if ch == '\t' {
            col + TAB - col % TAB
        } else {
            col + 1
        }
    })
}
