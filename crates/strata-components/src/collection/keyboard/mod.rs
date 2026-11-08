//! Keyboard navigation over the collection (C1c).
//!
//! [`KeyMachine`] turns keys on a focused collection into selection operations on its
//! [`Selection`] and [`Outcome`]s: scroll the cursor into view, open an entry, the cursor
//! reached the collection's edge, nothing to select. Every move goes through
//! [`CollectionGeometry::neighbor`] and [`CollectionGeometry::page_step`], so the list, the icons
//! grid and each Columns column behave the same. It is plain data with no Rune elements, driven
//! through a [`Surface`]; the collection element (C1e) feeds it through its keyboard seam. It
//! follows Strata at `upstream/main` 35ee677ab: `src/app/navigation/cursor.rs` and `visual.rs`,
//! `src/ui/browser/view_selection.rs`, `src/ui/window/key_routing.rs`,
//! `src/ui/window/keyboard/items/tenxer_cursor.rs`, and the tables in `src/ui/keys/bindings/`.
//!
//! # The two key maps
//!
//! [`KeyMap::default_map`] and [`KeyMap::tenxer`]; a host rebinds through [`KeyMap::bind`].
//!
//! | Keys | Default map | 10xer map |
//! | --- | --- | --- |
//! | Arrows | [`Command::Select`] a step | [`Command::Cursor`] a step |
//! | `h` `j` `k` `l` | The arrows, with [`Letters::Arrows`] | The arrows' moves |
//! | Shift+arrows | [`Command::Extend`] a step | [`Command::Swallow`]: `v` and `V` are the range keys |
//! | Ctrl+Left, Ctrl+Right | Cursor a step | |
//! | Ctrl+Up, Ctrl+Down | Select the first, the last | Cursor the first, the last |
//! | Home, End | Select the first, the last | Cursor the first, the last; also Shift+G |
//! | Shift+Home, Shift+End | Extend to the first, the last | |
//! | Page Up, Page Down | Select a page | Cursor a page; also Ctrl+B, Ctrl+F, Ctrl+Page Up, Ctrl+Page Down |
//! | Shift+Page Up, Shift+Page Down | Extend a page | |
//! | Ctrl+U, Ctrl+D | | Cursor half a page |
//! | Space | | [`Command::ToggleAdvance`] |
//! | Ctrl+Space | [`Command::Toggle`] | Toggle |
//! | `v`, Shift+V | | [`Command::Range`] |
//! | Ctrl+A | [`Command::SelectAll`] | Select all |
//! | Ctrl+R | | [`Command::Invert`] |
//! | Escape | [`Command::Clear`] | The host's Escape ladder |
//! | Enter | [`Command::Activate`] | Activate; also `o` |
//!
//! Keys match by Rune's chords, following the layout: see [Layouts](self#layouts).
//!
//! # The rules
//!
//! - **Select** moves the cursor and selects only its entry, and the anchor follows: plain
//!   arrows, Home, End and the page keys all re-anchor (lgse/strata#2026).
//! - **Extend** selects from the anchor to the target. With nothing selected, the anchor first
//!   moves to the cursor; a step then selects the cursor's entry alone without moving, so
//!   Shift+Down with nothing selected selects one entry and the next selects two
//!   (`extend_selection`), while a page or a jump extends from the cursor at once
//!   (`extend_page_selection`). After Escape the range so starts at the cursor.
//! - **Cursor** moves the cursor and keeps the selection, and the anchor stays. During a 10xer
//!   range it walks the range instead.
//! - **With no cursor**, Down, Right, Page Down and Shift+Down start at the first entry; Up,
//!   Left, Page Up and Shift+Up at the last.
//! - **Steps** go to [`CollectionGeometry::neighbor`]. Where it has none, Left and Right cross
//!   a row end with [`KeySettings::wrap_rows`] (the icons grid, lgse/strata#2044); otherwise the
//!   machine emits [`Outcome::Edge`] and changes nothing. What an edge does (the sidebar, the
//!   header, the next column, the parent folder, nothing) is the host's and the view's, as is
//!   Keep arrows in file list. An empty collection reports an edge for every step, so Up from
//!   an empty folder can still reach the header.
//! - **Pages** move by [`CollectionGeometry::page_step`] entries, half a page by half that
//!   (at least one), clamped to the first and last entry. The page size is the geometry's: the
//!   rows that fit minus one, times the tiles per row in Icons.
//! - **Escape** clears the selection and keeps the cursor. With nothing selected it isn't
//!   taken, so the view can use it (Columns closes its deepest column).
//! - Every move emits [`Outcome::Reveal`] for the cursor's entry.
//!
//! # 10xer
//!
//! - **Ranges.** `v` starts a range at the cursor that adds to the selection as it was;
//!   `V` one that removes from it. Moves and pages walk it; the same key again, or
//!   [`KeyHandle::end_range`] (the host's Escape ladder, leaving the mode), ends it and keeps
//!   what it selected. Any other selection command ends it too, as does any change to the
//!   selection the machine didn't make (a click, a sync that dropped entries), which it notices
//!   by the selection's revision at its next key. [`KeyHandle::range`] says whether one is
//!   active, for the `VISUAL`/`UNSET` footer (C18) and Columns' mirroring (C2).
//! - **Space** adds the cursor's entry (even over a load's automatic pick), or removes it,
//!   then moves the cursor to the next entry in display order. Inside a range, it flips the
//!   cursor's entry against the range and doesn't move, and later steps of the range keep the
//!   flip ([`Selection::toggle_in_run`], Strata's `toggle_visual_cursor`).
//! - In an empty folder, Space, `v`, `V`, Ctrl+A and Ctrl+R emit [`Outcome::NothingToSelect`].
//! - Commands from outside the collection (a `g g` chord step from C18, the Escape ladder, a
//!   window-level Ctrl+A) go through [`KeyHandle::run`] and run at the machine's next
//!   [`KeyMachine::tick`].
//!
//! # Type-ahead
//!
//! With [`Letters::TypeAhead`], a plain printable key the map doesn't bind finds the next entry
//! whose name starts with what was typed, case-insensitively, wrapping past the end, and
//! selects it. A character within [`KeySettings::type_ahead_timeout`] of the last extends the
//! prefix; the same letter again steps to the next entry starting with it. Space joins an open
//! prefix only. Time is the key's own clock ([`KeyPress::now`]), so tests run on a virtual
//! clock and no timer is needed.
//!
//! **Strata has no counterpart.** Its plain letters start type-to-search, which opens the
//! filter (`type_to_search_query` in `src/ui/window/key_routing.rs`; C18), or with Type to
//! search off act as `h j k l`. So type-ahead is opt-in and off by default, as the work package
//! asks.
//!
//! # Layouts
//!
//! Keys match by `rune::Chord`, so a letter follows the key's label under the active layout,
//! and a key that types a non-Latin character falls back to the US key in its place. Rune's
//! layout matching needs a `Tx` the machine doesn't have: the `chord` module is a workaround,
//! with draft U-C1c-1 and the deviation from Strata's fallback order in its docs.
//!
//! # Decisions
//!
//! - **Return with no cursor** selects the first entry and opens it (Strata's grouped-List
//!   rule: Down, Return and Shift+Down start at the first row shown).
//! - **10xer Ctrl+Up and Ctrl+Down** move the cursor only, as the 10xer map's other jumps do.
//! - **Ctrl+Home and Ctrl+End** stay unbound.
//! - **A repeated key** (key repeat) counts as a press. Rune's scroll glide keeps its speed
//!   across requests, so a held arrow scrolls as one motion.
//! - **Plain printable bindings** apply whatever [`Letters`] says; only aliases and type-ahead
//!   follow it. The default map binds no plain printable key.
//! - **Ctrl+A in an empty collection** emits [`Outcome::NothingToSelect`] in both maps; the
//!   default map's host ignores it.
//!
//! # The shared handle
//!
//! The settings live in a [`KeyHandle`] the machine and the host share, as C1b's pointer
//! settings do, because the element keeps its controllers boxed where no accessor reaches. This
//! is a recorded deviation from the accessor rule.
//!
//! Work package: C1c, `docs/work/phase-2-composites/C1c-keyboard-navigation.md`.

#![warn(missing_docs)]

mod chord;
mod command;
mod keymap;
mod machine;
mod settings;
mod type_ahead;

#[cfg(test)]
mod tests;

pub use command::{Command, RangeKind, Target, Vertical};
pub use keymap::KeyMap;
pub use machine::{KeyMachine, KeyPress, Outcome, Surface};
pub use settings::{KeyHandle, KeySettings, Letters};

#[cfg(doc)]
use crate::{collection::selection::Selection, contract::CollectionGeometry};
