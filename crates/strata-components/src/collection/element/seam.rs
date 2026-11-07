//! The controller seam: where pointer, keyboard and marquee controllers (C1b, C1c, C1d) plug
//! into the element.
//!
//! A controller is a state machine the host keeps in its [`CollectionState`]
//! ([`Controllers`]). The element routes each input to it as plain data ([`PointerInput`],
//! [`KeyInput`]), with a [`Ctx`] that gives it the source, the geometry and the selection. A
//! controller changes the selection through [`Ctx::apply`] and asks for everything else
//! through [`Effect`]s: scrolling a row into view, starting a marquee, waking later, or an
//! [`Intent`] the host acts on (open, rename, drag, context menu). The element then applies the
//! effects, and tells the host and assistive technology what changed.
//!
//! The controllers' own work packages implement these traits in their own modules
//! (`collection::pointer`, `collection::keyboard`, `collection::marquee`), and hosts install
//! them, so a controller never edits the element.
//!
//! [`CollectionState`]: super::CollectionState

use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

use rune::{KeyEvent, Mods, PhysicalKeyCode, PointerButton};

use crate::collection::selection::{Changed, Selection};
use crate::contract::{CollectionGeometry, EntryId, EntrySource, Point, Rect};

/// Which part of a pointer gesture an input is.
///
/// Used by: C1b, C1d, C24.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerPhase {
    /// A button went down. A secondary press arrives only as this, with no move or release.
    Down,
    /// The pointer moved while the button is held.
    Move,
    /// The button came up, or the gesture was cancelled ([`PointerInput::cancelled`]).
    Up,
}

/// Where in a row a point falls, as Strata's pointer intent tells them apart
/// (`src/ui/pointer.rs`, `upstream/main` 35ee677ab).
///
/// Used by: C1b, C1d, C3, C4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    /// On the entry itself: its icon, thumbnail or the drawn text of its name.
    Content,
    /// Blank row space, or no entry at all.
    Gutter,
}

/// What a pointer input is over.
///
/// Used by: C1b, C1d.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hit {
    /// The entry under the pointer, or `None` over empty space.
    pub index: Option<usize>,
    /// Which part of the entry's row. Empty space is [`Region::Gutter`].
    pub region: Region,
}

/// One pointer input on the collection, in content coordinates.
///
/// Fields may be added.
///
/// Used by: C1b, C1d, C24.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct PointerInput {
    /// Down, move or up.
    pub phase: PointerPhase,
    /// The button that drove it: primary, middle, or secondary (a [`PointerPhase::Down`]
    /// only).
    pub button: PointerButton,
    /// The pointer in content coordinates (see `contract::geometry`).
    pub point: Point,
    /// The pointer in window points, for popups and native drags.
    pub window: Point,
    /// The pointer's travel since the press, in points.
    pub travel: Point,
    /// What the pointer is over.
    pub hit: Hit,
    /// The modifiers held.
    pub mods: Mods,
    /// Which click of a run the press is: 1, then 2, 3 for presses each within Rune's
    /// double-click interval and 4 pt of the one before. C1b applies its own interval.
    pub clicks: u32,
    /// On [`PointerPhase::Up`]: the gesture ended without a release (Escape, focus lost).
    pub cancelled: bool,
    /// The host's clock, in seconds (virtual in tests).
    pub now: f64,
}

/// One key the focused collection heard.
///
/// Fields may be added.
///
/// Used by: C1c, C1d (Escape).
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct KeyInput<'a> {
    /// The key and its modifiers, as Rune reports them.
    pub event: &'a KeyEvent,
    /// The physical key behind it, for chords matched by position.
    pub physical: Option<PhysicalKeyCode>,
    /// The character the key types with no modifiers under the current layout.
    pub base: Option<char>,
    /// Whether the key is repeating.
    pub repeat: bool,
    /// The host's clock, in seconds.
    pub now: f64,
}

