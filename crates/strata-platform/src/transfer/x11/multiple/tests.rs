use x11rb::NONE;

use super::{answered_pairs, parse_pairs};

#[test]
fn an_atom_pair_list_reads_as_pairs() {
    assert_eq!(parse_pairs(&[1, 2, 3, 4]), Some(vec![(1, 2), (3, 4)]));
    assert_eq!(parse_pairs(&[]), Some(vec![]));
}

#[test]
fn an_odd_count_is_no_atom_pair_list() {
    assert_eq!(parse_pairs(&[1, 2, 3]), None);
}

#[test]
fn a_refused_pair_gets_none_for_its_property_and_the_rest_stay_as_asked() {
    let asked = [(10, 20), (11, 21), (12, 22)];
    assert_eq!(
        answered_pairs(&asked, &[false, true, false]),
        vec![10, 20, 11, NONE, 12, 22]
    );
}
