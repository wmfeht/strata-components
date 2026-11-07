use super::*;

fn ladder() -> Ladder {
    Ladder::new(vec![32.0, 48.0, 64.0, 96.0], 64.0).expect("valid")
}

#[test]
fn a_step_moves_along_the_ladder_and_stops_at_its_ends() {
    let l = ladder();
    assert_eq!(l.step(64.0, 1), 96.0);
    assert_eq!(l.step(64.0, -1), 48.0);
    assert_eq!(l.step(48.0, 2), 96.0);
    assert_eq!(l.step(96.0, 1), 96.0);
    assert_eq!(l.step(32.0, -3), 32.0);
}

#[test]
fn a_step_from_between_steps_lands_on_the_next_one_in_its_direction() {
    let l = ladder();
    assert_eq!(l.step(50.0, 1), 64.0);
    assert_eq!(l.step(50.0, -1), 48.0);
    assert_eq!(l.step(20.0, 1), 32.0);
    assert_eq!(l.step(300.0, -1), 96.0);
    assert_eq!(l.step(50.0, 0), 50.0);
}

#[test]
fn a_ladder_must_increase_and_hold_its_default() {
    assert!(Ladder::new(vec![], 0.0).is_none());
    assert!(Ladder::new(vec![32.0, 32.0], 32.0).is_none());
    assert!(Ladder::new(vec![64.0, 32.0], 32.0).is_none());
    assert!(Ladder::new(vec![32.0, 64.0], 48.0).is_none());
    assert!(Ladder::new(vec![32.0, f32::NAN], 32.0).is_none());
}

#[test]
fn a_tile_takes_the_smallest_thumbnail_class_that_covers_it() {
    let t = ThumbnailSteps::new(vec![128.0, 256.0]).expect("valid");
    assert_eq!(t.for_size(32.0), 0);
    assert_eq!(t.for_size(128.0), 0);
    assert_eq!(t.for_size(129.0), 1);
    assert_eq!(t.for_size(1024.0), 1);
    assert!(ThumbnailSteps::new(vec![]).is_none());
    assert!(ThumbnailSteps::new(vec![0.0, 128.0]).is_none());
}

#[test]
fn an_unknown_icon_key_has_no_icon() {
    let look = Look::fallback(false);
    assert!(look.icon_named("no-such-icon").is_none());
    assert!(std::ptr::eq(
        look.icon_named(IconName::Folder.id())
            .expect("in the catalog"),
        look.icon(IconName::Folder)
    ));
}

#[test]
fn the_fallback_follows_the_appearance() {
    for dark in [false, true] {
        let look = Look::fallback(dark);
        assert_eq!(look.kit.dark, dark);
        assert_eq!(fallback(dark).c.bg_base, look.kit.c.bg_base);
    }
    assert_ne!(
        Look::fallback(true).kit.c.bg_base,
        Look::fallback(false).kit.c.bg_base
    );
}