/// Whether a controller took an input.
///
/// Used by: C1b, C1c, C1d.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flow {
    /// It took it: nothing else hears it.
    Handled,
    /// It didn't: the next in line hears it (a key goes on to the page and to Rune).
    Pass,
}

/// How an entry was activated.
///
/// Used by: C1b, C1c.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Activation {
    /// A single click, in single-click mode.
    Click,
    /// A double click.
    DoubleClick,
    /// Return, or another key.
    Key,
}

/// What a controller asks the host to do. The host decides what each means (its policy).
///
/// Used by: C1b, C1c, C1d, C2, C3, C4, and hosts.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Intent {
    /// Open the entry.
    Activate {
        /// The entry.
        id: EntryId,
        /// How.
        how: Activation,
    },
    /// Open the entry in a new tab, or a new window (a middle click).
    OpenInNew {
        /// The entry.
        id: EntryId,
        /// A new window rather than a tab.
        window: bool,
    },
    /// Start a drag of these entries: the host builds the `DragPayload`.
    BeginDrag {
        /// The dragged entries, in display order.
        ids: Vec<EntryId>,
    },
    /// Start renaming the entry.
    BeginRename {
        /// The entry.
        id: EntryId,
    },
    /// Open a context menu for the entry, or the background, at `anchor` (window points).
    ContextMenu {
        /// The entry, or `None` for the background.
        target: Option<EntryId>,
        /// Where to open it, in window points.
        anchor: Rect,
    },
    /// Anything else a controller and its host agree on.
    Other(Rc<dyn Any>),
}

/// What a controller asks the element to do after an input.
///
/// Used by: C1b, C1c, C1d, C24.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Effect {
    /// Scroll the entry at this index into view, mounted or not.
    Reveal(usize),
    /// Start a marquee at this content point, with these modifiers: the element hands it to
    /// the marquee controller, which takes the gesture's moves and its release.
    BeginMarquee {
        /// The press, in content coordinates.
        at: Point,
        /// The modifiers held at the press.
        mods: Mods,
    },
    /// Scroll the viewport so its top shows content y `y`: a glide when `glide`, else a jump.
    ScrollTo {
        /// The content offset to show at the top.
        y: f32,
        /// Glide on Rune's spring rather than jump.
        glide: bool,
    },
    /// Keyboard mode on (the cursor outline shows, row hover waits for the pointer to move)
    /// or off.
    KeyboardMode(bool),
    /// Run the controllers' `tick` again after this long: a timeout.
    WakeAfter(Duration),
    /// Hand the host an intent.
    Intent(Intent),
}

/// What a controller sees and changes while it handles an input.
///
/// Used by: C1b, C1c, C1d.
pub struct Ctx<'a> {
    /// The entries, in display order.
    pub source: &'a dyn EntrySource,
    /// Where they sit, mounted or not.
    pub geometry: &'a dyn CollectionGeometry,
    /// The part of the content the viewport shows, in content coordinates.
    pub visible: Rect,
    /// The host's clock, in seconds.
    pub now: f64,
    selection: &'a mut Selection,
    changed: Changed,
    effects: Vec<Effect>,
}

impl<'a> Ctx<'a> {
    pub(super) fn new(
        source: &'a dyn EntrySource,
        geometry: &'a dyn CollectionGeometry,
        visible: Rect,
        now: f64,
        selection: &'a mut Selection,
    ) -> Self {
        Self {
            source,
            geometry,
            visible,
            now,
            selection,
            changed: Changed::NONE,
            effects: Vec::new(),
        }
    }

    /// The selection, to read.
    pub fn selection(&self) -> &Selection {
        self.selection
    }

    /// Runs one selection operation on the source, and keeps what it changed so the element
    /// can tell the host and assistive technology:
    /// `cx.apply(|sel, src| sel.select_only(src, i))`.
    pub fn apply(
        &mut self,
        op: impl FnOnce(&mut Selection, &dyn EntrySource) -> Changed,
    ) -> Changed {
        let changed = op(self.selection, self.source);
        self.changed |= changed;
        changed
    }

