//! The virtual-row collection element with row slots (C1e): the Rune element under the list,
//! the icons grid and each Columns column.
//!
//! [`Collection`] draws an [`EntrySource`] as virtual rows inside a scroll viewport, from the
//! [`CollectionState`] the host keeps (its [`Selection`] among it). Rows host slots the host or
//! the view fills ([`Slot`]): an icon or thumbnail (C25), a label or rename editor (C5),
//! trailing cells (C4, [`ContentSlot`]), a drop highlight on folder rows and a drag source
//! (W1's [`Dnd`] binder). Drags out start from the pointer controller's
//! [`Intent::BeginDrag`], not from rows (see Decisions). Pointer, keyboard and marquee input
//! goes through one seam
//! ([`seam`]) to the controllers C1b, C1c and C1d install; with none, a press selects.
//!
//! ```ignore
//! struct App { listing: ListSource, coll: CollectionState, look: Look, version: u64 }
//!
//! Collection::new(
//!     "files",
//!     |s: &App| (&s.listing, &s.coll),
//!     |s: &mut App| (&s.listing, &mut s.coll),
//!     |s: &App| &s.look,
//!     |s: &App| s.version,
//! )
//! .name("Files")
//! .on_cursor(|s: &mut App, id, _| s.preview(id))
//! .build(s)
//! ```
//!
//! # The listing version (required)
//!
//! The host passes `listing_version`, a value it changes whenever the source's order or
//! entries change while its length stays the same: a re-sort, a refresh that replaces entries,
//! a rename that changes an id. Rune then skips re-reading every row's key on a commit, which
//! is what keeps a 100,000-entry list's commits independent of its length. A change of length
//! is always noticed (at a cost that grows with the list: Rune issue #203). A stale version
//! leaves rows keyed by their old ids; the rows still draw the right entries, since handlers
//! hit-test by index and resolve ids at the event.
//!
//! # Coordinates
//!
//! Every handler sits on the container. A pointer's content point is its window point minus
//! the viewport's window origin plus the scroll offset ([`CollectionState::content_point`]),
//! as `contract::geometry` defines it; [`CollectionState::content_to_window`] goes back, for
//! popup anchors (C21).
//!
//! # Performance
//!
//! Rows are built by index (`El::virtual_rows`), only for the mounted window, and each row is
//! a `memo` keyed by its entry id, versioned by its entry, its [`RowState`], the look and its
//! slots' versions. Scrolling commits no view; hover is drawn by Rune; the viewport's layout
//! and visible rows are recorded as bookkeeping. A selection change rebuilds only the rows
//! whose state changed. The perf test (`tests/perf.rs`) holds this to rune-kit's Table.
//!
//! # Decisions
//!
//! - **Focus.** Rune (at the pinned rev) has no programmatic focus for elements other than
//!   text fields, no focus-change notification and no focus-within styles (Rune draft
//!   U-C1e-4). So the element never moves focus: the position-restore bullets on focus after
//!   Back, Forward and Up, and the first-visit refocus, wait for that API. A selection looks
//!   the same whether or not the collection has focus (Strata dims it when unfocused), and the
//!   cursor outline follows keyboard mode (the last input that moved the cursor was a key)
//!   instead of keyboard focus.
//! - **Selection fill.** Selected rows use rune-kit's table selection colour
//!   (`TableStyle::from_kit(kit).selected`), hover Kit's `c.hover`, the cursor outline
//!   `c.border_focus` and a drop target or Enter target `c.text_accent`.
//! - **Rows move at once.** Inserted and removed rows land without growth or exit motion
//!   (`El::instant_changes`), so what is drawn always matches the index geometry presses and
//!   the marquee hit-test against. Removal effects (C23's dissolve, a slide) are the host's.
//! - **No drag source on rows.** Rune counts `El::drag_out` as a primary press handler
//!   (`Handlers::has_primary`, `el/handler_set.rs` at the pinned rev), so a row with one becomes
//!   the press target (`runtime/input/hit.rs`, `primary_target`) and the container's `on_drag`
//!   never hears presses on it: no selection, no click intent, no drag threshold. So rows get
//!   no `drag_out`, and the element takes no `DragSource`. A drag starts when the pointer
//!   controller (C1b) emits [`Intent::BeginDrag`] past its threshold, with the window point;
//!   the host binds that to the system drag (X2, through the platform shim).
//! - **Selecting from assistive technology.** Options have no press handler of their own
//!   (all input goes to the container, above), so an assistive client's default action on an
//!   option (AT-SPI's "click") does nothing at the pinned rev. Rune has no way to route an
//!   option's action to its container with the target's key (Rune draft U-C1e-8). Until it
//!   does, a screen-reader user selects with the keyboard, through the container's active
//!   descendant (C1c).
//! - **No cursor reveal on a pointer press.** A key or a tick that moves the cursor scrolls it
//!   into view; a press moves it only onto what the viewport already shows.
//!
//! Work package: C1e, `docs/work/phase-2-composites/C1e-collection-element.md`.

