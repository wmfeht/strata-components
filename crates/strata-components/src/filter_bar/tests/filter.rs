use rune::TextEdit;

use crate::collection::selection::{Position, Selection};
use crate::contract::{EntryId, EntryKind, ListEntry, ListSource};
use crate::filter_bar::filter::{FilterIntent, FilterState, Polled, Results, Shows};

fn source(names: &[&str]) -> ListSource {
    ListSource::new(
        names
            .iter()
            .enumerate()
            .map(|(i, n)| ListEntry::new(EntryId::new(i as u64 + 1), *n, EntryKind::File, "file"))
            .collect(),
    )
}

fn id(n: u64) -> EntryId {
    EntryId::new(n)
}

/// An open bar with `text` typed and its results shown, the field focused.
fn typed(st: &mut FilterState, text: &str) -> Polled {
    let _ = st.open();
    st.edit = TextEdit::new(text);
    st.edit.focused = true;
    let out = st.text_changed_for_tests();
    st.shown(text);
    out
}

impl FilterState {
    fn text_changed_for_tests(&mut self) -> Polled {
        self.text_changed(Position::default)
    }
}

fn r<'a>(src: &'a ListSource, sel: &'a Selection) -> Results<'a> {
    Results {
        source: src,
        selection: sel,
    }
}

// browser/search/filter/enter-target: "In Icons and List, Enter in the field with no pick
// opens the outlined top result", and the outline needs the field's keyboard.
#[test]
fn enter_opens_the_top_result_outlined_while_the_field_has_the_keyboard() {
    let src = source(&["report.pdf", "reports", "repo"]);
    let sel = Selection::new();
    let mut st = FilterState::new();
    let _ = typed(&mut st, "rep");
    assert_eq!(st.target(r(&src, &sel)), Some(id(1)));
    assert_eq!(st.outlined(r(&src, &sel)), Some(id(1)));
    st.edit.focused = false;
    assert_eq!(
        st.outlined(r(&src, &sel)),
        None,
        "focus elsewhere removes the outline"
    );
    assert_eq!(
        st.target(r(&src, &sel)),
        Some(id(1)),
        "Enter's target stays"
    );
}

// "A pick… removes the outline; Enter then opens the pick", in display order: Strata's
// `filter_activation_position` opens the first selected result.
#[test]
fn after_a_pick_enter_opens_the_first_selected_result_in_display_order() {
    let src = source(&["a", "b", "c", "d"]);
    let mut sel = Selection::new();
    let mut st = FilterState::new();
    let _ = typed(&mut st, "x");
    let _ = sel.select_only(&src, 3);
    let _ = sel.toggle(&src, 1);
    st.pick();
    assert_eq!(sel.cursor(), Some(id(2)), "the cursor is on the last click");
    assert_eq!(st.target(r(&src, &sel)), Some(id(2)));
    assert_eq!(st.outlined(r(&src, &sel)), None);
}

// "Deselecting every result after a pick returns the outline and Enter to the top result."
#[test]
fn deselecting_every_result_after_a_pick_returns_the_top_result() {
    let src = source(&["a", "b"]);
    let mut sel = Selection::new();
    let mut st = FilterState::new();
    let _ = typed(&mut st, "x");
    let _ = sel.select_only(&src, 1);
    st.pick();
    assert_eq!(st.target(r(&src, &sel)), Some(id(2)));
    let _ = sel.clear();
    assert_eq!(st.target(r(&src, &sel)), Some(id(1)));
    assert_eq!(st.outlined(r(&src, &sel)), Some(id(1)));
}

// "In search results, editing the query text drops the pick and the results' selection";
// "Editing the query and typing it back… still counts as a query edit"; "In Trash, Recent,
// or a network share… a query edit keeps the selection but drops the pick"; "A same-query
// update… keeps the pick".
#[test]
fn a_query_edit_drops_the_pick_and_a_same_query_update_keeps_it() {
    let mut st = FilterState::new();
    let _ = typed(&mut st, "duck");
    st.pick();
    assert!(!st.shown("duck"), "a same-query update keeps the pick");
    assert!(st.tracker.picked);
    // Typed back: an edit, though the text is the same.
    st.edit = TextEdit::new("duck");
    let _ = st.text_changed_for_tests();
    assert!(
        st.shown("duck"),
        "results replacing the listing drop their selection"
    );
    assert!(!st.tracker.picked);

    let mut st = FilterState::new();
    st.set_shows(Shows::InPlace);
    let _ = typed(&mut st, "duck");
    st.pick();
    st.edit = TextEdit::new("duck ");
    let _ = st.text_changed_for_tests();
    assert!(
        !st.shown("duck "),
        "in place, the listing keeps its selection"
    );
    assert!(!st.tracker.picked, "but the pick goes");
}

