//! StrataDialog: the modal shell every Strata dialog builds on, with its backdrop, motion,
//! dismissal, initial focus, and Enter and Escape (C15, `app/dialogs`).
//!
//! Work package: C15, `docs/work/phase-1-leaves/C15-strata-dialog.md`.
//!
//! Two builders:
//!
//! - [`StrataDialog`] is an action dialog: a header with an icon, a title, a subtitle, an
//!   optional spinner and a close button; a body; and a footer of buttons, the primary last.
//!   Every action dialog uses it: delete confirmations, conflicts, Connect to server,
//!   passphrases, Format, Open With, the update dialog, C26's mount prompts.
//! - [`DialogShell`] is what both share: the blurred backdrop, open and close motion,
//!   dismissal, the size rules and the Enter routing, around any panel. The search palette
//!   (C17), the shortcut reference (C19) and Settings draw their own panel in it.
//!
//! Both push into the overlay layer `rune_kit::root(base, layer)` mounts, as rune-kit's
//! overlays do, and take the look as `&Look` at build time. Handlers are `Fn(&mut S)`
//! closures; the component holds no state of its own. Strata's policy (which form counts as
//! dirty, where focus goes after a dialog that changes the listing) stays with the caller.
//!
//! # Why not rune-kit's `Dialog`
//!
//! The work package says to wrap `rune_kit::Dialog`. It builds on Rune's primitives instead
//! (`El::modal`, `El::backdrop_blur`, presence `Transition`s, `El::default_button`,
//! `El::tab_index`), because rune-kit's `Dialog` at the pinned rev:
//!
//! - has a fixed 368 pt height, or `fit()`, which doesn't scroll, so a long dialog can't stay
//!   inside the window;
//! - hardcodes its radius, padding and title type instead of reading the `Look`;
//! - has no footer slot, no icon, no danger tone and no spinner;
//! - builds its scrim internally, so it can't take `backdrop_blur`;
//! - has no scale-and-fade motion and no switch for the animations setting;
//! - puts its close button first in Tab order, which makes the close button, not the first
//!   field, the dialog's initial focus.
//!
//! # Keyboard
//!
//! - **Escape** calls `on_dismiss`, from anywhere in the dialog, blocking or not. Rune's modal
//!   handles it. A blocking dialog's caller decides what dismissal means (C16 docks its
//!   progress dialog).
//! - **Enter** on a focused button, checkbox, switch or other non-text control presses that
//!   control, so Enter with Cancel focused cancels (Strata's rule, which differs from AppKit's,
//!   where Return always goes to the default button). The panel's key handler turns the Enter
//!   into a Space press, which Rune applies on the next frame.
//! - **Enter** in a single-line field, or with focus on nothing, presses the primary button
//!   (Rune's `default_button`). A disabled primary drops its handler, so Enter does nothing.
//! - **Enter** in a multi-line field types a newline only with [`newline_on_enter`]: Rune's
//!   multi-line editor treats a plain Return in a paragraph as submit (a chat composer's
//!   rule), so give every `TextArea` in a dialog `.on_key(newline_on_enter(access))`.
//! - **Left and Right** on a footer button move along the footer and stop at its ends.
//!
//! # Focus
//!
//! - Rune's modal traps Tab, makes the background inert, and on close returns focus to the
//!   element focused before the dialog opened, nested dialogs in stack order. A dialog that
//!   opens as another closes keeps the first one's opener. Focus that something else took
//!   while the dialog closed stays where it is.
//! - Initial focus is the first Tab stop in the dialog: the first body control, so the first
//!   field. [`Focus::Primary`], [`Focus::Secondary`] or [`Focus::Action`] start on a footer
//!   button instead. Rune has no call that focuses a button from code, so the dialog gives
//!   that button, every footer button after it and the close button positive `tab_index`es.
//!   Rune visits those first, then the rest in document order, which keeps Tab's cycle the
//!   same, only starting elsewhere. A form starts on another field by setting its
//!   `TextEdit::focused` when it opens.
//! - The close button comes last in document order and is pinned to the header's corner, so
//!   that the first field, not the close button, is the first Tab stop. The trade-off: a
//!   screen reader reaches the close button last.
//! - [`StrataDialog::return_focus`] names where focus goes on close instead of the opener:
//!   Strata uses it for dialogs that change the listing.
//!
//! # Known gaps
//!
//! - A chained dialog opened by a keyboard close shows its default button's focus ring
//!   (`app/dialogs`: "Closing a dialog by keyboard as a dialog chained on it opens…", the
//!   no-ring half, unverified in Strata). Rune has no way to clear keyboard focus mode.
//! - Rune doesn't clamp a long frame's step in presence motion to 33 ms (`app/dialogs`: "A
//!   frame longer than 33 ms…", unverified in Strata).
//! - Up and Down don't move focus between body controls by position (`app/dialogs`: "With
//!   focus outside a text field, an unmodified arrow key…", unverified). Left and Right move
//!   along the footer, and Down opens a focused select, which is Rune's own.
//! - A click on the backdrop during the close fade reaches whatever is under it: the exiting
//!   dialog is inert in Rune, as Strata's insensitive GTK layer is.
//!
//! # Follow-ups
//!
//! - The constants below could become a `dialog` slot (`dialog.backdrop.blur`,
//!   `dialog.panel.radius`, …) in strata-kit, in a W2 addition of its own.

