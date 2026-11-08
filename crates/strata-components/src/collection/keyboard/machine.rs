//! The keyboard state machine: keys in, selection operations and [`Outcome`]s out. Plain data,
//! no Rune elements.

use rune::{Key, KeyEvent, PhysicalKeyCode};

use super::command::{Command, RangeKind, Target, Vertical};
use super::settings::{KeyHandle, Letters};
use super::type_ahead::TypeAhead;
use crate::collection::selection::{AnchorRule, Changed, Extend, Selection};
use crate::contract::{CollectionGeometry, Direction, EntryId, EntrySource};

/// One key the focused collection heard, as data.
///
/// Fields may be added: build one with [`KeyPress::new`].
///
/// Used by: the C1e adapter, tests.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct KeyPress<'a> {
    /// The key and its modifiers, as Rune reports them.
    pub event: &'a KeyEvent,
    /// The physical key behind it (`tx.physical_key()`).
    pub physical: Option<PhysicalKeyCode>,
    /// The character the key types with no modifiers under the current layout
    /// (`tx.key_base()`).
    pub base: Option<char>,
    /// Whether the key is repeating. A repeat counts as a press (see the module docs).
    pub repeat: bool,
    /// The host's clock, in seconds.
    pub now: f64,
}

impl<'a> KeyPress<'a> {
    /// `event` with no physical key or base, not repeating, at time 0. Set the other fields
    /// after.
    pub fn new(event: &'a KeyEvent) -> Self {
        Self {
            event,
            physical: None,
            base: None,
            repeat: false,
            now: 0.0,
        }
    }
}

/// What the machine asks for besides selection changes. What each means is the host's policy.
///
/// Variants may be added.
///
/// Used by: the C1e adapter, C2, C18, hosts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// Scroll the entry at this index into view: the cursor after a move.
    Reveal(usize),
    /// Open the entry (Enter, 10xer `o`).
    Activate {
        /// The entry.
        id: EntryId,
    },
    /// The cursor can't move this way inside the collection. The host decides: the sidebar or
    /// the pane header (unless Keep arrows in file list is on), the next Columns column (C2),
    /// the parent folder or the folder under the cursor (10xer List and Columns), or nothing
    /// (List's Right).
    Edge(Direction),
    /// A 10xer selection key in an empty folder: the host flashes `Nothing to select` (C18).
    NothingToSelect,
}

/// What the machine reads and changes: the source, its geometry, the selection, and where its
/// outcomes go. C1e's `Ctx` is one; tests use a plain one.
///
/// Used by: the C1e adapter.
pub trait Surface {
    /// The entries, in display order.
    fn source(&self) -> &dyn EntrySource;

    /// Where they sit, mounted or not.
    fn geometry(&self) -> &dyn CollectionGeometry;

    /// The selection, to read.
    fn selection(&self) -> &Selection;

    /// Runs one selection operation, and returns what it changed.
    fn apply(&mut self, op: &mut dyn FnMut(&mut Selection, &dyn EntrySource) -> Changed)
    -> Changed;

    /// Hands on an outcome.
    fn emit(&mut self, outcome: Outcome);
}

/// Where a target lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Landing {
    At(usize),
    Edge(Direction),
    Nowhere,
}

/// The keyboard state machine of one collection.
///
/// Feed it each key with [`KeyMachine::key`], and run [`KeyMachine::tick`] when the host has
/// queued commands on its [`KeyHandle`]. It changes the selection through the [`Surface`] and
/// emits [`Outcome`]s for the rest. The [module docs](super) give the rules.
///
/// Used by: the C1e adapter.
#[derive(Debug)]
pub struct KeyMachine {
    handle: KeyHandle,
    type_ahead: TypeAhead,
    /// The selection's revision after the machine's own last change during a 10xer range. A
    /// different revision means something else (a click) changed the selection, which ends
    /// the range, as a pointer change ends one in Strata.
    range_revision: u64,
}

impl KeyMachine {
    /// A machine reading `handle` on every key.
    pub fn new(handle: KeyHandle) -> Self {
        Self {
            handle,
            type_ahead: TypeAhead::default(),
            range_revision: 0,
        }
    }

    /// The handle it reads.
    pub fn handle(&self) -> &KeyHandle {
        &self.handle
    }

    /// Handles one key. True when the machine took it; false lets the page and Rune hear it.
    pub fn key(&mut self, press: &KeyPress<'_>, s: &mut dyn Surface) -> bool {
        let (command, letters, timeout) = self.handle.with(|settings| {
            (
                settings
                    .map
                    .lookup(press, settings.letters, settings.platform),
                settings.letters,
                settings.type_ahead_timeout,
            )
        });
        if let Some(command) = command {
            self.type_ahead.reset();
            return self.run(command, s);
        }
        if letters != Letters::TypeAhead {
            return false;
        }
        let Some(c) = printable(press.event) else {
            return false;
        };
        if c == ' ' && !self.type_ahead.open(press.now, timeout) {
            return false;
        }
        let cursor = s.selection().cursor_index(s.source());
        if let Some(i) = self
            .type_ahead
            .push(c, press.now, timeout, s.source(), cursor)
        {
            op(s, |sel, src| sel.select_only(src, i));
            s.emit(Outcome::Reveal(i));
        }
        true
    }

