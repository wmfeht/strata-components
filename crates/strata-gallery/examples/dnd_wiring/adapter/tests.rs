//! The adapter's mappings, headless: what a drag of files offers, byte for byte, and how W1's
//! actions and hovers map onto P1's.

use strata_components::contract::EntryId;

use super::*;

fn payload(uris: &[&str]) -> DragPayload {
    DragPayload::new(
        (1..=uris.len() as u64).map(EntryId::new).collect(),
        uris.iter().map(|u| (*u).to_owned()).collect(),
    )
}

/// operations/drag-and-drop: dragged items are offered as a file list other applications read:
/// `text/uri-list` (CRLF after each), GNOME's (`copy`, then each URI after a newline, none
/// at the end) and KDE's cut flag (`0`), as GTK and P2's clipboard write them.
#[test]
fn the_three_file_formats_are_written_as_gtk_writes_them() {
    let uris = vec!["file:///a/b%20c.txt".to_owned(), "file:///d".to_owned()];
    assert_eq!(
        encode_uri_list(&uris),
        b"file:///a/b%20c.txt\r\nfile:///d\r\n"
    );
    assert_eq!(
        encode_gnome_copied_files(&uris),
        b"copy\nfile:///a/b%20c.txt\nfile:///d"
    );
    let item = drag_item(&payload(&["file:///a"]), None);
    let mimes: Vec<&str> = item.mimes().collect();
    assert_eq!(mimes, [URI_LIST, GNOME_COPIED_FILES, KDE_CUT_SELECTION]);
    assert_eq!(item.allowed(), Actions::COPY | Actions::MOVE);
    assert!(!item.is_empty());
}

/// W1's actions map onto P1's one for one, both ways; ask goes to the source as a copy.
#[test]
fn actions_map_both_ways_and_ask_is_offered_as_a_copy() {
    let all = DropActions::COPY | DropActions::MOVE | DropActions::LINK;
    assert_eq!(
        actions_to_p1(all),
        Actions::COPY | Actions::MOVE | Actions::LINK
    );
    assert_eq!(actions_of_p1(actions_to_p1(all)), all);
    assert_eq!(actions_to_p1(DropActions::MOVE), Actions::MOVE);
    assert_eq!(
        actions_of_p1(Actions::ASK | Actions::COPY),
        DropActions::COPY
    );
    assert_eq!(action_to_p1(DropAction::Ask), DragAction::Copy);
    assert_eq!(action_to_p1(DropAction::Move), DragAction::Move);
    assert_eq!(action_to_p1(DropAction::Link), DragAction::Link);
    let item = drag_item(&payload(&["file:///a"]).actions(DropActions::COPY), None);
    assert_eq!(item.allowed(), Actions::COPY, "the payload's actions");
}

#[test]
fn hovers_map_onto_w1s() {
    assert_eq!(
        hover_of_p1(Hover::Accepting(DragAction::Move)),
        DropHover::Accepting
    );
    assert_eq!(hover_of_p1(Hover::Rejecting), DropHover::Rejecting);
    assert_eq!(hover_of_p1(Hover::Left), DropHover::Left);
}

/// The sentinel's end report: a drag that left every target and reached none by the next
/// tick ended (dropped, cancelled or gone); one that moved to another target didn't.
#[test]
fn a_drag_ends_when_it_leaves_every_target_until_the_next_tick() {
    struct S(u32);
    let w = Wiring::<S>::new(strata_platform::dnd::Dnd::new());
    let _ = w.sentinel(rune::el(), "root", &[URI_LIST], |s: &mut S, _| s.0 += 1);
    let mut s = S(0);
    let mut tx = Tx::default();
    note_hover(&w.hovered, Hover::Accepting(DragAction::Copy));
    assert!(!w.tick(&mut s, &mut tx), "over a target");
    // A move to another target: the first hears Left, the next its hover, in one update.
    note_hover(&w.hovered, Hover::Left);
    note_hover(&w.hovered, Hover::Rejecting);
    assert!(!w.tick(&mut s, &mut tx));
    note_hover(&w.hovered, Hover::Left);
    assert!(w.tick(&mut s, &mut tx), "left everything");
    assert_eq!(s.0, 1);
    assert!(!w.tick(&mut s, &mut tx), "once");
}
