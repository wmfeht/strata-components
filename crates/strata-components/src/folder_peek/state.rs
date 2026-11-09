//! The peek's state: whether peeking is on, which folder is peeked from where, and what the
//! pointer rests on. Plain transitions, no Rune.

use std::rc::Rc;

use super::place::{Placement, place};
use crate::collection::element::RestReport;
use crate::contract::{EntryId, Rect};

/// How a peek opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origin {
    /// The pointer rested on the folder for the open delay. Leaving both the folder and the
    /// card closes it.
    Hover,
    /// The host opened it from a key (Strata's `i` in 10xer mode): it stays until the host
    /// closes it, and its card is a polite live region.
    Keyboard,
}

/// The folder a peek shows: what the host lists.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PeekTarget {
    /// The key of the collection the folder's row is in.
    pub collection: Rc<str>,
    /// The folder's entry.
    pub folder: EntryId,
    /// A number new with each peek opened, for matching listing results to the peek that asked
    /// for them.
    pub serial: u64,
    /// How it opened.
    pub origin: Origin,
}

/// What a transition did, for the host's listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Nothing the host lists changed.
    None,
    /// A peek opened, or another replaced it: list [`PeekState::target`].
    Opened,
    /// The peek closed: drop its listing.
    Closed,
}

#[derive(Clone, Debug)]
struct Open {
    target: PeekTarget,
    place: Placement,
    /// The folder's row reports the pointer resting on it.
    row: bool,
}

/// The folder peek's state, which the host keeps and passes by accessor.
///
/// Off by default: the host turns it on from its preference ([`PeekState::set_enabled`]).
///
/// Used by: hosts (Strata's Icons and List panes).
#[derive(Clone, Debug, Default)]
pub struct PeekState {
    enabled: bool,
    open: Option<Open>,
    serial: u64,
    /// The card reports the pointer on it.
    card: bool,
}

impl PeekState {
    /// Peeking off, nothing open.
    pub fn new() -> Self {
        Self::default()
    }

    /// Turns peeking on or off. Off closes an open peek, and rows stop waiting on the pointer
    /// (their next build has no delays), so a pending peek never opens. True when a peek
    /// closed.
    pub fn set_enabled(&mut self, on: bool) -> bool {
        self.enabled = on;
        !on && self.close()
    }

    /// Whether peeking is on.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The folder peeked now, if any: the host lists it, and matches results to its
    /// [`PeekTarget::serial`].
    pub fn target(&self) -> Option<&PeekTarget> {
        self.open.as_ref().map(|o| &o.target)
    }

    /// Where the open card goes.
    pub fn placement(&self) -> Option<Placement> {
        self.open.as_ref().map(|o| o.place)
    }

    /// Whether entry `id` of collection `collection` is the folder a peek is open from: the
    /// host's [`RowMarks::peek_anchor`](crate::collection::element::RowMarks::peek_anchor).
    pub fn is_anchor(&self, collection: &str, id: EntryId) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| o.target.folder == id && &*o.target.collection == collection)
    }

    /// Closes the open peek. True when one was open, so a host's Escape stops here
    /// (`integration/10xer-mode`: an open peek closes before any other Escape step). Call it
    /// on C1b's `Intent::BeginDrag`, when a system drag ends, on navigating and on leaving
    /// a mode that kept a keyboard peek.
    pub fn close(&mut self) -> bool {
        self.card = false;
        self.open.take().is_some()
    }

    /// Opens a peek on folder `folder` of collection `collection`, whose row is at `item` in
    /// `viewport` (window points: C1e's `Collection::window_rect_of` and
    /// `CollectionState::viewport`). Nothing opens while peeking is off or when the viewport
    /// is narrower than the card.
    pub fn open(
        &mut self,
        collection: impl Into<Rc<str>>,
        folder: EntryId,
        origin: Origin,
        item: Rect,
        viewport: Rect,
    ) -> Change {
        if !self.enabled {
            return Change::None;
        }
        let Some(place) = place(item, viewport) else {
            return Change::None;
        };
        self.serial += 1;
        self.open = Some(Open {
            target: PeekTarget {
                collection: collection.into(),
                folder,
                serial: self.serial,
                origin,
            },
            place,
            row: origin == Origin::Hover,
        });
        Change::Opened
    }

    /// The keyboard peek (Strata's `i`): closes the peek open on this folder, else opens one
    /// with [`Origin::Keyboard`].
    pub fn toggle(
        &mut self,
        collection: impl Into<Rc<str>>,
        folder: EntryId,
        item: Rect,
        viewport: Rect,
    ) -> Change {
        let collection = collection.into();
        if self.is_anchor(&collection, folder) {
            self.close();
            return Change::Closed;
        }
        self.open(collection, folder, Origin::Keyboard, item, viewport)
    }

    /// A folder row's rest report (C1e's [`RestReport`]).
    pub(super) fn row_rest(&mut self, r: &RestReport) -> Change {
        if !self.enabled {
            return Change::None;
        }
        let mine = self.is_anchor(&r.collection, r.id);
        if mine {
            let open = self.open.as_mut().expect("anchored");
            open.row = r.inside;
            return self.settle();
        }
        // Rune counts the rows under the card as hovered too (Rune draft U-C22-1): one the
        // pointer only rests on through the card opens nothing.
        if !r.inside || self.card {
            return Change::None;
        }
        self.open(r.collection.clone(), r.id, Origin::Hover, r.row, r.viewport)
    }

    /// The card's own rest report.
    pub(super) fn card_rest(&mut self, inside: bool) -> Change {
        self.card = inside && self.open.is_some();
        self.settle()
    }

    /// A hover peek with the pointer on neither its folder nor its card closes.
    fn settle(&mut self) -> Change {
        match &self.open {
            Some(o) if o.target.origin == Origin::Hover && !o.row && !self.card => {
                self.close();
                Change::Closed
            }
            _ => Change::None,
        }
    }
}