    /// Runs the commands the host queued on the handle ([`KeyHandle::run`]). True when one
    /// of them did something.
    pub fn tick(&mut self, s: &mut dyn Surface) -> bool {
        let mut handled = false;
        while let Some(command) = self.handle.next() {
            handled |= self.run(command, s);
        }
        handled
    }

    /// Runs `command` as a key bound to it would. True when it took the key.
    pub fn run(&mut self, command: Command, s: &mut dyn Surface) -> bool {
        if self.handle.range().is_some() && s.selection().revision() != self.range_revision {
            self.handle.set_range(None);
        }
        if !keeps_range(command) {
            self.handle.set_range(None);
        }
        let taken = self.dispatch(command, s);
        self.range_revision = s.selection().revision();
        taken
    }

    fn dispatch(&mut self, command: Command, s: &mut dyn Surface) -> bool {
        let len = s.source().len();
        match command {
            Command::Select(target) => {
                match self.land(target, s) {
                    Landing::At(i) => {
                        op(s, |sel, src| sel.select_only(src, i));
                        s.emit(Outcome::Reveal(i));
                    }
                    Landing::Edge(d) => s.emit(Outcome::Edge(d)),
                    Landing::Nowhere => {}
                }
                true
            }
            Command::Extend(target) => {
                self.extend(target, s);
                true
            }
            Command::Cursor(target) => {
                match self.land(target, s) {
                    Landing::At(i) => {
                        match self.handle.range() {
                            Some(kind) => op(s, |sel, src| sel.extend_to(src, i, kind.extend())),
                            None => op(s, |sel, src| sel.move_cursor(src, i, AnchorRule::Keep)),
                        };
                        s.emit(Outcome::Reveal(i));
                    }
                    Landing::Edge(d) => s.emit(Outcome::Edge(d)),
                    Landing::Nowhere => {}
                }
                true
            }
            Command::Toggle => {
                if len > 0 {
                    let i = cursor_or_first(s);
                    op(s, |sel, src| sel.toggle(src, i));
                    s.emit(Outcome::Reveal(i));
                }
                true
            }
            Command::ToggleAdvance => {
                if len == 0 {
                    s.emit(Outcome::NothingToSelect);
                } else if self.handle.range().is_some() {
                    // Inside a `v`/`V` range, Space flips the cursor's entry against the range
                    // and stays put; later steps keep the flip (Strata's
                    // `toggle_visual_cursor`).
                    let i = cursor_or_first(s);
                    op(s, |sel, src| sel.toggle_in_run(src, i));
                    s.emit(Outcome::Reveal(i));
                } else {
                    toggle_advance(s);
                }
                true
            }
            Command::SelectAll => {
                if len == 0 {
                    s.emit(Outcome::NothingToSelect);
                } else if op(s, |sel, src| sel.select_all(src)).cursor {
                    s.emit(Outcome::Reveal(0));
                }
                true
            }
            Command::Invert => {
                if len == 0 {
                    s.emit(Outcome::NothingToSelect);
                } else {
                    op(s, |sel, src| sel.invert(src));
                }
                true
            }
            Command::Range(kind) => {
                self.range(kind, s);
                true
            }
            Command::Clear => {
                if s.selection().is_empty() {
                    return false;
                }
                op(s, |sel, _| sel.clear());
                true
            }
            Command::Activate => activate(s),
            Command::Swallow => true,
        }
    }

    /// Shift: the range from the anchor. With nothing selected, the anchor first moves to the
    /// cursor (or the end the move starts from); a one-entry step then selects that entry
    /// alone without moving, as Strata's `extend_selection` does, while a page or a jump
    /// extends from it at once (`extend_page_selection`).
    fn extend(&mut self, target: Target, s: &mut dyn Surface) {
        let len = s.source().len();
        if len == 0 {
            return;
        }
        let cursor = s.selection().cursor_index(s.source());
        let anchored = s.selection().anchor_index(s.source()).is_some();
        let fresh = s.selection().is_empty() || !anchored || cursor.is_none();
        let origin = cursor.unwrap_or(if forward(target) { 0 } else { len - 1 });
        if fresh {
            op(s, |sel, src| sel.move_cursor(src, origin, AnchorRule::Move));
        }
        let to = match (target, fresh) {
            (Target::Step(_), true) => origin,
            _ => match self.land(target, s) {
                Landing::At(i) => i,
                Landing::Edge(_) | Landing::Nowhere => origin,
            },
        };
        op(s, |sel, src| sel.extend_to(src, to, Extend::Replace));
        s.emit(Outcome::Reveal(to));
    }

