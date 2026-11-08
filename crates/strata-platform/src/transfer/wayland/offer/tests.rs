use super::*;

fn offered() -> Vec<String> {
    vec!["text/uri-list".into(), "text/plain".into()]
}

#[test]
fn finish_before_a_drop_or_action_is_refused() {
    let mut s = DropState::new(3);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    assert!(s.finish().is_err(), "no drop yet");
    s.on_drop();
    assert!(s.finish().is_err(), "no action yet");
    s.on_action(0);
    assert!(s.finish().is_err(), "the action is none");
    s.on_action(Action::Ask.bits());
    assert!(s.finish().is_err(), "ask isn't settled");
    s.on_action(Action::Copy.bits());
    assert_eq!(s.finish(), Ok(true));
}

#[test]
fn finish_after_accepting_nothing_is_refused() {
    let mut s = DropState::new(3);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    s.accept(None, &offered()).expect("accept nothing");
    s.on_drop();
    s.on_action(Action::Copy.bits());
    assert!(s.finish().is_err());
}

#[test]
fn an_action_outside_the_mask_is_refused() {
    let mut s = DropState::new(3);
    let (copy, mv) = (Action::Copy.bits(), Action::Move.bits());
    assert!(s.set_actions(copy | mv, copy).is_ok());
    assert!(s.set_actions(copy, 0).is_ok(), "no preference is allowed");
    assert!(
        s.set_actions(copy, mv).is_err(),
        "preferred not in the mask"
    );
    assert!(
        s.set_actions(copy | mv, copy | mv).is_err(),
        "two preferred"
    );
    assert!(s.set_actions(8, 0).is_err(), "an unknown bit");
    assert!(
        s.accept(Some("image/png"), &offered()).is_err(),
        "a type not offered"
    );
    assert!(
        DropState::new(2).set_actions(copy, copy).is_err(),
        "version 2 has no actions"
    );
}

#[test]
fn nothing_after_finish() {
    let mut s = DropState::new(3);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    s.on_drop();
    s.on_action(Action::Move.bits());
    assert_eq!(s.finish(), Ok(true));
    assert!(s.finish().is_err());
    assert!(s.accept(Some("text/plain"), &offered()).is_err());
    assert!(s.set_actions(1, 1).is_err());
}

#[test]
fn an_old_offer_is_destroyed_without_finish() {
    let mut s = DropState::new(1);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    s.on_drop();
    assert_eq!(s.finish(), Ok(false));
}
