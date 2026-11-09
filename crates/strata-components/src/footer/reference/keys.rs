//! The keys inside the reference's panel.
//!
//! Rune hands a focused text field's keys to its `on_input_key`, then its editor, then the
//! window's `on_key`, never to the field's ancestors (probe P1). Keys on other focused elements
//! reach the panel content's `on_key` before focus navigation. So the search field routes its
//! own keys, and the content routes the categories' and the results'. Both take every key the
//! panel doesn't use (bar Tab and Escape, which Rune's modal handles), so no browsing shortcut
//! runs under the reference.

use std::rc::Rc;

use rune::{Chord, Key, KeyEvent, Mods, Tx};

use super::search::Lines;
use super::state::{ReferenceKeys, ReferenceState};

/// What the key handlers need: the state's accessor, the keys, the element keys and the lines
/// on screen.
pub(super) struct Route<S> {
    pub get_mut: fn(&mut S) -> &mut ReferenceState,
    pub keys: ReferenceKeys,
    pub search: String,
    pub categories: String,
    pub results: String,
    pub line_key: Rc<dyn Fn(usize) -> String>,
    pub lines: Rc<Lines>,
}

/// Where focus is in the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Area {
    Search,
    Categories,
    Results,
    Other,
}

fn any(chords: &[Chord], ev: &KeyEvent, tx: &Tx) -> bool {
    chords.iter().any(|c| c.matches_tx(ev, tx))
}

fn plain(m: Mods) -> bool {
    !(m.shift || m.cmd || m.ctrl || m.alt || m.logo)
}

/// Sends `key` through the window's routing on the next frame: the focus moves Rune has no call
/// for at the pinned rev (upstream #173).
fn send(tx: &mut Tx, key: Key, shift: bool) {
    tx.send_key(KeyEvent {
        key,
        mods: Mods {
            shift,
            ..Mods::default()
        },
    });
}

