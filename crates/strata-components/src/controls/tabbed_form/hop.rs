//! The focus hop: how a component moves the keyboard to an element from code at Rune
//! 5d421fd, which has no `Tx::focus` (upstream #173; Rune dev past the pin adds one for web
//! focus, #384, not adopted).
//!
//! A zero-size, unfocusable leaf naming the target as its `focus_return` is drawn for one
//! frame, then dropped. When a leaf like that goes and the keyboard is inside the leaf's
//! parent, Rune's `sync_requested_focus` (`runtime/input/focus.rs`) hands focus to the
//! target's first focusable element (its `focus_selected` one first). So the leaf must sit in
//! an element that holds the focused control: the search field's frame, or a dialog body
//! around a form and its buttons. Probe P1 (C20c plan): it moves the keyboard from a focused
//! core `text_input` to a button, and from a focused button to a `text_input`.
//!
//! The keyboard must still be somewhere when the leaf goes: Rune forgets the last focused
//! element once focus drops (a focused control turning `inert`, `runtime/input/focus.rs`), and
//! then moves it nowhere. So an owner that hides the focused control (a form's page) does it in
//! the commit that drops the leaf, where the target is shown and the old focus still counts.
//!
//! A hop can name fallbacks: each target gets a leaf, the first choice last among them, and
//! `sync_requested_focus` tries the dropped leaves from the last in the tree to the first, so
//! a target with nothing focusable shown hands over to the next (the form's Ctrl+Page Down: the
//! new page, else its tab).
//!
//! Used by: [`super::TabbedFormState::show_error`], [`super::TabbedForm::window_key`], C20c's
//! settings search (Enter).

use std::time::Duration;

use rune::{El, Tx, el};

/// A pending hop, held in its owner's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Hop {
    /// In order of preference; empty when no hop is pending.
    targets: Vec<String>,
    drawn: bool,
}

impl Hop {
    /// Move the keyboard to the element keyed `target` once the leaf has been drawn and
    /// dropped. A new request replaces one not yet done.
    pub(crate) fn request(&mut self, target: String, tx: &mut Tx) {
        self.request_first(vec![target], tx);
    }

    /// Move the keyboard to the first of `targets` that has a focusable element shown.
    pub(crate) fn request_first(&mut self, targets: Vec<String>, tx: &mut Tx) {
        self.targets = targets;
        self.drawn = false;
        tx.wake_after(Duration::ZERO);
    }

    /// The first target's leaf, keyed `key`, while the hop is pending.
    pub(crate) fn leaf(&self, key: &str) -> Option<El> {
        self.targets.first().map(|t| leaf(key.to_owned(), t))
    }

    /// Every target's leaf, keyed `{key}-{n}` by preference, the first choice last, for
    /// siblings in one element.
    pub(crate) fn leaves(&self, key: &str) -> Vec<El> {
        let n = self.targets.len();
        (0..n)
            .rev()
            .map(|i| leaf(format!("{key}-{i}"), &self.targets[i]))
            .collect()
    }

    /// Whether the next [`Hop::tick`] drops the leaf: the commit that moves the keyboard.
    pub(crate) fn dropping(&self) -> bool {
        !self.targets.is_empty() && self.drawn
    }

    /// From the owner's poll: the first tick after a request keeps the leaf for a frame that
    /// draws it (a request made in a poll before this tick still gets that frame); the next
    /// drops it, which moves the keyboard. True when the view changes.
    pub(crate) fn tick(&mut self, tx: &mut Tx) -> bool {
        if self.targets.is_empty() {
            return false;
        }
        if self.drawn {
            self.targets.clear();
            self.drawn = false;
        } else {
            self.drawn = true;
            tx.wake_after(Duration::ZERO);
        }
        true
    }
}

fn leaf(key: String, target: &str) -> El {
    el().key(key)
        .absolute(0.0, 0.0)
        .size(0.0, 0.0)
        .a11y_hidden()
        .focus_return(target.to_owned())
}
