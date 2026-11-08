//! [`CollectionState`]: what the host keeps for one collection, behind the element's accessor.

use std::cell::Cell;
use std::ops::Range;

use rune::{Live, Tx};

use super::keys::{line_key, line_path, viewport_path};
use super::seam::{Controllers, Hit};
use crate::collection::selection::{Changed, Listing, Position, ScrollMemo, Selection, SyncRule};
use crate::contract::{EntryId, EntrySource, Point, Rect};

/// Where a drag hovering the collection would drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DropOver {
    /// On this folder-like entry's row.
    Row(EntryId),
    /// On the collection's background: the folder it shows.
    Background,
}

/// One collection's state: its selection, its controllers, and what the element records about
/// its viewport (where it shows, how far it is scrolled, which rows are visible).
///
/// The host keeps one per collection (a list, a grid, one Columns column) for the life of the
/// view, and hands it to the element by accessor. A folder change resets it with
/// [`CollectionState::enter`] rather than replacing it, so the scroll offset the element
/// follows stays the same value.
///
/// Used by: hosts, C2 (one per column), C3, C4, C14b, C18, C21.
pub struct CollectionState {
    /// The selection, cursor and anchor (C1a).
    pub selection: Selection,
    /// The controllers input is routed to (C1b, C1c, C1d).
    pub controllers: Controllers,
    pub(super) viewport: Rect,
    pub(super) scroll: Live<f32>,
    /// The scroll offset as a layer offset, `[0, -offset]`: what the marquee's layer rides.
    pub(super) scroll_layer: Live<[f32; 2]>,
    pub(super) visible: Range<usize>,
    pub(super) drop: Option<DropOver>,
    pub(super) hover_live: bool,
    pub(super) keyboard: bool,
    pub(super) active: bool,
    pub(super) layout: u64,
    pub(super) pending: Option<Position>,
    /// The scroll offset `enter` asked for while a restore waits: a scroll away from it is the
    /// person's, and cancels the restore.
    pub(super) pending_offset: f32,
    /// How many entries a line holds, as the last build or layout worked it out: a cell so the
    /// view (which reads the state) can record it, and sync, reveal and the visible range
    /// agree with what it built.
    pub(super) per_line: Cell<usize>,
    /// The row height, as last laid out.
    pub(super) row_height: f32,
    /// The pointer gesture in progress belongs to the collection (its press landed in the
    /// viewport), with what the press was over.
    pub(super) gesture: Option<Hit>,
    /// The selection as last announced: its revision, how many were selected and of how many.
    pub(super) announced: (u64, usize, usize),
}

impl Default for CollectionState {
    fn default() -> Self {
        Self {
            selection: Selection::new(),
            controllers: Controllers::default(),
            viewport: Rect::default(),
            scroll: Live::new(0.0),
            scroll_layer: Live::new([0.0, 0.0]),
            visible: 0..0,
            drop: None,
            hover_live: true,
            keyboard: false,
            active: true,
            layout: 0,
            pending: None,
            pending_offset: 0.0,
            per_line: Cell::new(1),
            row_height: 0.0,
            gesture: None,
            announced: (0, 0, 0),
        }
    }
}

impl std::fmt::Debug for CollectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectionState")
            .field("selection", &self.selection)
            .field("controllers", &self.controllers)
            .field("viewport", &self.viewport)
            .field("scroll", &self.scroll.get())
            .field("visible", &self.visible)
            .field("drop", &self.drop)
            .field("keyboard", &self.keyboard)
            .field("layout", &self.layout)
            .field("pending", &self.pending.is_some())
            .finish()
    }
}

impl CollectionState {
    /// Nothing selected, nothing recorded yet.
    pub fn new() -> Self {
        Self::default()
    }

    // Reading what the element recorded.

