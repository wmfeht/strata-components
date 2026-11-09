//! [`ColumnsState`]: what the host keeps for one window's column strip, and the lifecycle
//! calls it makes as Strata opens, closes and returns to folders.

use rune::{Spring, Tx};
use strata_kit::look::Look;

use super::reveal::ColumnSpan;
use super::sources::ColumnSources;
use super::width::{self, Resize};
use super::{drag_scroll::DragScroll, keys};
use crate::collection::element::{CollectionState, viewport_path};
use crate::collection::keyboard::{KeyHandle, Keyboard};
use crate::collection::marquee::Marquee;
use crate::collection::pointer::PointerIntent;
use crate::collection::selection::{Listing, Position, SyncRule};
use crate::contract::{EntryId, EntrySource, Point, Rect};
use crate::scroll::{Autoscroll, AutoscrollPointer, Bar, ScrollFeed};

/// Where the keyboard cursor goes when a column opens.
///
/// Used by: hosts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    /// The active column stays where it is: mirroring's child column (Up or Down onto a
    /// folder), shown without moving focus.
    #[default]
    No,
    /// The new column becomes the active one, with nothing selected in it: a click on a folder,
    /// a double click.
    Column,
    /// The new column becomes the active one, with the cursor on its first entry once it has
    /// one: Right or Return on a folder (Strata's `focus_child`).
    FirstEntry,
}

/// A folder a column opens on: its listing as it stands, whether that is complete, and the
/// position remembered for it, if any.
///
/// Used by: hosts.
#[derive(Clone, Copy)]
pub struct Opening<'a> {
    /// The folder's listing.
    pub source: &'a dyn EntrySource,
    /// Whether it is complete or still loading.
    pub listing: Listing,
    /// The cursor, selection and scroll remembered for it.
    pub position: Option<&'a Position>,
}

impl<'a> Opening<'a> {
    /// `source`, complete or `listing`, with no position remembered.
    pub fn new(source: &'a dyn EntrySource, listing: Listing) -> Self {
        Self {
            source,
            listing,
            position: None,
        }
    }

    /// The same opening, restoring `position`.
    pub fn position(mut self, position: &'a Position) -> Self {
        self.position = Some(position);
        self
    }
}

/// How a column comes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Entrance {
    /// Slides 24 points in from the left as it fades in: a new column.
    Slide,
    /// Fades in where the column it replaces stood: a sibling.
    Fade,
    /// No motion: a rebuilt strip.
    Still,
}

/// A cursor a column places once its listing allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pending {
    None,
    /// On the first entry ([`Focus::FirstEntry`] on a folder still loading).
    First,
    /// On the folder the column has open (Left into a column with no cursor that is still
    /// loading); nothing if it's gone by then.
    Open(EntryId),
}

/// One column: its collection's state, and what the strip keeps for it.
///
/// Used by: hosts.
pub struct ColumnState {
    /// The column's collection: its selection, cursor and controllers (C1a–C1d, C24's
    /// autoscroll decorator), as [`ColumnsState::push`] installs them. A host may replace the
    /// controllers.
    pub collection: CollectionState,
    /// The collection's key serial: new with each folder the column shows.
    pub(super) serial: u64,
    /// The shell's key serial: kept by a sibling that takes the column's place.
    pub(super) slot: u64,
    pub(super) feed: ScrollFeed,
    pub(super) bar: Bar,
    /// The width in base points (at the default text size).
    pub(super) width: f32,
    pub(super) entrance: Entrance,
    pub(super) keys: KeyHandle,
    pub(super) pending: Pending,
}

impl ColumnState {
    /// The column's key handle, shared with its keyboard controller: the 10xer range, and
    /// commands from outside the column (C1c's [`KeyHandle`]).
    pub fn key_handle(&self) -> &KeyHandle {
        &self.keys
    }

    /// The width it draws at before the text size, in base points.
    pub fn width(&self) -> f32 {
        self.width
    }
}

