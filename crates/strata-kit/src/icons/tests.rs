//! Shared by W2 (the semantic set) and K3 (the catalogs): every catalog keeps these.

use std::collections::BTreeSet;

use super::*;

fn sets() -> Vec<IconSet> {
    let mut sets = vec![PLACEHOLDER];
    sets.extend(catalogs());
    sets
}

#[test]
fn every_name_resolves_in_every_catalog() {
    for set in sets() {
        for &name in IconName::ALL {
            let icon = (set.icon)(name);
            assert!(
                !icon.body.is_empty() && icon.view > 0.0,
                "{}: {name:?} has an empty icon",
                set.name
            );
        }
    }
}

#[test]
fn every_icon_in_a_catalog_is_found_by_its_name() {
    for set in sets() {
        let mut names = BTreeSet::new();
        for icon in set.all {
            assert!(names.insert(icon.name), "{}: {} twice", set.name, icon.name);
            let found =
                (set.named)(icon.name).unwrap_or_else(|| panic!("{}: {}", set.name, icon.name));
            assert!(
                std::ptr::eq(found, icon),
                "{}: {} finds another icon",
                set.name,
                icon.name
            );
        }
        assert!((set.named)("no-such-icon").is_none(), "{}", set.name);
    }
}

#[test]
fn the_placeholder_draws_each_name_with_its_own_icon() {
    for &name in IconName::ALL {
        let icon = (PLACEHOLDER.icon)(name);
        assert_eq!(icon.name, name.id());
    }
    let ids: BTreeSet<_> = IconName::ALL.iter().map(|n| n.id()).collect();
    assert_eq!(ids.len(), IconName::ALL.len(), "ids are unique");
}