mod action;
mod keys;
mod limit;
mod shell;
mod text;

#[cfg(test)]
mod tests;

pub use action::{Focus, StrataDialog, Tone};
pub use keys::newline_on_enter;
pub use limit::{Measure, fit, limit_on_change, remaining, trim_to_fit};
pub use shell::DialogShell;
pub use text::keep_words_whole;

/// The backdrop's key, shared by every dialog: the bottom dialog of a stack draws it, so a
/// dialog that opens as another closes keeps the same backdrop, and the blur doesn't dip.
pub const BACKDROP_KEY: &str = "strata-dialog-backdrop";

/// The backdrop's blur, a Gaussian sigma in points. Strata blurs the window content with GTK's
/// `push_blur(3.5)` (`src/ui/blur.rs`), whose radius is taken here as twice the sigma; check
/// against a Strata screenshot (open item).
pub const BACKDROP_BLUR: f32 = 1.75;

/// The scale an action dialog opens from and closes to (Strata's `.modal-hidden` transform,
/// `src/style.css`).
pub const ENTRANCE_SCALE: f32 = 0.96;

/// How long opening and closing take, in seconds (Strata's 200 ms transitions,
/// `src/style.css` and `src/ui/modal.rs`).
pub const MOTION_SECONDS: f32 = 0.2;

/// An action dialog's default minimum width, in points (`.action-dialog`, `src/style.css`).
/// Strata's other widths: compact 440, wide 520, message dialogs 560.
pub const WIDTH: f32 = 480.0;

/// The width of a message dialog, whose text wraps (`message_dialog_layout`,
/// `src/ui/controls.rs`).
pub const MESSAGE_WIDTH: f32 = 560.0;

/// The panel's corner radius before roundness (`.action-dialog`'s 9 px, `src/style.css`).
pub const PANEL_RADIUS: f32 = 9.0;

/// The header icon box's side and corner radius before roundness (`.action-dialog-symbol`).
pub const ICON_BOX: f32 = 40.0;
pub const ICON_BOX_RADIUS: f32 = 6.0;

/// A keyboard-focused footer button's ring: 3 pt wide, flush against its border, in the
/// accent (or danger) colour at 28 % (`app/dialogs`, lgse/strata#2049).
pub const FOCUS_RING_WIDTH: f32 = 3.0;
pub const FOCUS_RING_ALPHA: f32 = 0.28;

/// The longest Hangul word, in characters, that [`keep_words_whole`] keeps on one line
/// (`app/dialogs`: "Message dialog text keeps Korean words of up to 12 characters whole").
pub const KOREAN_WORD_MAX: usize = 12;
