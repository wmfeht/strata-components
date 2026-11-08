//! What the keyboard machine reads on every key, shared live with the host through a
//! [`KeyHandle`], with the 10xer range state and the commands the host queues.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use rune::KeyPlatform;

use super::command::{Command, RangeKind};
use super::keymap::KeyMap;

/// What plain printable keys (no Ctrl, Alt or Super) do when the map doesn't bind them.
///
/// Used by: C2, C3, C4, and hosts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Letters {
    /// They go on to the host: Strata's default, where Type to search is on and the typed
    /// character starts a search (C18). `h j k l` don't move.
    #[default]
    Pass,
    /// The map's printable aliases act as the arrows: `h j k l` with Type to search off.
    Arrows,
    /// They find an entry by its name's prefix (see the [module docs](super#type-ahead)).
    /// Strata has no counterpart; it is off unless a host asks for it.
    TypeAhead,
}

/// The keyboard machine's settings.
///
/// Fields may be added: start from [`KeySettings::default`] or [`KeySettings::tenxer`].
///
/// Used by: C2, C3, C4, C18, and hosts.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct KeySettings {
    /// Which chords run which commands.
    pub map: KeyMap,
    /// What unbound plain printable keys do.
    pub letters: Letters,
    /// How long after a typed character the next one still extends the type-ahead prefix.
    /// 1 s by default.
    pub type_ahead_timeout: Duration,
    /// Whether Left and Right cross row ends, to the previous or next entry, when the geometry
    /// has no neighbour that way. On for the icons grid (C3), as Strata's Icons does
    /// (lgse/strata#2044); W1's geometries never wrap themselves. Off for lists and columns,
    /// where Left and Right then reach the view's edge.
    pub wrap_rows: bool,
    /// The platform whose chord conventions keys match by. The current one by default; tests
    /// pin it.
    pub platform: KeyPlatform,
}

impl Default for KeySettings {
    /// The default map, letters passed on, no wrapping.
    fn default() -> Self {
        Self {
            map: KeyMap::default_map(),
            letters: Letters::Pass,
            type_ahead_timeout: Duration::from_secs(1),
            wrap_rows: false,
            platform: KeyPlatform::current(),
        }
    }
}

impl KeySettings {
    /// The 10xer map, with its letters as moves (10xer doesn't use type-to-search).
    pub fn tenxer() -> Self {
        Self {
            map: KeyMap::tenxer(),
            letters: Letters::Arrows,
            ..Self::default()
        }
    }
}

#[derive(Debug, Default)]
struct Shared {
    settings: RefCell<KeySettings>,
    range: Cell<Option<RangeKind>>,
    queue: RefCell<VecDeque<Command>>,
}

/// The settings one keyboard machine reads, its 10xer range, and commands for it to run,
/// shared with the host.
///
/// Clones share one value. Components take state by accessor, but the element keeps its
/// controllers boxed in its `CollectionState` (C1e's seam), where no accessor reaches; so, as
/// C1b's `PointerHandle` does, the host keeps a clone, changes the settings through it (the
/// 10xer switch, Type to search, a rebinding), reads the range for the footer, and queues
/// commands that start outside the collection. This is a recorded deviation from the accessor
/// rule.
///
/// Used by: C2, C3, C4, C18, and hosts.
#[derive(Clone, Debug, Default)]
pub struct KeyHandle(Rc<Shared>);

impl KeyHandle {
    /// A handle holding `settings`.
    pub fn new(settings: KeySettings) -> Self {
        Self(Rc::new(Shared {
            settings: RefCell::new(settings),
            ..Shared::default()
        }))
    }

    /// The settings now.
    pub fn get(&self) -> KeySettings {
        self.0.settings.borrow().clone()
    }

    /// Replaces the settings.
    pub fn set(&self, settings: KeySettings) {
        *self.0.settings.borrow_mut() = settings;
    }

    /// Changes the settings in place: `handle.update(|s| s.letters = Letters::Arrows)`.
    pub fn update(&self, f: impl FnOnce(&mut KeySettings)) {
        f(&mut self.0.settings.borrow_mut());
    }

    /// Reads the settings without copying them.
    pub(super) fn with<R>(&self, f: impl FnOnce(&KeySettings) -> R) -> R {
        f(&self.0.settings.borrow())
    }

    /// The active 10xer range, for the `VISUAL` and `UNSET` footer (C18) and Columns' mirroring
    /// (C2).
    pub fn range(&self) -> Option<RangeKind> {
        self.0.range.get()
    }

    /// Ends a 10xer range and keeps what it selected: a step of the host's Escape ladder, or
    /// leaving 10xer mode. False if none was active.
    pub fn end_range(&self) -> bool {
        self.0.range.take().is_some()
    }

    pub(super) fn set_range(&self, range: Option<RangeKind>) {
        self.0.range.set(range);
    }

    /// Queues `command` for the machine to run at its next `tick`, as a key would run it: a
    /// 10xer chord step (`g g` is `Command::Cursor(Target::First)`), a window-level Ctrl+A, the
    /// host's Escape ladder. The host then calls the collection's `tick`.
    pub fn run(&self, command: Command) {
        self.0.queue.borrow_mut().push_back(command);
    }

    /// Whether commands wait for the next `tick`.
    pub fn pending(&self) -> bool {
        !self.0.queue.borrow().is_empty()
    }

    pub(super) fn next(&self) -> Option<Command> {
        self.0.queue.borrow_mut().pop_front()
    }
}
