//! CodeView's own logic, with no window: how a text splits into rows, where matches fall,
//! which row a selected text is, the position bar's rule and the editor's indent edits. What
//! the view does on screen is the gallery's (`strata-gallery/tests/c10.rs`, the c10 scripts).

use std::sync::Arc;

use rune::{ScrollPos, TextEdit, Tx};

use super::copy::locate;
use super::editor::{indent, newline, outdent};
use super::find::Found;
use super::position::{Keys, Position};
use super::rows::{CHUNK, Rows, columns_of};
use super::state::{CodeState, Request};
use crate::document_view::Folded;
use crate::scroll::BarLook;

mod perf;

fn rows(text: &str) -> Rows {
    Rows::new(Arc::from(text))
}

#[test]
fn a_long_line_splits_as_strata_s_source_preview_does() {
    let text = format!("a\n{}\nb\n", "0123456789".repeat(500));
    let r = rows(&text);
    let lens: Vec<usize> = (0..r.len()).map(|i| r.source(i).len()).collect();
    // 5,000 bytes: one 2 KiB row, then the last 2,952 split evenly.
    assert_eq!(lens, [1, CHUNK, 1476, 1476, 1]);
    let conts: Vec<bool> = (0..r.len()).map(|i| r.row(i).cont).collect();
    assert_eq!(conts, [false, false, true, true, false]);
    let lines: Vec<usize> = (0..r.len()).map(|i| r.row(i).line).collect();
    assert_eq!(lines, [0, 1, 1, 1, 2]);
    assert_eq!(r.lines(), 3, "a final line break adds no empty line");
}

#[test]
fn a_split_never_cuts_a_character() {
    let text = "é".repeat(3000);
    let r = rows(&text);
    assert!(r.len() > 1);
    for i in 0..r.len() {
        assert!(r.source(i).len() <= CHUNK);
        assert!(r.source(i).chars().all(|c| c == 'é'));
    }
}

#[test]
fn rows_leave_out_line_breaks_and_carriage_returns() {
    let r = rows("one\r\ntwo\n\nthree");
    let shown: Vec<&str> = (0..r.len()).map(|i| r.source(i)).collect();
    assert_eq!(shown, ["one", "two", "", "three"]);
    assert_eq!(rows("").len(), 1, "an empty text is one empty line");
    assert_eq!(r.row_at(r.line_start(1)), 1);
    assert_eq!(r.lines_of(1..3), 1..3);
}

#[test]
fn columns_count_tabs_to_four_column_stops() {
    assert_eq!(columns_of("\tx"), 5);
    assert_eq!(columns_of("ab\tx"), 5);
    assert_eq!(columns_of("abcd\tx"), 9);
    let r = rows("short\n\tlonger line\n");
    assert_eq!(r.columns, 15);
    assert_eq!(r.widest.first(), Some(&1));
}

#[test]
fn a_match_across_a_split_is_cut_to_each_row() {
    let text = format!("{}needle{}", "x".repeat(CHUNK - 3), "y".repeat(3000));
    let r = rows(&text);
    let found = Found::new(&Folded::new(&text), "NEEDLE");
    assert_eq!(found.ranges, vec![(CHUNK - 3)..(CHUNK + 3)]);
    let first: Vec<_> = found.in_row(&r, 0).collect();
    let second: Vec<_> = found.in_row(&r, 1).collect();
    assert_eq!(first, [(0, CHUNK - 3..CHUNK)]);
    assert_eq!(second, [(0, 0..3)]);
}

#[test]
fn a_selected_text_s_row_comes_from_its_path() {
    let path = "gallery/preview-body/code/#0/code-scroll/code-rows/code-l42/#1/t";
    assert_eq!(locate("code", path), Some(42));
    assert_eq!(
        locate("code", "a/code-l42/#1/n"),
        None,
        "the gutter isn't text"
    );
    assert_eq!(locate("other", path), None);
}

#[test]
fn find_clears_then_selects_and_centres_each_match() {
    use crate::preview_drawer::HostedPreview;
    let text = format!("ab\n{}NEEDLE\nneedle", "x".repeat(3000));
    let mut st = CodeState::new(text);
    st.find("needle");
    // The 3,006-byte line splits into two rows of 1,503: the match is 1,497 bytes into row 2.
    assert_eq!(
        st.pending(),
        [
            Request::Clear,
            Request::Reveal("code-l2".into()),
            Request::Select("code-l2/t".into(), 1497..1503),
        ]
    );
    st.find_step(true);
    assert_eq!(st.current_match(), Some(1));
    st.find_step(true);
    assert_eq!(st.current_match(), Some(0), "wraps at the end");
    st.find("");
    assert!(st.matches().is_empty());
}