#![warn(missing_docs)]

mod a11y;
mod keys;
mod lines;
mod route;
mod row;
pub mod seam;
mod state;

#[cfg(test)]
mod tests;

use std::any::Any;
use std::borrow::Cow;
use std::ops::Range;
use std::rc::Rc;

use rune::{El, GrowthSpec, PointerButton, ScrollPos, ScrollSpec, Tx, col};
use strata_kit::look::Look;

pub use a11y::{Announcement, Roles, default_announcement};
pub use keys::{ICON, LABEL, ROWS, STATUS, VIEWPORT, part_path, row_key, row_path, viewport_path};
pub use lines::{Lines, OnePerLine};
pub use row::{ContentSlot, RowCx, RowMarks, RowState, Slot};
pub use seam::{
    Activation, Controllers, Ctx, Effect, Flow, Hit, Intent, KeyController, KeyInput,
    MarqueeController, PointerController, PointerInput, PointerPhase, Region, default_pointer,
};
pub use state::{CollectionState, DropOver};

#[cfg(doc)]
use crate::collection::selection::Selection;
use crate::contract::{
    CollectionGeometry, Dnd, DropHover, DropZone, Dropped, EntryId, EntrySource, Point, Rect,
};

type GetFn<S> = Rc<dyn Fn(&S) -> (&dyn EntrySource, &CollectionState)>;
type GetMutFn<S> = Rc<dyn Fn(&mut S) -> (&dyn EntrySource, &mut CollectionState)>;
type LookFn<S> = Rc<dyn Fn(&S) -> &Look>;
type ReadFn<S, T> = Rc<dyn Fn(&S) -> T>;
type RowHeightFn<S> = Rc<dyn Fn(&S, &Look) -> f32>;
type RegionFn<S> = Rc<dyn Fn(&S, usize, Point) -> Region>;
type PartFn<S> = Rc<dyn Fn(&S) -> Option<El>>;
type RenameFn<S> = (ReadFn<S, Option<EntryId>>, Slot<S>);
type MarksFn<S> = Rc<dyn Fn(&S, EntryId) -> RowMarks>;
type IntentFn<S> = Rc<dyn Fn(&mut S, Intent, &mut Tx)>;
type CursorFn<S> = Rc<dyn Fn(&mut S, Option<EntryId>, &mut Tx)>;
type SelectionFn<S> = Rc<dyn Fn(&mut S, &mut Tx)>;
type VisibleFn<S> = Rc<dyn Fn(&mut S, Range<usize>) -> bool>;
type AnnounceFn<S> = Rc<dyn Fn(&S, &Announcement) -> String>;

/// What [`Collection`] was built with, shared by its rows and handlers.
pub(crate) struct Config<S> {
    key: Rc<str>,
    get: GetFn<S>,
    get_mut: GetMutFn<S>,
    look: LookFn<S>,
    version: ReadFn<S, u64>,
    clock: Option<ReadFn<S, f64>>,
    settings: Option<Rc<dyn Any>>,
    name: Option<String>,
    roles: Roles,
    row_height: RowHeightFn<S>,
    lines: Rc<dyn Lines>,
    dnd: Dnd<S>,
    read_only: bool,
    on_drop: Option<row::DropFn<S>>,
    icon: Option<Slot<S>>,
    label: Option<Slot<S>>,
    content: Option<ContentSlot<S>>,
    rename: Option<RenameFn<S>>,
    marks: Option<MarksFn<S>>,
    regions: Option<RegionFn<S>>,
    header: Option<PartFn<S>>,
    overlay: Option<PartFn<S>>,
    empty: Option<PartFn<S>>,
    on_intent: Option<IntentFn<S>>,
    on_cursor: Option<CursorFn<S>>,
    on_selection: Option<SelectionFn<S>>,
    on_visible: Option<VisibleFn<S>>,
    announce: Option<AnnounceFn<S>>,
}