impl<S: 'static> Route<S> {
    fn area(&self, tx: &Tx) -> Area {
        match tx.focused() {
            Some(f) if f == self.search => Area::Search,
            Some(f) if f == self.results => Area::Results,
            Some(f) if f.starts_with(&self.categories) => Area::Categories,
            _ => Area::Other,
        }
    }

    /// Moves focus to `to` from `from` around the panel's three stops (search, the chosen
    /// category, the results), one Tab or Shift+Tab away whichever it is.
    fn focus(&self, from: Area, to: Area, tx: &mut Tx) {
        use Area::*;
        let forward = matches!(
            (from, to),
            (Search, Categories) | (Categories, Results) | (Results, Search)
        );
        let back = matches!(
            (from, to),
            (Search, Results) | (Categories, Search) | (Results, Categories)
        );
        if forward {
            send(tx, Key::Tab, false);
        } else if back {
            send(tx, Key::Tab, true);
        }
    }

    /// The focus keys (Ctrl+B, Ctrl+F, Ctrl+L): true when `ev` is one.
    fn area_key(&self, from: Area, ev: &KeyEvent, tx: &mut Tx) -> bool {
        let to = if any(&self.keys.categories, ev, tx) {
            Area::Categories
        } else if any(&self.keys.search, ev, tx) {
            Area::Search
        } else if any(&self.keys.list, ev, tx) {
            Area::Results
        } else {
            return false;
        };
        self.focus(from, to, tx);
        true
    }

    /// Scrolls the results for Up, Down, Page Up, Page Down, Home and End (and `j` and `k`
    /// outside the field): true when `ev` is one of them. A line step reveals the next line out
    /// of view (Rune has no line scroll: draft U-C19-1).
    fn scroll(&self, s: &mut S, ev: &KeyEvent, tx: &mut Tx, letters: bool, ends: bool) -> bool {
        if !plain(ev.mods) {
            return false;
        }
        let visible = (self.get_mut)(s).visible.clone();
        let n = self.lines.lines.len();
        let typed = |c: &str| letters && matches!(&ev.key, Key::Text(t) if t == c);
        if ev.key == Key::Down || typed("j") {
            if visible.end < n {
                tx.scroll_into_view((self.line_key)(visible.end));
            }
        } else if ev.key == Key::Up || typed("k") {
            if visible.start > 0 {
                tx.scroll_into_view((self.line_key)(visible.start - 1));
            }
        } else if ev.key == Key::PageDown {
            tx.scroll_page(self.results.clone(), 1);
        } else if ev.key == Key::PageUp {
            tx.scroll_page(self.results.clone(), -1);
        } else if ends && ev.key == Key::Home {
            tx.scroll_to_instant(self.results.clone(), 0.0, 0.0);
        } else if ends && ev.key == Key::End {
            tx.scroll_to_bottom_instant(self.results.clone());
        } else {
            return false;
        }
        true
    }

    /// The search field's keys, before its editor. False lets the editor (or Rune's focus
    /// navigation and modal) have the key; true takes it.
    pub fn field_key(&self, s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
        // `~` typed closes; a key that reads as `~` only through the layout types itself.
        if self.keys.toggles_typed(ev) {
            (self.get_mut)(s).close();
            return true;
        }
        if self.area_key(Area::Search, ev, tx) {
            return true;
        }
        if self.scroll(s, ev, tx, false, false) {
            return true;
        }
        !edits(ev)
    }

    /// The categories' and the results' keys, before focus navigation.
    pub fn panel_key(&self, s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if self.keys.toggles(ev, tx) {
            (self.get_mut)(s).close();
            return true;
        }
        let area = self.area(tx);
        if self.area_key(area, ev, tx) {
            return true;
        }
        if passes(ev) {
            return false;
        }
        match area {
            Area::Categories => {
                if plain(ev.mods) {
                    let arrow = match &ev.key {
                        Key::Text(t) if t == "h" => Some(Key::Left),
                        Key::Text(t) if t == "j" => Some(Key::Down),
                        Key::Text(t) if t == "k" => Some(Key::Up),
                        Key::Text(t) if t == "l" => Some(Key::Right),
                        _ => None,
                    };
                    if let Some(arrow) = arrow {
                        send(tx, arrow, false);
                        return true;
                    }
                    // The focus group's arrows, Home and End; Space and Enter press.
                    let group = matches!(
                        ev.key,
                        Key::Up | Key::Down | Key::Left | Key::Right | Key::Home | Key::End
                    ) || ev.key == Key::Enter
                        || matches!(&ev.key, Key::Text(t) if t == " ");
                    if group {
                        return false;
                    }
                }
                true
            }
            Area::Results => {
                self.scroll(s, ev, tx, true, true);
                true
            }
            Area::Search | Area::Other => true,
        }
    }
}

/// Keys Rune's modal and focus navigation handle: Tab, Shift+Tab and Escape.
fn passes(ev: &KeyEvent) -> bool {
    let m = ev.mods;
    let bare = !(m.cmd || m.ctrl || m.alt || m.logo);
    bare && matches!(ev.key, Key::Tab | Key::Escape)
}

/// Whether the search field's editor edits with `ev`: typing (AltGr's characters included), the
/// copy, cut, undo, redo and select-all chords, and the caret and word keys. Everything else, an
/// unused Ctrl, Alt or Super chord among them, is taken before it reaches the window
/// (`app/shortcut-reference`: "…reaches nothing else: Ctrl+, opens no Settings"). Ctrl+V is
/// taken too: Rune reads the clipboard into the focused field before it routes the key
/// (`window/events.rs`, `runtime/input/physical.rs` at the pin), so a Ctrl+V that reaches the
/// field is one no paste followed, and the editor has no use for it.
pub(super) fn edits(ev: &KeyEvent) -> bool {
    let m = ev.mods;
    if passes(ev) {
        return true;
    }
    match &ev.key {
        Key::Text(_) => !(m.cmd || m.ctrl || m.logo),
        Key::Command(c) => !m.alt && !m.logo && "acxzy".contains(*c),
        Key::Left | Key::Right | Key::Home | Key::End | Key::Backspace | Key::Delete => {
            !m.alt && !m.logo
        }
        _ => false,
    }
}