    /// The viewport's rect in window points, as last laid out.
    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    /// How far the viewport is scrolled, in points, as last drawn.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll.get()
    }

    /// The scroll offset as a live value, which follows a scroll in the same frame with no view
    /// commit: an overlay drawn in viewport coordinates (C1d's band, C24's marker) rides it.
    pub fn scroll_live(&self) -> Live<f32> {
        self.scroll.clone()
    }

    /// The entries in the viewport, as last reported: what thumbnails load first (C25).
    pub fn visible_range(&self) -> Range<usize> {
        self.visible.clone()
    }

    /// The part of the content the viewport shows, in content coordinates.
    pub fn visible_content(&self) -> Rect {
        Rect::new(0.0, self.scroll.get(), self.viewport.w, self.viewport.h)
    }

    /// The content point under window point `window`.
    pub fn content_point(&self, window: Point) -> Point {
        Point::new(
            window.x - self.viewport.x,
            window.y - self.viewport.y + self.scroll.get(),
        )
    }

    /// `rect`, in content coordinates, as window points: where a popup anchored to an entry
    /// opens (C21).
    pub fn content_to_window(&self, rect: Rect) -> Rect {
        rect.offset(self.viewport.x, self.viewport.y - self.scroll.get())
    }

    /// Where a drag hovering the collection would drop, while one does.
    pub fn drop_over(&self) -> Option<DropOver> {
        self.drop
    }

    /// Whether keyboard mode is on: the last input that moved the cursor was a key. The cursor
    /// outline shows only then, and row hover waits for the pointer to move.
    pub fn keyboard_mode(&self) -> bool {
        self.keyboard
    }

    /// Whether rows draw hover now: off after keyboard navigation until the pointer moves.
    pub fn row_hover(&self) -> bool {
        self.hover_live
    }

    /// The layout key the scroll offset is remembered under: see [`ScrollMemo`].
    pub fn layout_key(&self) -> u64 {
        self.layout
    }

    /// The text the live status line reads now: the last announced selection, worded by
    /// `wording`.
    pub(super) fn announcement(&self) -> super::Announcement {
        super::Announcement {
            selected: self.announced.1,
            total: self.announced.2,
        }
    }

    /// Whether a restore waits for the listing to complete.
    pub fn restore_pending(&self) -> bool {
        self.pending.is_some()
    }

    // Setting what the host decides.

    /// Sets the layout key: the view mode and anything that changes wrapping (an icons grid's
    /// width). A remembered scroll offset comes back only in the layout it was measured in. A
    /// new layout (a view switch) cancels a restore that waits for its listing.
    pub fn set_layout_key(&mut self, layout: u64) {
        if layout != self.layout {
            self.pending = None;
        }
        self.layout = layout;
    }

    /// Sets where a hovering drag would drop, for a binder that hears hovers itself (X2's
    /// system shim). Rune's binder sets it through the rows' drop zones.
    pub fn set_drop_over(&mut self, drop: Option<DropOver>) {
        self.drop = drop;
    }

    /// Shows the cursor or not: in Columns, only the column holding the keyboard cursor
    /// shows it (C2).
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Records keyboard navigation that moved the cursor from outside the element's own key
    /// handling (a page shortcut): keyboard mode turns on, and row hover waits for the pointer
    /// to move.
    pub fn keyboard_moved(&mut self) {
        self.keyboard = true;
        self.hover_live = false;
    }

    /// Takes the selection's current state as the one the live status line reads out, if an
    /// operation changed it since the last time (its revision moved on). The element does this
    /// after every input and in [`CollectionState::sync`] and [`CollectionState::enter`]; a
    /// host that changes the selection itself (a Select All shortcut) calls it after.
    pub fn note_selection(&mut self, source: &dyn EntrySource) {
        let revision = self.selection.revision();
        if revision != self.announced.0 {
            self.announced = (revision, self.selection.len(), source.len());
        }
    }

    // Keeping in step with the source.

    /// [`Selection::before_removal`]: call it right before the host removes entries.
    pub fn before_removal(&mut self, before: &dyn EntrySource, gone: impl Fn(EntryId) -> bool) {
        self.selection.before_removal(before, gone);
    }

    /// Brings the collection in step with `source` after it changed: [`Selection::sync`], then
    /// the cursor row keeps its place on screen (`Tx::keep_on_screen`). When the listing is
    /// complete and a restore waits for it, the restore applies now instead.
    ///
    /// `scope` is the collection's key, as given to the element.
    pub fn sync(
        &mut self,
        scope: &str,
        source: &dyn EntrySource,
        rule: SyncRule,
        tx: &mut Tx,
    ) -> Changed {
        if rule.listing == Listing::Complete
            && let Some(position) = self.pending.take()
        {
            let changed = self.apply_position(scope, source, &position, rule, tx);
            self.note_selection(source);
            return changed;
        }
        let changed = self.selection.sync(source, rule);
        if let Some(index) = self.selection.cursor_index(source) {
            // Rune holds a virtual row by its key: in a grid, the line the cursor's cell sits
            // in now. When the change moved the cell to another line, that line's key is new,
            // Rune finds no drawn rect to hold, and lets it go.
            tx.keep_on_screen(line_path(scope, source, index, self.per_line.get()));
        }
        self.note_selection(source);
        changed
    }

    /// Whether the line holding entry `index` shows whole in the viewport, as last laid out.
    fn shows_line(&self, index: usize) -> bool {
        let h = self.row_height;
        if h <= 0.0 {
            return false;
        }
        let top = (index / self.per_line.get().max(1)) as f32 * h;
        let offset = self.scroll.get();
        top >= offset && top + h <= offset + self.viewport.h
    }

    /// Takes `per` entries a line from now on. When that differs from before and there is a
    /// cursor, scrolls the cursor into view on `tx`: a reflow moves every entry to another
    /// line, and the lines' keys with them. True when the count changed.
    pub(super) fn reflow_to(
        &self,
        scope: &str,
        source: &dyn EntrySource,
        per: usize,
        tx: &mut Tx,
    ) -> bool {
        let per = per.max(1);
        if per == self.per_line.get() {
            return false;
        }
        self.per_line.set(per);
        if let Some(index) = self.selection.cursor_index(source) {
            self.reveal(scope, source, index, tx);
        }
        true
    }

    /// Scrolls the entry at `index` into view, mounted or not, just far enough.
    pub fn reveal(&self, scope: &str, source: &dyn EntrySource, index: usize, tx: &mut Tx) {
        if index >= source.len() {
            return;
        }
        let per = self.per_line.get().max(1);
        let first = source.id_at(index / per * per);
        tx.scroll_into_view_in(scope.to_owned(), line_key(first, per));
    }

    // Position restore.

    /// The position to remember for this folder as it is left: the selection's, with the
    /// scroll offset in the current layout.
    pub fn position(&self, source: &dyn EntrySource) -> Position {
        self.selection
            .position(source)
            .with_scroll(ScrollMemo::new(self.layout, self.scroll.get()))
    }

    /// Shows a folder: `source` is its listing as it stands (`listing` says whether it is
    /// complete), and `position` what was remembered for it, if anything.
    ///
    /// With a position, the selection, cursor and anchor come back. The scroll offset comes
    /// back if it was measured in the current layout; otherwise the cursor scrolls into view.
    /// While the listing loads, the restore waits for [`CollectionState::sync`] with a
    /// complete listing; a press, a key or a scroll in the collection before then cancels it,
    /// and so does a new layout key.
    ///
    /// Either way, the controllers' gestures (an open marquee) and keyboard mode end, and the
    /// view moves only by instant scrolls (an offset, an instant reveal, or the top), each of
    /// which stops a glide in progress: a controller's `reset`, which has no [`Ctx`], never
    /// leaves an auto-scroll glide running.
    ///
    /// [`Ctx`]: super::Ctx
    ///
    /// With none, the selection clears and the view starts at the top. Picking an entry
    /// (the folder you came from, the first entry) is the host's policy.
    pub fn enter(
        &mut self,
        scope: &str,
        source: &dyn EntrySource,
        position: Option<&Position>,
        listing: Listing,
        tx: &mut Tx,
    ) -> Changed {
        self.pending = None;
        self.drop = None;
        // The folder left takes its gestures and keyboard mode with it.
        self.controllers.reset();
        self.gesture = None;
        self.keyboard = false;
        self.hover_live = true;
        let changed = match position {
            Some(position) if listing == Listing::Loading => {
                self.pending = Some(position.clone());
                self.pending_offset = 0.0;
                let changed = self.selection.restore(&Position::default());
                tx.scroll_to_instant(viewport_path(scope), 0.0, 0.0);
                changed | self.selection.sync(source, SyncRule::loading())
            }
            Some(position) => self.apply_position(scope, source, position, SyncRule::default(), tx),
            None => {
                let changed = self.selection.restore(&Position::default());
                tx.scroll_to_instant(viewport_path(scope), 0.0, 0.0);
                let rule = match listing {
                    Listing::Complete => SyncRule::default(),
                    Listing::Loading => SyncRule::loading(),
                };
                changed | self.selection.sync(source, rule)
            }
        };
        self.note_selection(source);
        changed
    }

    /// Drops a restore that waits for its listing.
    pub fn cancel_restore(&mut self) {
        self.pending = None;
    }

    fn apply_position(
        &mut self,
        scope: &str,
        source: &dyn EntrySource,
        position: &Position,
        rule: SyncRule,
        tx: &mut Tx,
    ) -> Changed {
        let changed = self.selection.restore(position) | self.selection.sync(source, rule);
        match position.scroll.and_then(|m| m.offset_for(self.layout)) {
            Some(offset) => tx.scroll_to_instant(viewport_path(scope), 0.0, offset),
            None => match self.selection.cursor_index(source) {
                // Instant either way, and either way a scroll request, which stops a glide in
                // progress: the reveal when the row is out of view, else a scroll to where the
                // view is (a reveal that has nowhere to go might leave a glide running).
                Some(index) if self.shows_line(index) => {
                    tx.scroll_to_instant(viewport_path(scope), 0.0, self.scroll.get());
                }
                Some(index) => self.reveal(scope, source, index, tx),
                None => tx.scroll_to_instant(viewport_path(scope), 0.0, 0.0),
            },
        }
        changed
    }
}