impl<S> Config<S> {
    /// The row height for `s`.
    fn row_height_of(&self, s: &S) -> f32 {
        (self.row_height)(s, (self.look)(s))
    }

    /// The host's clock, or 0 without one.
    fn now(&self, s: &S) -> f64 {
        self.clock.as_ref().map_or(0.0, |c| c(s))
    }
}

/// The collection element's builder.
///
/// The host keeps the source and a [`CollectionState`] in its state and gives the element
/// accessors to both, as one pair, so a handler can read the source while it changes the
/// state. See the [module docs](self) for the required `listing_version`.
///
/// Used by: C2 (one per column), C3, C4, C14b (read-only), hosts.
pub struct Collection<S> {
    cfg: Config<S>,
}

impl<S: 'static> Collection<S> {
    /// A collection keyed `key` (its rows' keys and scroll requests start there), over the
    /// source and state `get` and `get_mut` return, drawn with the look `look` returns.
    /// `listing_version` changes whenever the source's order or entries change at the same
    /// length (see the [module docs](self#the-listing-version-required)).
    ///
    /// The accessors are closures, so one host can build several collections over its own
    /// collections, each capturing which one (C2's columns):
    /// `move |s: &App| (&s.columns[i].listing, &s.columns[i].state)`.
    pub fn new(
        key: impl Into<Rc<str>>,
        get: impl Fn(&S) -> (&dyn EntrySource, &CollectionState) + 'static,
        get_mut: impl Fn(&mut S) -> (&dyn EntrySource, &mut CollectionState) + 'static,
        look: impl Fn(&S) -> &Look + 'static,
        listing_version: impl Fn(&S) -> u64 + 'static,
    ) -> Self {
        Self {
            cfg: Config {
                key: key.into(),
                get: Rc::new(get),
                get_mut: Rc::new(get_mut),
                look: Rc::new(look),
                version: Rc::new(listing_version),
                clock: None,
                settings: None,
                name: None,
                roles: Roles::LISTBOX,
                row_height: Rc::new(|_, l| l.row.list),
                lines: Rc::new(OnePerLine),
                dnd: Dnd::rune(),
                read_only: false,
                on_drop: None,
                icon: None,
                label: None,
                content: None,
                rename: None,
                marks: None,
                regions: None,
                header: None,
                overlay: None,
                empty: None,
                on_intent: None,
                on_cursor: None,
                on_selection: None,
                on_visible: None,
                announce: None,
            },
        }
    }

    /// The name assistive technology reads for the collection ("Documents").
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.cfg.name = Some(name.into());
        self
    }

    /// The roles: [`Roles::LISTBOX`] (the default), [`Roles::GRID_ROWS`] or
    /// [`Roles::GRID_CELLS`].
    pub fn roles(mut self, roles: Roles) -> Self {
        self.cfg.roles = roles;
        self
    }

    /// The row height, read from the look: `row.list` by default, `row.columns` for a Columns
    /// column, `slots::list::row_height` for ListTable. Every row draws at exactly this
    /// height, so the geometry matches what is drawn.
    pub fn row_height(mut self, row_height: impl Fn(&S, &Look) -> f32 + 'static) -> Self {
        self.cfg.row_height = Rc::new(row_height);
        self
    }

    /// How entries pack into lines (C3's grid); [`OnePerLine`] by default.
    pub fn lines(mut self, lines: impl Lines + 'static) -> Self {
        self.cfg.lines = Rc::new(lines);
        self
    }

    /// The host's clock in seconds (virtual in tests), which controllers time clicks and
    /// timeouts by. Rune's `Tx` has no clock (Rune draft U-C1e-6).
    pub fn clock(mut self, clock: impl Fn(&S) -> f64 + 'static) -> Self {
        self.cfg.clock = Some(Rc::new(clock));
        self
    }

    /// The host's settings for its controllers (the double-click interval, the click mode),
    /// which they read with [`Ctx::settings`]. Taken as the view builds, so pass the current
    /// values each time.
    pub fn settings(mut self, settings: impl Any) -> Self {
        self.cfg.settings = Some(Rc::new(settings));
        self
    }

    /// The drag-and-drop binder: Rune's own by default.
    pub fn dnd(mut self, dnd: Dnd<S>) -> Self {
        self.cfg.dnd = dnd;
        self
    }

    /// Read-only: no drops and no rename, as the archive preview needs (C14b). Controllers
    /// see it as [`Ctx::read_only`].
    pub fn read_only(mut self) -> Self {
        self.cfg.read_only = true;
        self
    }

    /// Takes drops: on a folder-like row (`Some(id)`) or the background (`None`, the folder
    /// shown). Folder rows draw the drop highlight while a drag hovers them.
    pub fn on_drop(
        mut self,
        f: impl Fn(&mut S, Option<EntryId>, Dropped, &mut Tx) + 'static,
    ) -> Self {
        self.cfg.on_drop = Some(Rc::new(f));
        self
    }

    /// The icon or thumbnail part (C25). By default, the entry's catalog icon.
    pub fn icon(mut self, slot: Slot<S>) -> Self {
        self.cfg.icon = Some(slot);
        self
    }

    /// The label part (matched text, say). By default, the name in `ty.ui_compact`.
    pub fn label(mut self, slot: Slot<S>) -> Self {
        self.cfg.label = Some(slot);
        self
    }

    /// The row's content, from its icon and label (C4's cells). By default, the icon then
    /// the label.
    pub fn content(mut self, slot: ContentSlot<S>) -> Self {
        self.cfg.content = Some(slot);
        self
    }

    /// The rename editor (C5): drawn in the label's place on the row `renaming` names. A
    /// read-only collection draws none.
    pub fn rename(
        mut self,
        renaming: impl Fn(&S) -> Option<EntryId> + 'static,
        editor: Slot<S>,
    ) -> Self {
        self.cfg.rename = Some((Rc::new(renaming), editor));
        self
    }

    /// The host's marks on each row: cut, dragging, the Enter target, the open path.
    pub fn marks(mut self, marks: impl Fn(&S, EntryId) -> RowMarks + 'static) -> Self {
        self.cfg.marks = Some(Rc::new(marks));
        self
    }

    /// Which part of entry `index`'s row a point (in the row's own points) is on, for a row
    /// whose content isn't the default icon and label (C3's caption, C4's cells).
    pub fn regions(mut self, f: impl Fn(&S, usize, Point) -> Region + 'static) -> Self {
        self.cfg.regions = Some(Rc::new(f));
        self
    }

    /// A part above the viewport (C4's column headings).
    pub fn header(mut self, f: impl Fn(&S) -> Option<El> + 'static) -> Self {
        self.cfg.header = Some(Rc::new(f));
        self
    }

    /// A part drawn over the viewport, pinned to it below any header, in its coordinates
    /// (C1d's band, C24's marker). It takes no input: presses go through it to the rows.
    pub fn overlay(mut self, f: impl Fn(&S) -> Option<El> + 'static) -> Self {
        self.cfg.overlay = Some(Rc::new(f));
        self
    }

    /// What an empty collection shows, pinned over the viewport below any header.
    pub fn empty(mut self, f: impl Fn(&S) -> Option<El> + 'static) -> Self {
        self.cfg.empty = Some(Rc::new(f));
        self
    }

    /// Hears the controllers' intents (open, rename, drag, context menu).
    pub fn on_intent(mut self, f: impl Fn(&mut S, Intent, &mut Tx) + 'static) -> Self {
        self.cfg.on_intent = Some(Rc::new(f));
        self
    }

    /// Hears the cursor move, after an input moved it: the preview follows it.
    pub fn on_cursor(mut self, f: impl Fn(&mut S, Option<EntryId>, &mut Tx) + 'static) -> Self {
        self.cfg.on_cursor = Some(Rc::new(f));
        self
    }

    /// Hears the selection change, after an input changed it.
    pub fn on_selection(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.cfg.on_selection = Some(Rc::new(f));
        self
    }

    /// Hears which entries the viewport shows, when that changes (C25's priority). Return
    /// true only if the view reads what it changed: scrolling then commits.
    pub fn on_visible(mut self, f: impl Fn(&mut S, Range<usize>) -> bool + 'static) -> Self {
        self.cfg.on_visible = Some(Rc::new(f));
        self
    }

    /// The status line's wording for a selection change: [`default_announcement`] by
    /// default. The line changes only when an operation changed the selection
    /// ([`CollectionState::note_selection`]), so a listing change alone announces nothing.
    pub fn announce(mut self, f: impl Fn(&S, &Announcement) -> String + 'static) -> Self {
        self.cfg.announce = Some(Rc::new(f));
        self
    }

    /// Runs the controllers' timeouts and queued commands. Call it from the host's poll on
    /// every frame while a controller asks for wake-ups ([`Effect::WakeAfter`], an
    /// auto-scroll), with the same builder the view uses; true when state changed. A tick
    /// with nothing to do costs a [`Ctx`] and no commit.
    ///
    /// A command that starts outside the collection (a menu item, a verb typed in a footer
    /// prompt) reaches its controller this way: the controller queues it in a handle the
    /// host shares, and the next tick runs it with a [`Ctx`] (see [`KeyController::tick`]).
    pub fn tick(self, s: &mut S, tx: &mut Tx) -> bool {
        let cfg = Rc::new(self.cfg);
        route::tick(&cfg, s, tx)
    }

    /// The geometry of `s`'s entries as the element lays them out now, in content
    /// coordinates: for an anchor out of the element (C21's keyboard menu at the cursor,
    /// through [`CollectionState::content_to_window`]).
    pub fn geometry(&self, s: &S) -> Box<dyn CollectionGeometry> {
        let row_height = self.cfg.row_height_of(s);
        let (src, st) = (self.cfg.get)(s);
        self.cfg
            .lines
            .geometry(src.len(), row_height, st.viewport.w, st.viewport.h)
    }

    /// Entry `index`'s rect in window points, as the element lays it out now.
    pub fn window_rect_of(&self, s: &S, index: usize) -> Rect {
        let rect = self.geometry(s).rect_of(index);
        (self.cfg.get)(s).1.content_to_window(rect)
    }

    /// The element, filling the height its parent gives it.
    pub fn build(self, s: &S) -> El {
        let cfg = Rc::new(self.cfg);
        let (src, st) = (cfg.get)(s);
        let look = (cfg.look)(s);
        let len = src.len();
        let per = st.per_line.max(1);
        let lines = lines::line_count(len, per);
        let row_height = cfg.row_height_of(s);
        let frame = row::Frame {
            look_key: row::look_key(look),
            row_height,
            per_line: per,
            len,
            hover_live: st.hover_live,
            cursor_shown: st.keyboard && st.active,
            drop: st.drop,
            renaming: cfg
                .rename
                .as_ref()
                .filter(|_| !cfg.read_only)
                .and_then(|(f, _)| f(s)),
        };

        let key_cfg = cfg.clone();
        let build_cfg = cfg.clone();
        let visible_cfg = cfg.clone();
        let rows = col()
            .key(ROWS)
            .w_full()
            .growth(GrowthSpec::default())
            // Rows land at once: an animated gap would draw rows where the geometry doesn't
            // put them, and a press during it would hit the wrong entry.
            .instant_changes()
            .virtual_rows(
                lines,
                row_height,
                move |s: &S, line| {
                    let src = (key_cfg.get)(s).0;
                    Cow::Owned(keys::line_key(src.id_at(line * per), per))
                },
                move |s: &S, line| row::build_line(&build_cfg, &frame, s, line),
            )
            .virtual_rows_version(((cfg.version)(s), per, len))
            .on_visible_rows(move |s: &mut S, r| route::on_visible(&visible_cfg, s, r));

        let live = st.scroll.clone();
        let layer = st.scroll_layer.clone();
        let layout_cfg = cfg.clone();
        let viewport = col()
            .key(VIEWPORT)
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .on_scroll_offset(move |p: ScrollPos| {
                live.set(p.offset[1]);
                layer.set([0.0, -p.offset[1]]);
            })
            .on_layout_tx(move |s: &mut S, r, tx| route::on_layout(&layout_cfg, s, r, tx))
            .child(rows);

        let drag_cfg = cfg.clone();
        let secondary_cfg = cfg.clone();
        let key_cfg = cfg.clone();
        let move_cfg = cfg.clone();
        let mut root = col()
            .key(cfg.key.to_string())
            .role(cfg.roles.container)
            .focusable(true)
            .w_full()
            .h_full()
            .min_h(0.0)
            .clip_soft(0.0)
            .pointer_buttons([PointerButton::Middle])
            .on_drag(move |s: &mut S, d, tx| route::on_drag(&drag_cfg, s, d, tx))
            .on_secondary_press_local(move |s: &mut S, e, tx| {
                route::on_secondary(&secondary_cfg, s, e, tx)
            })
            .on_key(move |s: &mut S, e, tx| route::on_key(&key_cfg, s, e, tx))
            .on_pointer_move_local(move |s: &mut S, _, tx| {
                route::on_pointer_move(&move_cfg, s, tx)
            });
        if let Some(name) = &cfg.name {
            root = root.label(name.clone());
        }
        if let Some(index) = st.selection.cursor_index(src) {
            // A path under this collection, so a second collection showing the same folder
            // (another pane) can't answer for it.
            root = root.active_descendant(keys::entry_path(&cfg.key, src, index, per));
        }
        if let Some(header) = cfg.header.as_ref().and_then(|f| f(s)) {
            root = root.child(header);
        }
        // The viewport and what sits over it: the empty state and the overlay, pinned to the
        // viewport alone, below any header.
        let mut over = col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .clip_soft(0.0)
            .child(viewport);
        if len == 0
            && let Some(empty) = cfg.empty.as_ref().and_then(|f| f(s))
        {
            over = over.child(pinned(empty));
        }
        if let Some(band) = st
            .controllers
            .marquee
            .as_ref()
            .and_then(|m| m.overlay(look))
        {
            // In content coordinates: a layer at the content's origin that rides the scroll
            // offset in the same frame.
            let content = col()
                .pin(Some(0.0), Some(0.0), Some(0.0), None)
                .live_offset(&st.scroll_layer)
                .child(band);
            over = over.child(pinned(content).inert());
        }
        if let Some(overlay) = cfg.overlay.as_ref().and_then(|f| f(s)) {
            over = over.child(pinned(overlay).inert());
        }
        root = root.child(over);
        let announcement = st.announcement();
        let line = match &cfg.announce {
            Some(f) => f(s, &announcement),
            None => default_announcement(&announcement),
        };
        root = root.child(a11y::status(&line));
        if let Some(on_drop) = &cfg.on_drop
            && !cfg.read_only
        {
            root = cfg
                .dnd
                .drop_zone(root, background_zone(&cfg, on_drop.clone()));
        }
        root
    }
}

/// `part` over the whole of its parent.
fn pinned(part: El) -> El {
    col()
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .child(part)
}

/// The background's drop zone: drops go to the folder shown.
fn background_zone<S: 'static>(cfg: &Rc<Config<S>>, on_drop: row::DropFn<S>) -> DropZone<S> {
    let get_mut = cfg.get_mut.clone();
    let hover_mut = cfg.get_mut.clone();
    let edge = cfg.clone();
    DropZone::new(move |s: &mut S, dropped, tx| {
        (get_mut)(s).1.drop = None;
        on_drop(s, None, dropped, tx);
    })
    .on_hover(move |s: &mut S, hover, _offer, _tx| {
        let st = (hover_mut)(s).1;
        match hover {
            DropHover::Accepting | DropHover::Rejecting => {
                if st.drop.is_none() {
                    st.drop = Some(DropOver::Background);
                }
            }
            DropHover::Left => {
                if st.drop == Some(DropOver::Background) {
                    st.drop = None;
                }
            }
        }
    })
    .on_move(move |s: &mut S, point, tx| route::drop_edge_scroll(&edge, s, point, tx))
}