impl std::fmt::Debug for ColumnState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColumnState")
            .field("serial", &self.serial)
            .field("width", &self.width)
            .field("collection", &self.collection)
            .finish_non_exhaustive()
    }
}

/// One window's column strip: its columns, which one holds the keyboard cursor, the strip's
/// scroll position and bar, a column-edge resize and a held drag in progress.
///
/// The host keeps one per window (and one in the chooser) for the life of the view, beside its
/// listings ([`ColumnSources`]), and calls the lifecycle methods as Strata's policy opens,
/// closes and returns to folders: [`ColumnsState::push`], [`ColumnsState::swap_from`],
/// [`ColumnsState::truncate`], [`ColumnsState::replace`] and [`ColumnsState::return_to`]. The
/// listings and the columns change in the same handler.
///
/// Used by: hosts.
pub struct ColumnsState {
    pub(super) key: String,
    pub(super) columns: Vec<ColumnState>,
    pub(super) active: usize,
    /// The column the latest pointer or keyboard input targeted: its header shows the
    /// actions. A held target (an open context menu) wins.
    pub(super) targeted: Option<usize>,
    pub(super) held: Option<usize>,
    serial: u64,
    pub(super) strip_feed: ScrollFeed,
    pub(super) strip_bar: Bar,
    /// The strip's viewport in window points, as last laid out.
    pub(super) strip: Rect,
    /// A reveal asked for before the strip had a width.
    pub(super) reveal_after_layout: bool,
    default_width: f32,
    cap: Option<f32>,
    pub(super) drag: DragScroll,
    pub(super) resize: Option<Resize>,
    /// The column whose edge the pointer rests on, past the caption's delay.
    pub(super) edge_hover: Option<usize>,
    pub(super) autoscroll: Autoscroll,
    /// What an accessor of a column that has gone reads (a handler or a tick that outlived
    /// it): an empty collection.
    pub(super) spare: CollectionState,
    /// The folder the last activation opened a column for, with its column: the second press
    /// of that click's run leaves the new column active.
    pub(super) opened: Option<(usize, EntryId)>,
    /// The column pressed last was the active one at the press: a click on its empty space
    /// clears every column, rather than picking what it selects.
    pub(super) press_was_active: bool,
    bar_mode: crate::scroll::BarMode,
    /// The host's clock at the last tick.
    pub(super) now: f64,
    /// A reveal that waits for the next tick: a row's reveal jumped the strip this frame, which
    /// is held where it was, and the view's own reveal glides it from there.
    pub(super) reveal_next: bool,
    /// When the last reveal was asked for, and the strip's drawn width then: while the strip's
    /// width changes soon after (a drawer opening in the same handler, a width spring), the
    /// reveal is worked out again.
    pub(super) revealed: Option<(f64, f32)>,
}

impl std::fmt::Debug for ColumnsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColumnsState")
            .field("key", &self.key)
            .field("columns", &self.columns)
            .field("active", &self.active)
            .field("targeted", &self.targeted)
            .finish_non_exhaustive()
    }
}

/// The reveal's glide: about Strata's 220 ms `emphasized_deceleration`.
pub(super) const REVEAL: Spring = Spring::new(0.22, 0.0);

/// How long after a reveal a change of the strip's width works it out again, in seconds: a
/// drawer slides open in 260 ms.
const RECHECK: f64 = 0.5;

impl ColumnsState {
    /// No columns yet. `key` is the view's key (its strip's, and the start of every column's);
    /// `autoscroll` is the window's middle-click autoscroll (C24), which every column's pointer
    /// decorator shares.
    pub fn new(key: impl Into<String>, autoscroll: Autoscroll) -> Self {
        Self {
            key: key.into(),
            columns: Vec::new(),
            active: 0,
            targeted: None,
            held: None,
            serial: 0,
            strip_feed: ScrollFeed::new(),
            strip_bar: Bar::new(),
            strip: Rect::default(),
            reveal_after_layout: false,
            default_width: width::STANDARD,
            cap: None,
            drag: DragScroll::new(),
            resize: None,
            edge_hover: None,
            autoscroll,
            spare: CollectionState::new(),
            opened: None,
            press_was_active: false,
            bar_mode: crate::scroll::BarMode::Overlay,
            now: 0.0,
            reveal_next: false,
            revealed: None,
        }
    }

