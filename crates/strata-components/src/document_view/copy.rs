//! What copying a selection takes, by Strata's rule (`selection_text` in
//! `src/ui/virtual_preview/highlight.rs`): every unit between the selection's two ends copies
//! its whole copy text (an image's `[Image: alt]`, a table's rows, a rule), and each end unit
//! the part selected, followed by the block's newline when the selection goes on past it.
//!
//! Rune hands over the selected texts in tree order. Only the two end units are read from it;
//! the units between come from the model, so images and rules, which have no selectable
//! text, still copy. A selection that reaches more than 10,000 unbuilt rows (Rune's cap,
//! R4) is cut short there by Rune: its far end is wrong, except for Ctrl+A, which C11 copies
//! from the model.

use std::collections::BTreeMap;
use std::ops::Range;

use rune::{SelectedText, TextSelection};

use super::model::UnitKind;
use super::state::Content;

/// Which text of a unit a selected text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Part {
    /// The unit's text element, keyed `t`.
    Text,
    /// A table cell, keyed `r{row}c{column}`.
    Cell(usize, usize),
}

/// The unit and part of a selected text, from its path (`…/{key}-u{unit}/…/t`).
pub(crate) fn locate(key: &str, path: &str) -> Option<(usize, Part)> {
    let prefix = format!("{key}-u");
    let mut unit = None;
    let mut last = "";
    for seg in path.split('/') {
        if let Some(n) = seg.strip_prefix(&prefix).and_then(|n| n.parse().ok()) {
            unit = Some(n);
        }
        last = seg;
    }
    let part = if last == "t" {
        Part::Text
    } else {
        let (r, c) = last.strip_prefix('r')?.split_once('c')?;
        Part::Cell(r.parse().ok()?, c.parse().ok()?)
    };
    Some((unit?, part))
}

struct Piece<'a> {
    part: Part,
    range: Range<usize>,
    whole: bool,
    text: &'a SelectedText,
}

pub(crate) fn copy(state: &Content, sel: &TextSelection) -> String {
    let key = state.key.as_str();
    let units = &state.doc.units;
    let mut by_unit: BTreeMap<usize, Vec<Piece<'_>>> = BTreeMap::new();
    for t in sel.texts().iter().filter(|t| t.is_selected()) {
        if let Some((unit, part)) = locate(key, t.path())
            && unit < units.len()
        {
            by_unit.entry(unit).or_default().push(Piece {
                part,
                range: t.range(),
                whole: t.is_whole(),
                text: t,
            });
        }
    }
    let (Some(&first), Some(&last)) = (by_unit.keys().next(), by_unit.keys().next_back()) else {
        // Nothing of the document's: what Rune would copy.
        return sel.text().to_owned();
    };
    let mut out = String::new();
    for (i, unit) in units.iter().enumerate().take(last + 1).skip(first) {
        let Some(pieces) = by_unit.get(&i).filter(|_| i == first || i == last) else {
            out.push_str(&unit.copy_text);
            continue;
        };
        match &unit.kind {
            UnitKind::Table { rows, .. } => {
                let cells: usize = rows.iter().map(Vec::len).sum();
                if pieces.len() == cells && pieces.iter().all(|p| p.whole) {
                    out.push_str(&unit.copy_text);
                } else {
                    table_part(&mut out, pieces);
                    if i < last {
                        out.push('\n');
                    }
                }
            }
            _ => {
                let Some(p) = pieces.iter().find(|p| p.part == Part::Text) else {
                    continue;
                };
                let len = state.shown(i).0.len();
                if p.range.start == 0 && p.range.end >= len {
                    out.push_str(&unit.copy_text);
                    continue;
                }
                match state.shown_of(i) {
                    Some(shown) => out.push_str(&shown.copy(p.range.clone())),
                    None => out.push_str(&unit.text[p.range.clone()]),
                }
                if p.range.end >= len && i < last && unit.copy_text.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// The selected cells of a table, a tab between cells of a row and a newline between rows.
fn table_part(out: &mut String, pieces: &[Piece<'_>]) {
    let mut row = None;
    for p in pieces {
        let Part::Cell(r, _) = p.part else { continue };
        match row {
            Some(prev) if prev == r => out.push('\t'),
            Some(_) => out.push('\n'),
            None => {}
        }
        row = Some(r);
        out.push_str(p.text.text());
    }
}
