use rune::Tx;

use super::*;
use crate::contract::dnd::DropHover;

fn intents(p: Polled) -> Vec<LocationIntent> {
    p.intents
}

fn query(p: &Polled) -> CompletionQuery {
    p.intents
        .iter()
        .find_map(|i| match i {
            LocationIntent::Complete(q) => Some(q.clone()),
            _ => None,
        })
        .expect("a query")
}

fn two() -> Vec<Candidate> {
    vec![
        Candidate::new("Desktop/", "~/Desktop/"),
        Candidate::new("Documents/", "~/Documents/"),
    ]
}

// browser/navigation/location-bar: "Crumb labels longer than 32 characters are shortened in
// the middle."
#[test]
fn long_labels_shorten_in_the_middle() {
    assert_eq!(elide_middle("short", CRUMB_CHARS), "short");
    let exact = "a".repeat(CRUMB_CHARS);
    assert_eq!(elide_middle(&exact, CRUMB_CHARS), exact);
    let long = "a-rather-long-folder-name-to-shorten-in-the-middle";
    let cut = elide_middle(long, CRUMB_CHARS);
    assert_eq!(cut.chars().count(), CRUMB_CHARS);
    assert_eq!(cut, "a-rather-long-f…en-in-the-middle");
    // Characters, not bytes.
    let wide = "é".repeat(40);
    assert_eq!(
        elide_middle(&wide, CRUMB_CHARS).chars().count(),
        CRUMB_CHARS
    );
}

#[test]
fn begin_edit_selects_the_whole_path_and_asks_for_suggestions() {
    let mut s = LocationBarState::new();
    let p = s.begin_edit("~/Projects");
    assert!(s.editing());
    assert!(s.edit.focused);
    assert_eq!(s.edit.range(), (0, "~/Projects".len()));
    assert_eq!(query(&p).text, "~/Projects");
}

#[test]
fn escape_and_submit_end_the_edit_and_drop_the_listing() {
    let mut s = LocationBarState::new();
    let _ = s.begin_edit("~");
    assert_eq!(
        intents(s.cancel()),
        [LocationIntent::CancelCompletion, LocationIntent::Cancelled]
    );
    assert!(!s.editing());
    let _ = s.begin_edit("~");
    assert_eq!(
        intents(s.submit("/tmp".into())),
        [
            LocationIntent::CancelCompletion,
            LocationIntent::Submit {
                text: "/tmp".into()
            }
        ]
    );
}

#[test]
fn a_press_elsewhere_cancels_on_the_next_tick_only_after_the_entry_had_focus() {
    let mut s = LocationBarState::new();
    let mut tx = Tx::default();
    let _ = s.begin_edit("~");
    // Rune hasn't focused the field yet: nothing ends.
    s.edit.focused = false;
    assert!(intents(s.tick(0.0, &mut tx)).is_empty());
    s.edit.focused = true;
    let _ = s.tick(0.1, &mut tx);
    s.edit.focused = false;
    let out = intents(s.tick(0.2, &mut tx));
    assert!(out.contains(&LocationIntent::Cancelled), "{out:?}");
    assert!(!s.editing());
}

#[test]
fn a_late_or_stale_answer_is_dropped() {
    let mut s = LocationBarState::new();
    let first = query(&s.begin_edit("~"));
    s.edit.replace_text("~/D");
    let second = query(&s.edited());
    assert!(!s.deliver(Completions::new(first.generation, two())).changed);
    assert!(!s.suggestions_open());
    assert!(
        s.deliver(Completions::new(second.generation, two()))
            .changed
    );
    assert!(s.suggestions_open());
    // After the edit ends, an answer for it shows nothing.
    let third = query(&s.edited());
    let _ = s.cancel();
    let _ = s.deliver(Completions::new(third.generation, two()));
    assert!(!s.suggestions_open());
}

#[test]
fn navigating_keeps_an_entry_being_typed_in() {
    let mut s = LocationBarState::new();
    let _ = s.begin_edit("~");
    s.edit.replace_text("~/Mu");
    let _ = s.navigated();
    assert!(s.editing(), "the focused entry stays open");
    assert_eq!(s.edit.text, "~/Mu");
    s.edit.focused = false;
    let _ = s.navigated();
    assert!(
        !s.editing(),
        "an entry without the keyboard gives way to the crumbs"
    );
}

#[test]
fn a_resting_drag_springs_once_after_its_delay() {
    let mut s = LocationBarState::new();
    let mut tx = Tx::default();
    s.drag_over(1, "/home/demo", DropHover::Accepting, Some(SPRING_DELAY));
    assert!(
        intents(s.tick(1.0, &mut tx)).is_empty(),
        "the first tick stamps it"
    );
    assert!(intents(s.tick(1.5, &mut tx)).is_empty());
    assert_eq!(
        intents(s.tick(1.0 + SPRING_DELAY, &mut tx)),
        [LocationIntent::Open {
            id: "/home/demo".into(),
            via: Via::Spring
        }]
    );
    assert!(intents(s.tick(3.0, &mut tx)).is_empty(), "once");
    // Leaving before the delay disarms it, and so does spring-loading being off.
    s.drag_over(1, "/home/demo", DropHover::Accepting, Some(SPRING_DELAY));
    let _ = s.tick(4.0, &mut tx);
    s.drag_over(1, "/home/demo", DropHover::Left, Some(SPRING_DELAY));
    assert!(intents(s.tick(5.0, &mut tx)).is_empty());
    s.drag_over(1, "/home/demo", DropHover::Accepting, None);
    let _ = s.tick(6.0, &mut tx);
    assert!(intents(s.tick(7.0, &mut tx)).is_empty());
}

#[test]
fn the_copy_feedback_lasts_two_seconds() {
    let mut s = LocationBarState::new();
    let mut tx = Tx::default();
    s.copy_pressed();
    assert!(s.copied());
    let _ = s.tick(10.0, &mut tx);
    let _ = s.tick(10.0 + COPIED_FOR - 0.1, &mut tx);
    assert!(s.copied());
    assert!(s.tick(10.0 + COPIED_FOR, &mut tx).changed);
    assert!(!s.copied());
}

#[test]
fn a_leave_from_another_crumb_keeps_the_hover() {
    let mut s = LocationBarState::new();
    s.set_drop_hover(2, DropHover::Accepting);
    s.set_drop_hover(1, DropHover::Left);
    assert_eq!(s.drop_hover(), Some((2, DropHover::Accepting)));
    s.set_drop_hover(2, DropHover::Left);
    assert_eq!(s.drop_hover(), None);
}
