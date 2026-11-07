use rune::{Key, KeyEvent, Mods, TextEdit, Tx};

use super::keys::{Route, route, tab_indices};
use super::limit::trim_insertion;
use super::text::korean_word_ranges;
use super::{Measure, fit, keep_words_whole, newline_on_enter, remaining, trim_to_fit};

fn key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        mods: Mods::default(),
    }
}

fn footer() -> Vec<String> {
    ["d-skip", "d-secondary", "d-primary"]
        .map(String::from)
        .to_vec()
}

#[test]
fn enter_presses_the_focused_control_and_leaves_the_rest_to_the_default_button() {
    let f = footer();
    let enter = key(Key::Enter);
    assert_eq!(
        route(&enter, false, Some("d-secondary"), "d-dialog", &f),
        Route::Press,
        "Enter on Cancel presses Cancel"
    );
    assert_eq!(
        route(&enter, false, Some("remember"), "d-dialog", &f),
        Route::Press,
        "Enter on a body checkbox toggles it"
    );
    assert_eq!(route(&enter, false, None, "d-dialog", &f), Route::Pass);
    assert_eq!(
        route(&enter, false, Some("d-dialog"), "d-dialog", &f),
        Route::Pass
    );
    assert_eq!(
        route(&enter, true, Some("d-primary"), "d-dialog", &f),
        Route::Consume,
        "a held Return presses once"
    );
    let shifted = KeyEvent {
        key: Key::Enter,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    };
    assert_eq!(
        route(&shifted, false, Some("d-secondary"), "d-dialog", &f),
        Route::Pass
    );
}

#[test]
fn left_and_right_move_along_the_footer_and_stop_at_its_ends() {
    let f = footer();
    let at = |k: Key, focused: &str| route(&key(k), false, Some(focused), "d-dialog", &f);
    assert_eq!(at(Key::Left, "d-skip"), Route::Consume);
    assert_eq!(at(Key::Right, "d-skip"), Route::Forward);
    assert_eq!(at(Key::Left, "d-primary"), Route::Back);
    assert_eq!(at(Key::Right, "d-primary"), Route::Consume);
    assert_eq!(
        at(Key::Left, "name-field"),
        Route::Pass,
        "arrows off the footer stay Rune's"
    );
}

#[test]
fn the_initial_button_starts_tab_order_and_the_cycle_keeps_document_order() {
    // Document order: body controls, footer 0..n, close. Rune visits positive indices first,
    // ascending, then the rest in document order.
    for (initial, n) in [
        (Some(0), 3),
        (Some(1), 3),
        (Some(2), 3),
        (None, 3),
        (Some(0), 1),
    ] {
        let (footer, close) = tab_indices(initial, n);
        let mut doc: Vec<(String, i32)> = vec![("body".into(), 0)];
        doc.extend(
            footer
                .iter()
                .enumerate()
                .map(|(i, &t)| (format!("f{i}"), t)),
        );
        doc.push(("close".into(), close));
        let mut order: Vec<(usize, &(String, i32))> = doc.iter().enumerate().collect();
        order.sort_by_key(|(i, (_, t))| (if *t > 0 { *t } else { i32::MAX }, *i));
        let names: Vec<&str> = order.iter().map(|(_, (n, _))| n.as_str()).collect();
        let start = initial.map_or("body".to_owned(), |i| format!("f{i}"));
        assert_eq!(names[0], start, "{initial:?} of {n} opens there");
        let doc_names: Vec<&str> = doc.iter().map(|(n, _)| n.as_str()).collect();
        let at = doc_names
            .iter()
            .position(|n| *n == start)
            .expect("start is in the dialog");
        let rotated: Vec<&str> = doc_names[at..]
            .iter()
            .chain(&doc_names[..at])
            .copied()
            .collect();
        assert_eq!(
            names, rotated,
            "{initial:?} of {n}: the cycle is document order's"
        );
    }
}

#[test]
fn return_in_a_multiline_field_types_a_newline() {
    struct S {
        notes: TextEdit,
    }
    let handler = newline_on_enter(|s: &mut S| &mut s.notes);
    let mut s = S {
        notes: TextEdit::new("a"),
    };
    s.notes.set_caret(1);
    let mut tx = Tx::default();
    assert!(handler(&mut s, &key(Key::Enter), &mut tx));
    assert_eq!(s.notes.text, "a\n");
    assert!(!handler(&mut s, &key(Key::Tab), &mut tx));
}

#[test]
fn a_limit_counts_in_its_unit_and_cuts_on_a_whole_character() {
    assert_eq!(remaining("ÉTÉ-2026", 11, Measure::Bytes), 1);
    assert_eq!(remaining("ÉTÉ-2026", 11, Measure::Chars), 3);
    assert_eq!(remaining("𝄞a", 4, Measure::Utf16), 1);
    assert_eq!(fit("aÉb", 2, Measure::Bytes), "a", "É doesn't split");
    assert_eq!(fit("aÉb", 3, Measure::Bytes), "aÉ");
    assert_eq!(fit("𝄞𝄞", 3, Measure::Utf16), "𝄞");
    assert_eq!(fit("abc", 9, Measure::Chars), "abc");
}

#[test]
fn an_insertion_keeps_its_longest_leading_part_that_fits() {
    // "ab|cd" with "ÉÉÉ" pasted at the caret, under a 7-byte limit: one É fits.
    let mut e = TextEdit::new("abÉÉÉcd");
    e.set_caret(8);
    trim_insertion(&mut e, 7, Measure::Bytes);
    assert_eq!(e.text, "abÉcd");

    let mut e = TextEdit::new("abcde");
    e.set_caret(5);
    trim_insertion(&mut e, 3, Measure::Chars);
    assert_eq!(
        e.text, "abc",
        "a limit lowered below the text cuts it to fit"
    );

    let mut e = TextEdit::new("ÉTÉ-2026ab");
    trim_to_fit(&mut e, 11, Measure::Bytes);
    assert_eq!(
        e.text, "ÉTÉ-2026a",
        "switching the unit to bytes cuts the tail"
    );

    let mut e = TextEdit::new("hi");
    e.set_caret(2);
    trim_insertion(&mut e, 5, Measure::Chars);
    assert_eq!(e.text, "hi");
}

#[test]
fn short_korean_words_are_kept_whole_and_long_runs_stay_breakable() {
    let text = "다음 중 하나를 설치하세요: readonly에 a b";
    let words: Vec<&str> = korean_word_ranges(text)
        .into_iter()
        .map(|r| &text[r])
        .collect();
    assert_eq!(words, ["다음", "하나를", "설치하세요:", "readonly에"]);
    assert!(korean_word_ranges(&"가나다".repeat(5)).is_empty());
    assert!(korean_word_ranges("Cannot create “readonly”").is_empty());

    let long = "переноса_строк.txt을(를) /home/user/fx/readonly에서: archive.tar.gz.";
    let words: Vec<&str> = korean_word_ranges(long)
        .into_iter()
        .map(|r| &long[r])
        .collect();
    assert_eq!(words, ["t을(를)", "y에서:"]);

    let joined = keep_words_whole("다음 중 하나를");
    assert_eq!(joined, "다\u{2060}음 중 하\u{2060}나\u{2060}를");
    assert_eq!(joined.replace('\u{2060}', ""), "다음 중 하나를");
    assert_eq!(keep_words_whole("Delete 3 items?"), "Delete 3 items?");
}
