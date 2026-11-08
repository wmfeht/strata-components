use strata_kit::overrides::Overrides;

use super::reload::{Debounce, SETTLE};
use super::{FILE, FILE_BROKEN};

#[test]
fn the_fake_files_apply_and_the_broken_one_reports_its_row() {
    assert!(Overrides::from_bytes(FILE.as_bytes()).errors().is_empty());
    assert_eq!(
        Overrides::from_bytes(FILE_BROKEN.as_bytes()).errors().len(),
        1
    );
}

// settings/themes: "Changes to the file less than 75 ms apart trigger one reload, 75 ms after
// the last."
#[test]
fn changes_less_than_the_settle_apart_read_once_after_the_last() {
    let mut d = Debounce::default();
    d.changed(1.0);
    d.changed(1.05);
    assert!(
        !d.take_due(1.0 + SETTLE),
        "the second change moved the read"
    );
    assert!(d.take_due(1.05 + SETTLE));
    assert!(!d.take_due(2.0), "one read per burst");

    // Changes further apart than the settle each read.
    d.changed(3.0);
    assert!(d.take_due(3.1));
    d.changed(3.2);
    assert!(d.take_due(3.3));
}
