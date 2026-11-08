use std::cell::Cell;

use super::super::api::{DragItem, DropTarget};
use super::*;

fn step(phase: Phase, x: f32, cancelled: bool) -> Step {
    Step {
        phase,
        at: [x, 10.0],
        cancelled,
    }
}

#[test]
fn a_press_starts_a_drag_past_the_threshold_and_not_before() {
    let mut g = Guard::default();
    assert_eq!(g.phase(step(Phase::Down, 10.0, false), 8.0), Decision::Pass);
    assert_eq!(
        g.phase(step(Phase::Move, 18.0, false), 8.0),
        Decision::Pass,
        "8 pt is a press"
    );
    assert_eq!(
        g.phase(step(Phase::Move, 18.5, false), 8.0),
        Decision::Start
    );
}

/// Wayland: after the start the guard swallows the moves and the cancelled Up that P1's own
/// Escape brings, and the next press is the element's again.
#[test]
fn on_wayland_the_escapes_cancelled_up_is_swallowed() {
    let mut g = Guard::default();
    g.phase(step(Phase::Down, 0.0, false), 8.0);
    assert_eq!(
        g.phase(step(Phase::Move, 20.0, false), 8.0),
        Decision::Start
    );
    g.started(Release::Escape);
    assert_eq!(g.phase(step(Phase::Move, 30.0, false), 8.0), Decision::Own);
    assert_eq!(g.phase(step(Phase::Up, 30.0, true), 8.0), Decision::Own);
    assert_eq!(
        g.phase(step(Phase::Move, 40.0, false), 8.0),
        Decision::Pass,
        "no press"
    );
    assert_eq!(g.phase(step(Phase::Down, 40.0, false), 8.0), Decision::Pass);
}

/// X11: the real release is swallowed, while a cancelled Up (Escape, focus lost) cancels the
/// drag.
#[test]
fn on_x11_a_cancelled_up_cancels_the_drag_and_a_real_one_is_swallowed() {
    let mut g = Guard::default();
    g.phase(step(Phase::Down, 0.0, false), 8.0);
    g.started(Release::RealUp);
    assert_eq!(g.phase(step(Phase::Up, 30.0, false), 8.0), Decision::Own);
    g.phase(step(Phase::Down, 0.0, false), 8.0);
    g.started(Release::RealUp);
    assert_eq!(g.phase(step(Phase::Up, 30.0, true), 8.0), Decision::Cancel);
}

struct S {
    refuse: Cell<bool>,
}

fn s() -> S {
    S {
        refuse: Cell::new(false),
    }
}

fn offer(mimes: &[&str], actions: Actions) -> DropOffer {
    DropOffer {
        mimes: mimes.iter().map(|m| (*m).to_owned()).collect(),
        actions,
        local: false,
        mods: None,
        serial: 1,
        peeked: Vec::new(),
    }
}

fn uri_target() -> DropTarget<S> {
    DropTarget::new(|_: &mut S, _, _| {})
}

const PATH: &str = "#0/pane/#0/row/#0/label";

#[test]
fn a_target_that_doesnt_take_the_drag_passes_it_out() {
    let mut r = Registry::<S>::default();
    r.put("pane", uri_target());
    r.put("row", uri_target().mime_types(&["image/png"]));
    let o = offer(&["text/uri-list"], Actions::COPY | Actions::MOVE);
    assert_eq!(
        land(&r, Some(PATH), &s(), &o, None, None),
        Landing::Accepting {
            key: "pane".into(),
            action: DragAction::Copy,
            accept: "text/uri-list".into()
        }
    );
}

#[test]
fn a_target_with_on_reject_stops_what_it_refuses() {
    let mut r = Registry::<S>::default();
    r.put("pane", uri_target());
    r.put(
        "row",
        uri_target()
            .action(|s: &S, _, _| (!s.refuse.get()).then_some(DragAction::Move))
            .on_reject(|_: &mut S, _, _| {}),
    );
    let o = offer(&["text/uri-list"], Actions::COPY | Actions::MOVE);
    let st = s();
    assert!(matches!(
        land(&r, Some(PATH), &st, &o, None, None),
        Landing::Accepting { ref key, action: DragAction::Move, .. } if key == "row"
    ));
    st.refuse.set(true);
    assert_eq!(
        land(&r, Some(PATH), &st, &o, None, None),
        Landing::Rejecting { key: "row".into() }
    );
}

#[test]
fn modifiers_choose_the_action_where_they_are_known() {
    let mut r = Registry::<S>::default();
    r.put("pane", uri_target());
    let o = offer(&["text/uri-list"], Actions::COPY | Actions::MOVE);
    let shift = Some(Mods {
        shift: true,
        ..Mods::default()
    });
    assert!(matches!(
        land(&r, Some(PATH), &s(), &o, None, shift),
        Landing::Accepting {
            action: DragAction::Move,
            ..
        }
    ));
}

#[test]
fn a_local_payload_of_the_targets_type_lands_by_its_token() {
    let mut r = Registry::<S>::default();
    r.put("pane", uri_target().mime_types::<&str>(&[]).local::<u32>());
    let item = DragItem::new().local(7u32);
    let payload = item.local.expect("payload");
    let o = offer(&[], Actions::COPY);
    assert_eq!(
        land(&r, Some(PATH), &s(), &o, Some(("tok", &payload)), None),
        Landing::Accepting {
            key: "pane".into(),
            action: DragAction::Copy,
            accept: "tok".into()
        }
    );
    // Another type of payload: nothing to read, so nowhere.
    let other: Rc<dyn Any> = Rc::new("text");
    assert_eq!(
        land(&r, Some(PATH), &s(), &o, Some(("tok", &other)), None),
        Landing::Nowhere
    );
}

#[test]
fn source_actions_unknown_yet_negotiate_as_copy() {
    let mut r = Registry::<S>::default();
    r.put("pane", uri_target());
    let o = offer(&["text/uri-list"], Actions::NONE);
    assert!(matches!(
        land(&r, Some(PATH), &s(), &o, None, None),
        Landing::Accepting {
            action: DragAction::Copy,
            ..
        }
    ));
}