    /// Asks the element for `effect` once the input is handled.
    pub fn emit(&mut self, effect: Effect) {
        self.effects.push(effect);
    }

    /// What the operations so far changed.
    pub fn changed(&self) -> Changed {
        self.changed
    }

    pub(super) fn finish(self) -> (Changed, Vec<Effect>) {
        (self.changed, self.effects)
    }
}

/// Pointer intent (C1b): presses, moves and releases on the collection.
pub trait PointerController {
    /// Handles one pointer input.
    fn pointer(&mut self, input: &PointerInput, cx: &mut Ctx<'_>) -> Flow;

    /// Runs when a timeout it asked for ([`Effect::WakeAfter`]) may have passed, from the
    /// host's poll ([`Collection::tick`](super::Collection::tick)).
    fn tick(&mut self, _cx: &mut Ctx<'_>) -> Flow {
        Flow::Pass
    }
}

/// Keyboard navigation (C1c): keys while the collection has focus.
pub trait KeyController {
    /// Handles one key. [`Flow::Pass`] lets the page and Rune hear it.
    fn key(&mut self, input: &KeyInput<'_>, cx: &mut Ctx<'_>) -> Flow;

    /// As [`PointerController::tick`] (a type-ahead timeout).
    fn tick(&mut self, _cx: &mut Ctx<'_>) -> Flow {
        Flow::Pass
    }
}

/// Marquee selection (C1d). Started by [`Effect::BeginMarquee`]; while it is
/// [`active`](Self::active) it takes the gesture's moves and its release, and Escape.
pub trait MarqueeController {
    /// Starts a band at `at` with `mods` held.
    fn begin(&mut self, at: Point, mods: Mods, cx: &mut Ctx<'_>);

    /// The pointer moved to `at` (content coordinates).
    fn moved(&mut self, at: Point, cx: &mut Ctx<'_>);

    /// The button came up: keep what the band selected.
    fn end(&mut self, cx: &mut Ctx<'_>);

    /// Escape, or a cancelled gesture: put the selection back. False if no band was open.
    fn cancel(&mut self, cx: &mut Ctx<'_>) -> bool;

    /// Whether a band is open.
    fn active(&self) -> bool;

    /// The band, in content coordinates, for the overlay to draw.
    fn band(&self) -> Option<Rect>;

    /// As [`PointerController::tick`] (edge auto-scroll).
    fn tick(&mut self, _cx: &mut Ctx<'_>) -> Flow {
        Flow::Pass
    }
}

/// The controllers a collection routes to. Each slot is empty until its work package's
/// controller is installed; with no pointer controller, a press selects the entry under it
/// (or clears the selection over empty space).
///
/// Used by: hosts, C2, C3, C4.
#[derive(Default)]
pub struct Controllers {
    /// Pointer intent (C1b).
    pub pointer: Option<Box<dyn PointerController>>,
    /// Keyboard navigation (C1c).
    pub keyboard: Option<Box<dyn KeyController>>,
    /// Marquee (C1d).
    pub marquee: Option<Box<dyn MarqueeController>>,
}

impl std::fmt::Debug for Controllers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Controllers")
            .field("pointer", &self.pointer.is_some())
            .field("keyboard", &self.keyboard.is_some())
            .field("marquee", &self.marquee.is_some())
            .finish()
    }
}

/// The pointer behaviour with no pointer controller: a primary press selects only the entry
/// under it, or clears the selection over empty space.
pub(super) fn default_pointer(input: &PointerInput, cx: &mut Ctx<'_>) -> Flow {
    if input.phase != PointerPhase::Down || input.button != PointerButton::Primary {
        return Flow::Pass;
    }
    match input.hit.index {
        Some(i) => cx.apply(|sel, src| sel.select_only(src, i)),
        None => cx.apply(|sel, _| sel.clear()),
    };
    Flow::Handled
}
