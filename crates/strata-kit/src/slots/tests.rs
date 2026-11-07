use crate::look::Look;
use crate::tokens::{self, Value};

/// Each seeded slot token, with the core token it inherits from.
const SEEDS: &[(&str, &str)] = &[
    ("list.row.height", "rows.list"),
    ("sidebar.row.height", "rows.sidebar"),
    ("tabs.tab.height", "frame.bar"),
];

fn length(look: &Look, name: &str) -> f32 {
    match tokens::get(look, name) {
        Ok(Value::Length(x)) => x,
        other => panic!("{name} read as {other:?}"),
    }
}

#[test]
fn an_unset_slot_token_follows_its_source() {
    for &(slot, source) in SEEDS {
        let mut look = Look::fallback(false);
        let before = length(&look, slot);
        let raised = length(&look, source) + 5.0;
        tokens::set(&mut look, source, Value::Length(raised)).expect("set");
        assert_eq!(length(&look, slot), before + 5.0, "{slot} follows {source}");
    }
}

#[test]
fn a_set_slot_token_stops_following_its_source() {
    for &(slot, source) in SEEDS {
        let mut look = Look::fallback(false);
        tokens::set(&mut look, slot, Value::Length(50.0)).expect("set");
        tokens::set(&mut look, source, Value::Length(10.0)).expect("set");
        assert_eq!(length(&look, slot), 50.0, "{slot} keeps its own value");
    }
}

#[test]
fn the_tab_strip_grows_with_frame_density() {
    let mut look = Look::fallback(false);
    let compact = length(&look, "tabs.tab.height");
    tokens::set(&mut look, "frame.inset", Value::Length(4.0)).expect("set");
    assert_eq!(length(&look, "tabs.tab.height"), compact + 8.0);
}