    // Reading.

    /// How many columns there are.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Column `depth`.
    pub fn column(&self, depth: usize) -> Option<&ColumnState> {
        self.columns.get(depth)
    }

    /// Column `depth`, to change.
    pub fn column_mut(&mut self, depth: usize) -> Option<&mut ColumnState> {
        self.columns.get_mut(depth)
    }

    /// The column that holds the keyboard cursor.
    pub fn active(&self) -> usize {
        self.active
    }

    /// The column whose header shows the actions: the one an open menu holds, else the one the
    /// latest pointer or keyboard input targeted, else the active one.
    pub fn targeted(&self) -> usize {
        self.held.or(self.targeted).unwrap_or(self.active)
    }

    /// The key of column `depth`'s collection: its rows' key paths and scroll requests start
    /// there (`viewport_path`, `row_path`).
    pub fn column_key(&self, depth: usize) -> Option<String> {
        self.columns
            .get(depth)
            .map(|c| keys::column(&self.key, c.serial))
    }

    /// The strip's scroll position, as last drawn: how far it is scrolled sideways.
    pub fn strip_offset(&self) -> f32 {
        self.strip_feed.get().offset[0]
    }

    /// The strip's viewport in window points, as last laid out.
    pub fn strip_rect(&self) -> Rect {
        self.strip
    }

    /// When the strip's and the columns' scroll bars show: hidden at rest by default
    /// (`BarMode::Overlay`), or whenever they overflow (a desktop that turns overlay scrolling
    /// off).
    pub fn set_bar_mode(&mut self, mode: crate::scroll::BarMode) {
        self.strip_bar.set_mode(mode);
        for c in &self.columns {
            c.bar.set_mode(mode);
        }
        self.bar_mode = mode;
    }

    /// The window's autoscroll, which the host drives (`Autoscroll::drive`) and whose shield it
    /// mounts.
    pub fn autoscroll(&self) -> &Autoscroll {
        &self.autoscroll
    }

    /// The column a held file drag would drop into, as of the last frame.
    pub fn drop_column(&self) -> Option<usize> {
        self.drag.pointer().and(self.drag.hovered())
    }

    /// Whether the pointer is on the edge of the last column (the one that meets the preview)
    /// or resizing it: the host lights the preview divider's line then
    /// (`DrawerState::light_line`, C9).
    pub fn last_edge_lit(&self) -> bool {
        let last = self.columns.len().checked_sub(1);
        last.is_some() && (self.edge_hover == last || self.resize.as_ref().map(|r| r.depth) == last)
    }

    // Settings.

    /// The width new columns open at, in base points: the saved `browser_column_width` (or
    /// `chooser_column_width`), at least one standard column.
    pub fn set_default_width(&mut self, base: f32) {
        self.default_width = width::sane(base).unwrap_or(width::STANDARD);
    }

    /// The width a column may take at most, in points: the visible browser area (the preview
    /// split less its divider, the sidebar counted at full width). A new column opens at most
    /// this wide, and an edge drag stops there. Never below one standard column.
    pub fn set_width_cap(&mut self, cap: Option<f32>) {
        self.cap = cap.filter(|c| c.is_finite() && *c > 0.0);
    }

    pub(super) fn cap(&self) -> Option<f32> {
        self.cap
    }

    // The lifecycle.

    fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    fn new_column(&mut self, slot: Option<u64>, look: &Look, entrance: Entrance) -> ColumnState {
        let serial = self.next_serial();
        let slot = slot.unwrap_or(serial);
        let key = keys::column(&self.key, serial);
        let feed = ScrollFeed::new();
        let keys = KeyHandle::default();
        let mut collection = CollectionState::new();
        collection.controllers.pointer = Some(Box::new(super::pointer::ColumnPointer::new(
            Box::new(AutoscrollPointer::new(
                Some(Box::new(PointerIntent::new())),
                self.autoscroll.clone(),
                key,
                feed.clone(),
            )),
        )));
        collection.controllers.keyboard = Some(Box::new(Keyboard::new(keys.clone())));
        collection.controllers.marquee = Some(Box::new(Marquee::new()));
        let scale = width::scale(look);
        let base = match self.cap {
            // A saved width wider than the area opens narrowed to it; the setting stays.
            Some(cap) => self.default_width.min((cap / scale).max(width::STANDARD)),
            None => self.default_width,
        };
        let bar = Bar::new();
        bar.set_mode(self.bar_mode);
        ColumnState {
            collection,
            serial,
            slot,
            feed,
            bar,
            width: base,
            entrance,
            keys,
            pending: Pending::None,
        }
    }

    /// Opens a column after the last on `open` (the folder's listing as it stands, whether it is
    /// complete, and a remembered position). `focus` says whether it becomes the active column,
    /// and where its cursor goes; an active column is revealed. Returns its depth.
    pub fn push(&mut self, open: Opening<'_>, focus: Focus, look: &Look, tx: &mut Tx) -> usize {
        let column = self.new_column(None, look, Entrance::Slide);
        self.columns.push(column);
        let depth = self.columns.len() - 1;
        self.enter(depth, open, focus, look, tx);
        depth
    }

    /// Opens a sibling in column `depth`'s place: that column goes at once and the new one
    /// fades in where it stood, while the columns after it shrink away. As
    /// [`ColumnsState::push`] otherwise.
    pub fn swap_from(
        &mut self,
        depth: usize,
        open: Opening<'_>,
        focus: Focus,
        look: &Look,
        tx: &mut Tx,
    ) -> usize {
        if depth >= self.columns.len() {
            return self.push(open, focus, look, tx);
        }
        self.drop_from(depth);
        let slot = self.columns.get(depth).map(|c| c.slot);
        let column = self.new_column(slot, look, Entrance::Fade);
        self.columns.truncate(depth);
        self.columns.push(column);
        self.enter(depth, open, focus, look, tx);
        depth
    }

    fn enter(&mut self, depth: usize, open: Opening<'_>, focus: Focus, look: &Look, tx: &mut Tx) {
        let key = keys::column(&self.key, self.columns[depth].serial);
        let column = &mut self.columns[depth];
        column
            .collection
            .enter(&key, open.source, open.position, open.listing, tx);
        match focus {
            Focus::No => {
                let active = self.active.min(self.columns.len() - 1);
                self.set_active_quiet(active);
            }
            Focus::Column => {
                self.set_active_quiet(depth);
                self.reveal(look, tx);
            }
            Focus::FirstEntry => {
                self.set_active_quiet(depth);
                self.place_first(depth, open.source, open.listing, tx);
                self.reveal_after_rows(look, tx);
            }
        }
    }

    /// Closes the columns from `len` on: they narrow and fade out, unless animations are off.
    /// The active column moves to the last one left if it was among them, and is revealed.
    pub fn truncate(&mut self, len: usize, look: &Look, tx: &mut Tx) {
        let len = len.max(1);
        if len >= self.columns.len() {
            return;
        }
        self.drop_from(len);
        self.columns.truncate(len);
        let active = self.active.min(len - 1);
        self.set_active_quiet(active);
        self.reveal(look, tx);
    }

    /// Ends what belongs to the columns from `len` on: a resize of one, a menu held on one, its
    /// autoscroll.
    fn drop_from(&mut self, len: usize) {
        for c in self.columns.iter().skip(len) {
            self.autoscroll
                .stop_gone(Some(&keys::column(&self.key, c.serial)));
        }
        if self.resize.as_ref().is_some_and(|r| r.depth >= len) {
            self.resize = None;
        }
        if self.edge_hover.is_some_and(|d| d >= len) {
            self.edge_hover = None;
        }
        if self.held.is_some_and(|d| d >= len) {
            self.held = None;
        }
        if self.targeted.is_some_and(|d| d >= len) {
            self.targeted = None;
        }
        if self.opened.is_some_and(|(d, _)| d + 1 >= len) {
            self.opened = None;
        }
    }