    /// `v` or `V`: starts a range at the cursor, switches its kind, or ends one of this kind.
    fn range(&mut self, kind: RangeKind, s: &mut dyn Surface) {
        if self.handle.range() == Some(kind) {
            self.handle.set_range(None);
            return;
        }
        if s.source().is_empty() {
            self.handle.set_range(None);
            s.emit(Outcome::NothingToSelect);
            return;
        }
        // A load's automatic pick isn't part of the range's base (Strata's `start_visual`).
        op(s, |sel, _| sel.discard_provisional());
        let i = cursor_or_first(s);
        op(s, |sel, src| sel.move_cursor(src, i, AnchorRule::Move));
        op(s, |sel, src| sel.extend_to(src, i, kind.extend()));
        self.handle.set_range(Some(kind));
        s.emit(Outcome::Reveal(i));
    }

    /// Where `target` lands from the cursor, through the geometry.
    fn land(&self, target: Target, s: &dyn Surface) -> Landing {
        let len = s.source().len();
        if len == 0 {
            return match target {
                Target::Step(d) => Landing::Edge(d),
                _ => Landing::Nowhere,
            };
        }
        let last = len - 1;
        let cursor = s.selection().cursor_index(s.source());
        let page = s.geometry().page_step().max(1);
        let step = |v: Vertical, by: usize| match (cursor, v) {
            (None, Vertical::Down) => 0,
            (None, Vertical::Up) => last,
            (Some(i), Vertical::Down) => i.saturating_add(by).min(last),
            (Some(i), Vertical::Up) => i.saturating_sub(by),
        };
        match target {
            Target::First => Landing::At(0),
            Target::Last => Landing::At(last),
            Target::Page(v) => Landing::At(step(v, page)),
            Target::HalfPage(v) => Landing::At(step(v, (page / 2).max(1))),
            Target::Step(d) => {
                let Some(i) = cursor else {
                    return Landing::At(match d {
                        Direction::Down | Direction::Right => 0,
                        Direction::Up | Direction::Left => last,
                    });
                };
                if let Some(next) = s.geometry().neighbor(i, d) {
                    return Landing::At(next);
                }
                let wrap = self.handle.with(|settings| settings.wrap_rows);
                match d {
                    Direction::Left if wrap && i > 0 => Landing::At(i - 1),
                    Direction::Right if wrap && i < last => Landing::At(i + 1),
                    _ => Landing::Edge(d),
                }
            }
        }
    }
}

/// Runs one selection operation on the surface.
fn op(
    s: &mut dyn Surface,
    mut f: impl FnMut(&mut Selection, &dyn EntrySource) -> Changed,
) -> Changed {
    s.apply(&mut f)
}

/// Whether `command` leaves an active 10xer range going: moves walk it, Space flips inside
/// it, `v` and `V` end or switch it themselves.
fn keeps_range(command: Command) -> bool {
    matches!(
        command,
        Command::Cursor(_)
            | Command::ToggleAdvance
            | Command::Range(_)
            | Command::Activate
            | Command::Swallow
    )
}

/// Whether a target moves towards the last entry, so a range with no cursor starts at the
/// first.
fn forward(target: Target) -> bool {
    match target {
        Target::Step(d) => matches!(d, Direction::Down | Direction::Right),
        Target::Page(v) | Target::HalfPage(v) => v == Vertical::Down,
        Target::First => false,
        Target::Last => true,
    }
}

fn cursor_or_first(s: &dyn Surface) -> usize {
    s.selection().cursor_index(s.source()).unwrap_or(0)
}

/// 10xer Space outside a range: the first Space adds the cursor's entry even over a load's
/// automatic pick, then the cursor moves to the next entry in display order
/// (`toggle_cursor_and_advance`: `move_displayed_cursor(1, 1)`).
fn toggle_advance(s: &mut dyn Surface) {
    let len = s.source().len();
    let i = cursor_or_first(s);
    op(s, |sel, _| sel.discard_provisional());
    op(s, |sel, src| sel.toggle(src, i));
    let next = if i + 1 < len { i + 1 } else { i };
    if next != i {
        op(s, |sel, src| sel.move_cursor(src, next, AnchorRule::Keep));
    }
    s.emit(Outcome::Reveal(next));
}

/// Enter: opens the cursor's entry. With no cursor, the first entry takes the cursor and the
/// selection and opens (a recorded decision, from Strata's grouped-List rule that Return
/// starts at the first row shown). An empty collection leaves the key to the host.
fn activate(s: &mut dyn Surface) -> bool {
    let i = match s.selection().cursor_index(s.source()) {
        Some(i) => i,
        None if s.source().is_empty() => return false,
        None => {
            op(s, |sel, src| sel.select_only(src, 0));
            s.emit(Outcome::Reveal(0));
            0
        }
    };
    let id = s.source().id_at(i);
    s.emit(Outcome::Activate { id });
    true
}

/// The character a plain printable key types: no Ctrl, Alt or Super, Shift allowed.
fn printable(event: &KeyEvent) -> Option<char> {
    let m = event.mods;
    if m.cmd || m.ctrl || m.alt || m.logo {
        return None;
    }
    let Key::Text(text) = &event.key else {
        return None;
    };
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(c),
        _ => None,
    }
}
