use rune::{Key, KeyEvent, Mods};

use crate::filter_bar::prompt::{
    By, Candidate, Candidates, Polled, PromptIntent, PromptSpec, PromptState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum P {
    Find,
    Filter,
    Go,
    Rename,
}

fn press(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods::default(),
    }
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(c),
        mods: Mods {
            ctrl: true,
            cmd: true,
            ..Mods::default()
        },
    }
}

fn query(p: &Polled<P>) -> u64 {
    p.intents
        .iter()
        .find_map(|i| match i {
            PromptIntent::Complete { query, .. } => Some(query.generation),
            _ => None,
        })
        .expect("a listing asked for")
}

fn folders(names: &[&str]) -> Vec<Candidate> {
    names
        .iter()
        .map(|n| Candidate::folder(*n, *n, "~", format!("~/{n}/")))
        .collect()
}

fn submit(p: &Polled<P>) -> Option<(String, Option<String>, bool, bool)> {
    p.intents.iter().find_map(|i| match i {
        PromptIntent::Submit {
            text,
            chosen,
            stepped,
            settled,
            ..
        } => Some((text.clone(), chosen.clone(), *stepped, *settled)),
        _ => None,
    })
}

// integration/10xer-mode/places: "The entry is cleared on… a replacing prompt": opening a
// prompt over another closes it, and none of its text reaches the new kind.
#[test]
fn a_new_prompt_replaces_the_open_one_without_its_text() {
    let mut st = PromptState::new();
    let _ = st.open(P::Go, PromptSpec::go(), "", None);
    st.edit.text = "smb://user:secret@host".into();
    let out = st.open(P::Filter, PromptSpec::filter(), "", None);
    assert!(out.intents.contains(&PromptIntent::CancelCompletion));
    assert!(out.intents.contains(&PromptIntent::Dismissed {
        kind: P::Go,
        text: "smb://user:secret@host".into(),
        by: By::Replaced
    }));
    assert_eq!(st.kind(), Some(P::Filter));
    assert_eq!(st.text(), "");
}

// integration/10xer-mode/search: "Pressing `f` again pre-fills the current filter query", with
// the caret after it, unselected, so typing extends it.
#[test]
fn a_prefilled_prompt_keeps_its_text_unselected() {
    let mut st = PromptState::new();
    let _ = st.open(P::Filter, PromptSpec::filter(), "rep md", None);
    assert_eq!(st.edit.range(), (6, 6));
    assert!(st.focused());
}

// integration/10xer-mode/file-verbs: "`r` and F2 open `rename ›`… A file's stem… is selected."
#[test]
fn rename_opens_with_the_range_the_host_names_selected() {
    let mut st = PromptState::new();
    let _ = st.open(
        P::Rename,
        PromptSpec::rename(),
        "report.final.pdf",
        Some(0..12),
    );
    assert_eq!(st.edit.range(), (0, 12));
}

// integration/10xer-mode/search: "In the /, ?, f, and s prompts, Up and Down move the listing
// cursor while the prompt keeps focus"; rename's Up and Down do nothing.
#[test]
fn up_and_down_step_the_listing_choose_a_candidate_or_do_nothing() {
    let mut st = PromptState::new();
    let _ = st.open(P::Find, PromptSpec::find(), "", None);
    let out = st.key(&press(Key::Down), false).expect("taken");
    assert_eq!(
        out.intents,
        vec![PromptIntent::Step {
            kind: P::Find,
            delta: 1
        }]
    );

    let out = st.open(P::Go, PromptSpec::go(), "", None);
    let g = query(&out);
    let _ = st.deliver(Candidates {
        generation: g,
        items: folders(&["a", "b", "c"]),
        done: true,
    });
    let _ = st.key(&press(Key::Up), false).expect("taken");
    assert_eq!(
        st.chosen_index(),
        Some(2),
        "Up from the first wraps to the last"
    );
    assert!(st.stepped());

    let _ = st.open(P::Rename, PromptSpec::rename(), "a", None);
    let out = st.key(&press(Key::Down), false).expect("taken");
    assert_eq!(out, Polled::default());
}

// integration/10xer-mode/places: "Up and Down choose a candidate, and results that arrive
// later keep that choice"; without a choice, the first follows the ranking.
#[test]
fn a_chosen_candidate_survives_later_batches_and_a_stale_answer_is_dropped() {
    let mut st = PromptState::new();
    let g = query(&st.open(P::Go, PromptSpec::go(), "", None));
    let _ = st.deliver(Candidates {
        generation: g,
        items: folders(&["a", "b"]),
        done: false,
    });
    assert_eq!(st.chosen().map(|c| c.id.as_str()), Some("a"));
    let _ = st.key(&press(Key::Down), false);
    let _ = st.deliver(Candidates {
        generation: g,
        items: folders(&["c", "a", "b"]),
        done: false,
    });
    assert_eq!(st.chosen().map(|c| c.id.as_str()), Some("b"));

    st.edit.text = "x".into();
    let next = query(&st.edited());
    assert!(next > g);
    assert_eq!(
        st.deliver(Candidates {
            generation: g,
            items: folders(&["stale"]),
            done: true,
        }),
        Polled::default()
    );
    assert_eq!(
        st.candidates().len(),
        3,
        "the previous list stays until the new one lands"
    );
}

