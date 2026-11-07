//! [`CollectionState`]: what the host keeps for one collection, behind the element's accessor.

use std::ops::Range;

use rune::{Live, Tx};

use super::keys::{line_key, row_path, viewport_path};
use super::seam::Controllers;
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
    pub(super) visible: Range<usize>,
    pub(super) drop: Option<DropOver>,
    pub(super) hover_live: bool,
    pub(super) keyboard: bool,
    pub(super) active: bool,
    pub(super) layout: u64,
    pub(super) pending: Option<Position>,
    pub(super) per_line: usize,
}

impl Default for CollectionState {
    fn default() -> Self {
        Self {
            selection: Selection::new(),
            controllers: Controllers::default(),
            viewport: Rect::default(),
            scroll: Live::new(0.0),
            visible: 0..0,
            drop: None,
            hover_live: true,
            keyboard: false,
            active: true,
            layout: 0,
            pending: None,
            per_line: 1,
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

    /// Whether a restore waits for the listing to complete.
    pub fn restore_pending(&self) -> bool {
        self.pending.is_some()
    }

    // Setting what the host decides.

    /// Sets the layout key: the view mode and anything that changes wrapping (an icons grid's
    /// width). A remembered scroll offset comes back only in the layout it was measured in.
    pub fn set_layout_key(&mut self, layout: u64) {
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
            return self.apply_position(scope, source, &position, rule, tx);
        }
        let changed = self.selection.sync(source, rule);
        if let Some(id) = self.selection.cursor()
            && source.index_of(id).is_some()
        {
            tx.keep_on_screen(row_path(scope, id));
        }
        changed
    }

    /// Scrolls the entry at `index` into view, mounted or not, just far enough.
    pub fn reveal(&self, scope: &str, source: &dyn EntrySource, index: usize, tx: &mut Tx) {
        if index >= source.len() {
            return;
        }
        let per = self.per_line.max(1);
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
    /// complete listing; a press or a key in the collection before then cancels it.
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
        match position {
            Some(position) if listing == Listing::Loading => {
                self.pending = Some(position.clone());
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
        }
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
                Some(index) => self.reveal(scope, source, index, tx),
                None => tx.scroll_to_instant(viewport_path(scope), 0.0, 0.0),
            },
        }
        changed
    }
}
