use super::Echoes;

#[test]
fn a_write_with_no_selection_event_before_its_sync_was_dropped() {
    let mut e = Echoes::default();
    e.arm(7);
    assert!(e.done(7));
}

#[test]
fn a_write_echoed_before_its_sync_took() {
    let mut e = Echoes::default();
    e.arm(7);
    e.selection();
    assert!(!e.done(7));
}

#[test]
fn a_selection_event_from_before_a_write_doesnt_count_for_it() {
    let mut e = Echoes::default();
    e.arm(1);
    e.selection();
    e.arm(2);
    assert!(!e.done(1));
    assert!(e.done(2));
}

#[test]
fn a_sync_for_a_write_never_armed_says_nothing() {
    let mut e = Echoes::default();
    assert!(!e.done(3));
    e.arm(3);
    assert!(e.done(3));
    assert!(!e.done(3), "each sync answers once");
}
