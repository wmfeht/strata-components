//! The virtual-row collection element with row slots (C1e): the Rune element under the list,
//! the icons grid and each Columns column.
//!
//! [`Collection`] draws an [`EntrySource`] as virtual rows inside a scroll viewport, from the
//! [`CollectionState`] the host keeps (its [`Selection`] among it). Rows host slots the host or
//! the view fills ([`Slot`]): an icon or thumbnail (C25), a label or rename editor (C5),
//! trailing cells (C4, [`ContentSlot`]), a drop highlight on folder rows and a drag source
//! (W1's [`Dnd`] binder). Pointer, keyboard and marquee input goes through one seam
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
    MarqueeController, PointerController, PointerInput, PointerPhase, Region,
};
pub use state::{CollectionState, DropOver};

#[cfg(doc)]
use crate::collection::selection::Selection;
use crate::contract::{Dnd, DragSource, DropHover, DropZone, Dropped, EntryId, EntrySource, Point};

type GetFn<S> = fn(&S) -> (&dyn EntrySource, &CollectionState);
type GetMutFn<S> = fn(&mut S) -> (&dyn EntrySource, &mut CollectionState);
type RegionFn<S> = Rc<dyn Fn(&S, usize, Point) -> Region>;
type PartFn<S> = Rc<dyn Fn(&S) -> Option<El>>;
type RenameFn<S> = (fn(&S) -> Option<EntryId>, Slot<S>);
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
    look: fn(&S) -> &Look,
    version: fn(&S) -> u64,
    clock: Option<fn(&S) -> f64>,
    name: Option<String>,
    roles: Roles,
    row_height: fn(&Look) -> f32,
    lines: Rc<dyn Lines>,
    dnd: Dnd<S>,
    read_only: bool,
    on_drop: Option<row::DropFn<S>>,
    drag_source: Option<DragSource<S>>,
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
    pub fn new(
        key: impl Into<Rc<str>>,
        get: GetFn<S>,
        get_mut: GetMutFn<S>,
        look: fn(&S) -> &Look,
        listing_version: fn(&S) -> u64,
    ) -> Self {
        Self {
            cfg: Config {
                key: key.into(),
                get,
                get_mut,
                look,
                version: listing_version,
                clock: None,
                name: None,
                roles: Roles::LISTBOX,
                row_height: |l| l.row.list,
                lines: Rc::new(OnePerLine),
                dnd: Dnd::rune(),
                read_only: false,
                on_drop: None,
                drag_source: None,
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
    pub fn row_height(mut self, row_height: fn(&Look) -> f32) -> Self {
        self.cfg.row_height = row_height;
        self
    }

    /// How entries pack into lines (C3's grid); [`OnePerLine`] by default.
    pub fn lines(mut self, lines: impl Lines + 'static) -> Self {
        self.cfg.lines = Rc::new(lines);
        self
    }

    /// The host's clock in seconds (virtual in tests), which controllers time clicks and
    /// timeouts by. Rune's `Tx` has no clock (Rune draft U-C1e-6).
    pub fn clock(mut self, clock: fn(&S) -> f64) -> Self {
        self.cfg.clock = Some(clock);
        self
    }

    /// The drag-and-drop binder: Rune's own by default.
    pub fn dnd(mut self, dnd: Dnd<S>) -> Self {
        self.cfg.dnd = dnd;
        self
    }

    /// Read-only: no drops, no drag-out and no rename, as the archive preview needs (C14b).
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

    /// Makes every row a drag source carrying what `source` gives (the host builds the payload
    /// from the selection).
    pub fn drag_source(mut self, source: DragSource<S>) -> Self {
        self.cfg.drag_source = Some(source);
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

    /// The rename editor (C5): drawn in the label's place on the row `renaming` names.
    pub fn rename(mut self, renaming: fn(&S) -> Option<EntryId>, editor: Slot<S>) -> Self {
        self.cfg.rename = Some((renaming, editor));
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

    /// A part drawn over the viewport, in its coordinates (C1d's band, C24's marker). It takes
    /// no input.
    pub fn overlay(mut self, f: impl Fn(&S) -> Option<El> + 'static) -> Self {
        self.cfg.overlay = Some(Rc::new(f));
        self
    }

    /// What an empty collection shows.
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
    /// default.
    pub fn announce(mut self, f: impl Fn(&S, &Announcement) -> String + 'static) -> Self {
        self.cfg.announce = Some(Rc::new(f));
        self
    }

    /// Runs the controllers' timeouts. Call it from the host's poll, with the same builder
    /// the view uses; true when state changed.
    pub fn tick(self, s: &mut S, tx: &mut Tx) -> bool {
        let cfg = Rc::new(self.cfg);
        route::tick(&cfg, s, tx)
    }

    /// The element, filling the height its parent gives it.
    pub fn build(self, s: &S) -> El {
        let cfg = Rc::new(self.cfg);
        let (src, st) = (cfg.get)(s);
        let look = (cfg.look)(s);
        let len = src.len();
        let per = cfg.lines.per_line().max(1);
        let lines = lines::line_count(len, per);
        let row_height = (cfg.row_height)(look);
        let frame = row::Frame {
            look_key: row::look_key(look),
            row_height,
            per_line: per,
            len,
            hover_live: st.hover_live,
            cursor_shown: st.keyboard && st.active,
            drop: st.drop,
            renaming: cfg.rename.as_ref().and_then(|(f, _)| f(s)),
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
            .virtual_rows_version(((cfg.version)(s), per))
            .on_visible_rows(move |s: &mut S, r| route::on_visible(&visible_cfg, s, r));

        let live = st.scroll.clone();
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
            root = root.active_descendant(row_key(src.id_at(index)));
        }
        if let Some(header) = cfg.header.as_ref().and_then(|f| f(s)) {
            root = root.child(header);
        }
        root = root.child(viewport);
        if len == 0
            && let Some(empty) = cfg.empty.as_ref().and_then(|f| f(s))
        {
            root = root.child(empty);
        }
        if let Some(overlay) = cfg.overlay.as_ref().and_then(|f| f(s)) {
            root = root.child(overlay);
        }
        let announcement = Announcement {
            selected: st.selection.len(),
            total: len,
        };
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

/// The background's drop zone: drops go to the folder shown.
fn background_zone<S: 'static>(cfg: &Rc<Config<S>>, on_drop: row::DropFn<S>) -> DropZone<S> {
    let get_mut = cfg.get_mut;
    let edge = cfg.clone();
    DropZone::new(move |s: &mut S, dropped, tx| {
        (get_mut)(s).1.drop = None;
        on_drop(s, None, dropped, tx);
    })
    .on_hover(move |s: &mut S, hover, _offer, _tx| {
        let st = (get_mut)(s).1;
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