    /// Replaces every column with fresh ones for `sources` (another location, a rebuilt strip,
    /// a switch back to Columns), with no motion, and `active` holding the cursor. Each column
    /// restores the position `position` gives it.
    pub fn replace(
        &mut self,
        sources: &dyn ColumnSources,
        active: usize,
        position: &dyn Fn(usize) -> Option<Position>,
        look: &Look,
        tx: &mut Tx,
    ) {
        self.drop_from(0);
        self.columns.clear();
        self.drag.reset();
        // Old columns go at once, new ones come in at once.
        tx.instant();
        for depth in 0..sources.len() {
            let column = self.new_column(None, look, Entrance::Still);
            self.columns.push(column);
            let key = keys::column(&self.key, self.columns[depth].serial);
            let listing = if sources.loading(depth) {
                Listing::Loading
            } else {
                Listing::Complete
            };
            let pos = position(depth);
            self.columns[depth].collection.enter(
                &key,
                sources.source(depth),
                pos.as_ref(),
                listing,
                tx,
            );
        }
        if !self.columns.is_empty() {
            self.set_active_quiet(active.min(self.columns.len() - 1));
            self.reveal(look, tx);
        }
    }

    /// Back, Forward or Up to the ancestor in column `depth`: the columns after it close, it
    /// becomes the active column, and `came_from` (the folder the strip came from, if that
    /// column lists it) is selected with the cursor on it, scrolled into view. The came-from
    /// folder's own column stays closed.
    pub fn return_to(
        &mut self,
        depth: usize,
        sources: &dyn ColumnSources,
        came_from: Option<EntryId>,
        look: &Look,
        tx: &mut Tx,
    ) {
        if depth >= self.columns.len() {
            return;
        }
        self.truncate(depth + 1, look, tx);
        self.set_active_quiet(depth);
        let key = keys::column(&self.key, self.columns[depth].serial);
        let source = sources.source(depth);
        let column = &mut self.columns[depth];
        if let Some(index) = came_from.and_then(|id| source.index_of(id)) {
            column.collection.selection.select_only(source, index);
            column.collection.note_selection(source);
            column.collection.reveal(&key, source, index, tx);
        }
        self.reveal_after_rows(look, tx);
    }

    /// Brings column `depth` in step with its listing after it changed
    /// (`CollectionState::sync`), and places a cursor the column was waiting to place: on the
    /// first entry once there is one, or on the folder it has open once it is listed (nothing,
    /// if that folder is gone).
    pub fn sync(&mut self, depth: usize, source: &dyn EntrySource, rule: SyncRule, tx: &mut Tx) {
        let Some(column) = self.columns.get_mut(depth) else {
            return;
        };
        let key = keys::column(&self.key, column.serial);
        column.collection.sync(&key, source, rule, tx);
        let complete = rule.listing == Listing::Complete;
        match column.pending {
            Pending::First if !source.is_empty() => {
                column.pending = Pending::None;
                place(column, &key, source, 0, tx);
            }
            Pending::Open(id) if complete => {
                column.pending = Pending::None;
                if column.collection.selection.cursor().is_none()
                    && let Some(index) = source.index_of(id)
                {
                    place(column, &key, source, index, tx);
                }
            }
            Pending::First if complete => column.pending = Pending::None,
            _ => {}
        }
    }

    // The active column.

    /// Makes column `depth` the one that holds the keyboard cursor, and reveals it.
    pub fn set_active(&mut self, depth: usize, look: &Look, tx: &mut Tx) {
        if depth < self.columns.len() {
            self.set_active_quiet(depth);
            self.reveal(look, tx);
        }
    }

    /// Holds column `depth`'s header actions while a menu opened from it is open, wherever the
    /// pointer goes (`None` when it closes).
    pub fn hold_target(&mut self, depth: Option<usize>) {
        self.held = depth.filter(|d| *d < self.columns.len());
    }