// integration/10xer-mode/places: "Tab writes the chosen folder into the prompt…, and never
// acts on it." It is an edit, so a new listing follows.
#[test]
fn tab_writes_the_chosen_candidate_and_lists_again() {
    let mut st = PromptState::new();
    let g = query(&st.open(P::Go, PromptSpec::go(), "~/d", None));
    let _ = st.deliver(Candidates {
        generation: g,
        items: folders(&["dev", "docs"]),
        done: true,
    });
    let _ = st.key(&press(Key::Down), false);
    let out = st.key(&press(Key::Tab), false).expect("taken");
    assert_eq!(st.text(), "~/docs/");
    assert!(submit(&out).is_none());
    assert!(
        out.intents
            .iter()
            .any(|i| matches!(i, PromptIntent::Edited { .. }))
    );
    assert!(query(&out) > g);
}

// integration/10xer-mode/places: "Enter on a typed path that exists acts on it at once;
// otherwise Enter waits for the search to finish." The host asks the prompt to wait; an edit
// drops the waiting submit.
#[test]
fn a_submit_can_wait_for_the_listing_to_finish() {
    let mut st = PromptState::new();
    let g = query(&st.open(P::Go, PromptSpec::go(), "~/d", None));
    let out = st.key(&press(Key::Enter), false).expect("taken");
    let (_, _, _, settled) = submit(&out).expect("a submit");
    assert!(!settled);
    assert!(st.wait_until_settled(st.serial()));
    let _ = st.deliver(Candidates {
        generation: g,
        items: folders(&["dev"]),
        done: false,
    });
    let out = st.deliver(Candidates {
        generation: g,
        items: folders(&["dev", "docs"]),
        done: true,
    });
    assert_eq!(
        submit(&out),
        Some(("~/d".into(), Some("dev".into()), false, true))
    );
    assert!(!st.wait_until_settled(st.serial()), "nothing pending now");

    let g = query(&st.open(P::Go, PromptSpec::go(), "~/d", None));
    assert!(st.wait_until_settled(st.serial()));
    st.edit.text = "~/do".into();
    let g2 = query(&st.edited());
    let _ = st.deliver(Candidates {
        generation: g2,
        items: folders(&["docs"]),
        done: true,
    });
    assert!(g2 > g);
    assert!(st.text() == "~/do");
}

// integration/10xer-mode: "A `create ›` or `rename ›` name check still running when the prompt
// is dismissed or edited creates or renames nothing": its answer names an older serial.
#[test]
fn a_check_answer_after_an_edit_or_a_dismissal_is_dropped() {
    let mut st = PromptState::new();
    let _ = st.open(P::Rename, PromptSpec::rename(), "notes.txt", None);
    let asked = st.serial();
    st.edit.text = "notes2.txt".into();
    let _ = st.edited();
    assert!(!st.refuse(asked, "“notes.txt” already exists"));
    assert!(!st.invalid());
    let asked = st.serial();
    assert!(st.refuse(asked, "“notes2.txt” already exists"));
    assert!(st.invalid());
    assert_eq!(st.hint(), Some("“notes2.txt” already exists"));
    let _ = st.key(&press(Key::Escape), false);
    assert!(!st.is_open());
    assert!(!st.show_hint(asked, Some("late".into())));
}

// integration/10xer-mode/places: `go ›` "keeps no undo history".
#[test]
fn a_prompt_without_undo_swallows_the_undo_keys() {
    let mut st = PromptState::new();
    let _ = st.open(P::Go, PromptSpec::go(), "", None);
    assert_eq!(st.key(&ctrl('z'), false), Some(Polled::default()));
    assert_eq!(st.key(&ctrl('y'), false), Some(Polled::default()));
    let _ = st.open(P::Find, PromptSpec::find(), "", None);
    assert_eq!(st.key(&ctrl('z'), false), None, "the editor undoes");
}

// integration/10xer-mode/file-verbs: "Esc, a clicked row, another prompt, or leaving the mode
// discards a `create ›` or `rename ›` entry": a field that lost the keyboard closes.
#[test]
fn losing_the_keyboard_dismisses_the_prompt() {
    let mut st = PromptState::new();
    let _ = st.open(P::Rename, PromptSpec::rename(), "a", None);
    assert_eq!(st.tick(), Polled::default());
    st.edit.focused = false;
    let out = st.tick();
    assert_eq!(
        out.intents,
        vec![PromptIntent::Dismissed {
            kind: P::Rename,
            text: "a".into(),
            by: By::Blur
        }]
    );
    assert!(!st.is_open());
}

// A held Enter or Escape acts once.
#[test]
fn repeats_of_enter_and_escape_do_nothing() {
    let mut st = PromptState::new();
    let _ = st.open(P::Find, PromptSpec::find(), "x", None);
    assert_eq!(st.key(&press(Key::Enter), true), Some(Polled::default()));
    assert_eq!(st.key(&press(Key::Escape), true), Some(Polled::default()));
    assert!(st.is_open());
}