/// A bar over `n` rows in a viewport 100 points tall at window y 0, rows
/// mounted as Rune would report them at `offset`.
fn bar(n: usize) -> Position {
    let p = Position::default();
    let look = BarLook::app(&strata_kit::look::Look::fallback(false));
    p.configure(
        Keys {
            scroll: "s".into(),
            list: "l".into(),
            row_prefix: "r".into(),
            fixed: None,
        },
        n,
        look,
    );
    p.track(100.0);
    p.viewport([0.0, 0.0, 100.0, 100.0]);
    p
}

fn scroll(p: &Position, offset: f32, heights: &[f32]) {
    let mut tx = Tx::default();
    let mut y = -offset;
    let mut first = None;
    let mut last = 0;
    for (r, h) in heights.iter().enumerate() {
        if y + h > -200.0 && y < 300.0 {
            p.row(r, [0.0, y, 100.0, *h], &mut tx);
            if y + h > 0.0 && first.is_none() {
                first = Some(r);
            }
            if y < 100.0 {
                last = r + 1;
            }
        }
        y += h;
    }
    p.set_visible(first.unwrap_or(0)..last);
    let total: f32 = heights.iter().sum();
    let mut pos = ScrollPos::default();
    pos.offset = [0.0, offset];
    pos.max = [0.0, (total - 100.0).max(0.0)];
    pos.viewport = [100.0, 100.0];
    pos.drawn = [100.0, 100.0];
    p.follow(pos);
}

#[test]
fn the_position_is_rows_not_offsets() {
    let p = bar(100);
    let heights = vec![10.0; 100];
    scroll(&p, 0.0, &heights);
    assert_eq!(p.progress(), 0.0, "at the first row's top");
    scroll(&p, 450.0, &heights);
    assert!((p.start() - 45.0).abs() < 1e-3);
    assert!((p.progress() - 0.5).abs() < 1e-3);
    scroll(&p, 900.0, &heights);
    assert_eq!(p.progress(), 1.0, "with the last row's end in view");
}

#[test]
fn a_row_measured_below_never_moves_the_thumb_back() {
    let p = bar(100);
    let mut heights = vec![10.0; 100];
    scroll(&p, 450.0, &heights);
    let before = p.progress();
    // A row below the viewport measures taller: the content holds, and so does the thumb.
    heights[80] = 200.0;
    scroll(&p, 450.0, &heights);
    assert_eq!(p.progress(), before);
    // Moving forward never takes it back.
    scroll(&p, 460.0, &heights);
    assert!(p.progress() >= before);
}

#[test]
fn return_keeps_the_indent_up_to_the_caret() {
    let mut e = TextEdit::new("    let x = 1;");
    e.set_caret(e.text.len());
    newline(&mut e);
    assert_eq!(e.text, "    let x = 1;\n    ");
    let mut e = TextEdit::new("\tif x {");
    e.set_caret(1);
    newline(&mut e);
    assert_eq!(e.text, "\t\n\tif x {");
}

#[test]
fn tab_and_shift_tab_follow_gtksourceview() {
    // Spaces to the next stop, counting a tab as reaching one.
    let mut e = TextEdit::new("\tab");
    e.set_caret(3);
    indent(&mut e);
    assert_eq!(e.text, "\tab  ");
    // A selection ending at a line's start leaves that line alone.
    let mut e = TextEdit::new("a\nb\nc");
    e.set_selection(0, 4);
    indent(&mut e);
    assert_eq!(e.text, "    a\n    b\nc");
    outdent(&mut e);
    assert_eq!(e.text, "a\nb\nc");
    // Shift+Tab takes a tab, or up to four spaces, and the caret keeps its place in the text.
    let mut e = TextEdit::new("\tx\n      y");
    e.set_caret(e.text.len());
    outdent(&mut e);
    assert_eq!(e.text, "\tx\n  y");
    assert_eq!(e.range(), (e.text.len(), e.text.len()));
    let mut e = TextEdit::new("\tx");
    e.set_caret(2);
    outdent(&mut e);
    assert_eq!((e.text.as_str(), e.range()), ("x", (1, 1)));
}