    /// The active column without a reveal: only it shows the cursor. A column the keyboard
    /// leaves drops a cursor it was waiting to place (Strata's restore gives up once focus
    /// leaves the list, `cursor.rs` `list_keeps_focus`): a late listing there moves nothing.
    pub(super) fn set_active_quiet(&mut self, depth: usize) {
        self.active = depth;
        self.targeted = Some(depth);
        for (i, c) in self.columns.iter_mut().enumerate() {
            c.collection.set_active(i == depth);
            if i != depth {
                c.pending = Pending::None;
            }
        }
    }

    /// The cursor on column `depth`'s first entry, or once it has one.
    pub(super) fn place_first(
        &mut self,
        depth: usize,
        source: &dyn EntrySource,
        listing: Listing,
        tx: &mut Tx,
    ) {
        let key = keys::column(&self.key, self.columns[depth].serial);
        let column = &mut self.columns[depth];
        if column.collection.selection.cursor().is_some() {
            return;
        }
        if source.is_empty() {
            if listing == Listing::Loading {
                column.pending = Pending::First;
            }
            return;
        }
        place(column, &key, source, 0, tx);
    }

    /// Left into column `depth` with no cursor: the cursor goes on the folder it has open, or
    /// waits for it while the column loads.
    pub(super) fn place_open(&mut self, depth: usize, sources: &dyn ColumnSources, tx: &mut Tx) {
        let key = keys::column(&self.key, self.columns[depth].serial);
        let source = sources.source(depth);
        let open = sources.open_child(depth);
        let loading = sources.loading(depth);
        let column = &mut self.columns[depth];
        if column.collection.selection.cursor().is_some() {
            return;
        }
        match open.and_then(|id| source.index_of(id).map(|i| (id, i))) {
            Some((_, index)) => place(column, &key, source, index, tx),
            None => {
                if let (Some(id), true) = (open, loading) {
                    column.pending = Pending::Open(id);
                }
            }
        }
    }

    /// Clears the selection in every column (the cursors stay).
    pub(super) fn clear_all(&mut self, sources: &dyn ColumnSources) {
        for (depth, c) in self.columns.iter_mut().enumerate() {
            if depth < sources.len() && !c.collection.selection.is_empty() {
                c.collection.selection.clear();
                c.collection.note_selection(sources.source(depth));
            }
        }
    }

    // The strip's scroll.

    /// Each column's span in strip content points, root first, as the widths stand.
    pub(super) fn spans(&self, look: &Look) -> Vec<(f32, f32)> {
        let scale = width::scale(look);
        let mut x = 0.0;
        self.columns
            .iter()
            .enumerate()
            .map(|(d, c)| {
                let w = self.drawn_width(d, c, scale);
                let span = (x, x + w);
                x += w;
                span
            })
            .collect()
    }

    /// Column `depth`'s width as drawn now: its own, or a resize's live one.
    pub(super) fn drawn_width(&self, depth: usize, c: &ColumnState, scale: f32) -> f32 {
        let base = match &self.resize {
            Some(r) if r.depth == depth => r.live,
            _ => c.width,
        };
        width::drawn(base, scale)
    }

    /// [`ColumnsState::reveal`] after a row of a column was revealed in the same transaction (a
    /// key, a placed cursor). The row's reveal (`scroll_into_view_in`) scrolls every viewport
    /// around the row, the strip included, at once, and a glide asked for in the same frame
    /// would start from where that left the strip. So the strip is held where it was drawn this
    /// frame (an offset request, which Rune applies after reveals), and the view's reveal glides
    /// it from there on the next tick. Rune's scoped reveal can't be kept to its own viewport's
    /// axis (Rune draft U-C2-4).
    pub(super) fn reveal_after_rows(&mut self, _look: &Look, tx: &mut Tx) {
        if self.strip.w > 0.0 && !self.columns.is_empty() {
            tx.scroll_to_instant(keys::strip(&self.key), self.strip_offset(), 0.0);
            self.reveal_next = true;
            tx.wake_after(std::time::Duration::ZERO);
        } else {
            self.reveal_after_layout = true;
        }
    }