// "With 10xer mode on, no result is outlined and a query edit keeps the selected result."
#[test]
fn tenxer_mode_outlines_nothing_and_keeps_the_selection() {
    let src = source(&["a", "b"]);
    let mut sel = Selection::new();
    let mut st = FilterState::new();
    st.set_tenxer(true);
    let _ = typed(&mut st, "a");
    assert_eq!(st.outlined(r(&src, &sel)), None);
    assert_eq!(
        st.target(r(&src, &sel)),
        None,
        "nothing selected, nothing to open"
    );
    let _ = sel.select_only(&src, 1);
    st.edit = TextEdit::new("ab");
    let _ = st.text_changed_for_tests();
    assert!(!st.shown("ab"));
    assert_eq!(st.target(r(&src, &sel)), Some(id(2)));
}

// "While the field has focus, the list keeps the top result scrolled into view as results
// arrive. A button press, wheel scroll… stops that scrolling; the next query edit scrolls
// back to the top result."
#[test]
fn the_top_result_follows_until_the_results_scroll_and_again_after_an_edit() {
    let src = source(&["a", "b"]);
    let sel = Selection::new();
    let mut st = FilterState::new();
    let _ = typed(&mut st, "a");
    assert_eq!(st.take_reveal(r(&src, &sel)), Some(0));
    assert_eq!(st.take_reveal(r(&src, &sel)), None, "once per batch");
    st.results_scrolled();
    st.shown("a");
    assert_eq!(st.take_reveal(r(&src, &sel)), None, "the person scrolled");
    st.edit.text = "ab".into();
    let _ = st.text_changed_for_tests();
    st.shown("ab");
    assert_eq!(st.take_reveal(r(&src, &sel)), Some(0));
}

// "Escape in the filter field, or clearing the query, restores the normal listing", with the
// listing's own selection when results replaced it.
#[test]
fn closing_or_clearing_gives_back_the_listing_and_its_selection() {
    let before = Position {
        cursor: Some(id(7)),
        selected: vec![id(7)],
        ..Position::default()
    };
    let mut st = FilterState::new();
    let out = st.open_with("r", before.clone());
    assert_eq!(out.intents, vec![FilterIntent::Query { text: "r".into() }]);
    let out = st.close();
    assert_eq!(
        out.intents,
        vec![
            FilterIntent::ShowListing {
                restore: Some(before.clone())
            },
            FilterIntent::Closed
        ]
    );
    assert!(!st.is_open());

    let _ = st.open_with("r", before.clone());
    st.edit = TextEdit::new("");
    let out = st.text_changed_for_tests();
    assert_eq!(
        out.intents,
        vec![FilterIntent::ShowListing {
            restore: Some(before.clone())
        }]
    );
    assert!(st.is_open(), "clearing keeps the field open");

    let mut st = FilterState::new();
    st.set_shows(Shows::InPlace);
    let _ = st.open_with("r", before);
    let out = st.close();
    assert_eq!(
        out.intents,
        vec![
            FilterIntent::ShowListing { restore: None },
            FilterIntent::Closed
        ]
    );
}

// An empty filter closing has no listing to give back.
#[test]
fn closing_an_empty_filter_only_closes() {
    let mut st = FilterState::new();
    let _ = st.open();
    assert_eq!(st.close().intents, vec![FilterIntent::Closed]);
    assert_eq!(st.close(), Polled::default());
}

// The outline shows and goes with the field's keyboard: tick reports the change.
#[test]
fn tick_reports_the_field_gaining_and_losing_the_keyboard() {
    let mut st = FilterState::new();
    let _ = st.open();
    assert!(st.tick().changed);
    assert!(!st.tick().changed);
    st.edit.focused = false;
    assert!(st.tick().changed);
    assert!(st.is_open(), "the filter stays open");
    st.focus_field();
    assert!(st.edit.focused);
}
