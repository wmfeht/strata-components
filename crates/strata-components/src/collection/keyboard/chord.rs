//! Matching a [`Chord`] against a key press by layout, without a `Tx`.
//!
//! **Rune workaround (draft U-C1c-1).** Rune follows the layout in `Chord::matches_tx`, which
//! reads `tx.key_base()` and `tx.physical_key()` from a `Tx`. The keyboard machine has no `Tx`:
//! C1e's seam hands it the key, its base character and its physical key as data, so the
//! machine stays testable without a runtime. The matcher that takes those parts
//! (`Chord::matches_with`) and the US key table (`shortcuts::us_char`) are private at the pinned
//! rev (`crates/rune/src/input/chord.rs`, `crates/rune/src/shortcuts/keys.rs`). The draft asks
//! for a public `Chord::matches_parts(event, base, physical, platform)`; once it exists, this
//! module goes.
//!
//! Until then, a key that types ASCII goes to `Chord::matches_on` as it is: a letter follows
//! the key's label under the active layout (Dvorak's `j` is the key labelled J). A key that
//! types anything else (Cyrillic, Greek, an accented letter) is read as the ASCII character it
//! types with no modifiers (`key_base`), else as the US-QWERTY character of its physical key,
//! the fallback Strata applies to non-Latin layouts (`docs/keyboard-shortcuts.md` at
//! `upstream/main` 35ee677ab).
//!
//! **Deviation from Strata:** Strata first tries the Latin character the same key types in
//! another configured layout, and uses US-QWERTY only when no configured layout has one. Rune
//! can't list the configured layouts, so the US fallback applies at once.

use rune::{Chord, Key, KeyEvent, KeyPlatform, PhysicalKeyCode};

use super::machine::KeyPress;

/// Whether `press` is `chord` on `platform`, following the layout as the module docs say.
pub(super) fn matches(chord: &Chord, press: &KeyPress<'_>, platform: KeyPlatform) -> bool {
    match fallback(press) {
        Some(event) => chord.matches_on(&event, platform),
        None => chord.matches_on(press.event, platform),
    }
}

/// The press read as an ASCII character, when it typed a character that isn't ASCII and one
/// can stand in for it.
fn fallback(press: &KeyPress<'_>) -> Option<KeyEvent> {
    let Key::Text(text) = &press.event.key else {
        return None;
    };
    let mut chars = text.chars();
    let (Some(typed), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if typed.is_ascii() {
        return None;
    }
    let ascii = press
        .base
        .filter(char::is_ascii)
        .or_else(|| press.physical.and_then(us_char))?;
    Some(KeyEvent {
        key: Key::Text(ascii.to_string()),
        mods: press.event.mods,
    })
}

/// The character `key` types unshifted on a US-QWERTY layout, for the keys a chord can name by
/// character. A copy of Rune's private `shortcuts::us_char` (see the module docs).
pub(super) fn us_char(key: PhysicalKeyCode) -> Option<char> {
    use PhysicalKeyCode::*;
    const LETTERS: [PhysicalKeyCode; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO,
        KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [PhysicalKeyCode; 10] = [
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    ];
    if let Some(i) = LETTERS.iter().position(|&k| k == key) {
        return char::from_u32(u32::from(b'a') + i as u32);
    }
    if let Some(i) = DIGITS.iter().position(|&k| k == key) {
        return char::from_digit(i as u32, 10);
    }
    Some(match key {
        Minus => '-',
        Equal => '=',
        Comma => ',',
        Period => '.',
        Slash => '/',
        Semicolon => ';',
        Quote => '\'',
        BracketLeft => '[',
        BracketRight => ']',
        Backslash => '\\',
        Backquote => '`',
        _ => return None,
    })
}