    /// The reveal's share of a tick: a reveal waiting from the last frame, and a recent reveal
    /// worked out again while the strip's drawn width moves (the reveal is worked out from the
    /// width the strip had, and a glide is clamped to the strip's extent as it springs).
    pub(super) fn tick_reveal(&mut self, now: f64, look: &Look, tx: &mut Tx) {
        self.now = now;
        if std::mem::take(&mut self.reveal_next) {
            self.reveal(look, tx);
            return;
        }
        let Some((at, drawn)) = self.revealed else {
            return;
        };
        if now - at > RECHECK {
            self.revealed = None;
            return;
        }
        let page = self.strip_feed.get().drawn[0];
        if (page - drawn).abs() > 0.5 {
            self.reveal(look, tx);
            if let Some((_, w)) = self.revealed.as_mut() {
                *w = page;
            }
            tx.wake_after(std::time::Duration::ZERO);
        }
    }

    /// Scrolls the strip so the active column shows, as Strata's `ColumnSpan::reveal_target`
    /// does: a column clipped even a little comes fully into view, one that shows stays, and
    /// one wider than the strip that fills it stays. The columns after it (a mirrored child)
    /// come into view too where there is room, never at the active column's expense.
    pub(super) fn reveal(&mut self, look: &Look, tx: &mut Tx) {
        let page = self.strip.w;
        if page <= 0.0 || self.columns.is_empty() {
            self.reveal_after_layout = true;
            return;
        }
        self.reveal_after_layout = false;
        if !matches!(self.revealed, Some((at, _)) if at == self.now) {
            self.revealed = Some((self.now, self.strip_feed.get().drawn[0]));
        }
        let spans = self.spans(look);
        let active = self.active.min(spans.len() - 1);
        let (left, right) = spans[active];
        let total = spans.last().map_or(0.0, |s| s.1);
        let span = ColumnSpan {
            left,
            right,
            trailing: total - right,
        };
        let current = self.strip_offset();
        let max = (total - page).max(0.0);
        let target = span.reveal_target(current, page, 0.0, max);
        if (target - current).abs() < 0.5 {
            return;
        }
        let strip = keys::strip(&self.key);
        if look.animations {
            tx.scroll_to_with(strip, target, 0.0, REVEAL);
        } else {
            tx.scroll_to_instant(strip, target, 0.0);
        }
    }

    // Drags.

    /// Where an accepted file drag is over the strip (window points), or that it left the
    /// strip, was dropped, ended or was refused (`None`): what the drag autoscroll and the
    /// drop highlight follow. Rune's binder reaches it through the view's drop zones; a binder
    /// that hears the drag elsewhere (X2's) calls it. True when a frame is wanted: wake the
    /// loop (`tx.wake_after(ZERO)`).
    pub fn drag_over(&mut self, at: Option<Point>) -> bool {
        self.drag.over(at)
    }

    /// Ends a column-edge resize or a held drag, as when the strip leaves the view mid-gesture
    /// (a view switch): Rune tells an element removed under a drag nothing (Rune draft
    /// R-C4-2).
    pub fn reset(&mut self) {
        self.resize = None;
        self.edge_hover = None;
        self.drag.reset();
    }

    /// The scroll request target for column `depth`'s listing.
    pub(super) fn listing_path(&self, depth: usize) -> Option<String> {
        self.column_key(depth).map(|k| viewport_path(&k))
    }

    /// The column whose collection is keyed by `serial`, if it is still open.
    pub(super) fn depth_of(&self, serial: u64) -> Option<usize> {
        self.columns.iter().position(|c| c.serial == serial)
    }
}

/// The cursor on `index`, selected alone, as a person's choice: keyboard mode on, scrolled
/// into view.
fn place(column: &mut ColumnState, key: &str, source: &dyn EntrySource, index: usize, tx: &mut Tx) {
    column.collection.selection.select_only(source, index);
    column.collection.note_selection(source);
    column.collection.keyboard_moved();
    column.collection.reveal(key, source, index, tx);
}
