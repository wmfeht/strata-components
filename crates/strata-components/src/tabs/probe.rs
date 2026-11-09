//! Where the strip's parts drew, read back from hidden markers, for what Rune can't do from
//! the outside: read a viewport's scroll offset, or reveal with a margin (U-C7-2, U-C7-7).
//!
//! It leans on rune-kit's layout at the pinned rev, so it is fragile: the strip's root starts
//! its scroll viewport at its left edge, with nothing before it, and lists the tabs from the
//! viewport's start, tab 0 first. While a tab dragged in from elsewhere holds the gap before
//! tab 0, an offset read is off by that gap.

use std::cell::Cell;
use std::rc::Rc;

use rune::{El, Live, el};

use super::data::TabId;

/// What a reveal does now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Reveal {
    /// The active tab hasn't drawn yet.
    Pending,
    /// It shows clear of the fades.
    Clear,
    /// Scroll the viewport to this offset.
    To(f32),
}

/// The markers' rects, and what the strip was built with.
#[derive(Clone)]
pub(super) struct Probe {
    /// The strip root's left edge (an absolute marker at its origin).
    strip: Live<[f32; 4]>,
    /// Its right edge (one pinned to its top right).
    end: Live<[f32; 4]>,
    /// Tab 0's origin.
    first: Live<[f32; 4]>,
    /// The active tab's slot.
    active: Live<[f32; 4]>,
    /// The tab whose slot `active` marks, as last built.
    active_id: Rc<Cell<Option<TabId>>>,
    /// The fade over the viewport's edges, and what follows the viewport in the root (the New
    /// tab button and the gap before it), as last built.
    fade: Rc<Cell<f32>>,
    trailing: Rc<Cell<f32>>,
}

impl Default for Probe {
    fn default() -> Self {
        Probe {
            strip: Live::new([0.0; 4]),
            end: Live::new([0.0; 4]),
            first: Live::new([0.0; 4]),
            active: Live::new([0.0; 4]),
            active_id: Rc::new(Cell::new(None)),
            fade: Rc::new(Cell::new(0.0)),
            trailing: Rc::new(Cell::new(0.0)),
        }
    }
}

fn marker(key: &str) -> El {
    el().key(key).size(1.0, 1.0).a11y_hidden()
}

impl Probe {
    /// The markers for the strip's root: at its origin and its top right.
    pub(super) fn root_markers(&self) -> [El; 2] {
        [
            marker("probe-origin")
                .absolute(0.0, 0.0)
                .drawn_rect_into(&self.strip),
            marker("probe-end")
                .pin(None, Some(0.0), Some(0.0), None)
                .drawn_rect_into(&self.end),
        ]
    }

    /// The marker for tab 0's slot.
    pub(super) fn first_marker(&self) -> El {
        marker("probe-first")
            .absolute(0.0, 0.0)
            .drawn_rect_into(&self.first)
    }

    /// The marker for the active tab `id`'s slot, which fills it.
    pub(super) fn active_marker(&self, id: TabId) -> El {
        self.active_id.set(Some(id));
        el().key("probe-active")
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .a11y_hidden()
            .drawn_rect_into(&self.active)
    }

    /// What the strip was built with: its fade and the space after its viewport.
    pub(super) fn built(&self, fade: f32, trailing: f32, active: Option<TabId>) {
        self.fade.set(fade);
        self.trailing.set(trailing);
        if active.is_none() {
            self.active_id.set(None);
        }
    }

    /// How far the viewport has scrolled, in points.
    pub(super) fn offset(&self) -> f32 {
        self.strip.get()[0] - self.first.get()[0]
    }

    /// Where to scroll so the active tab `id` shows a fade's width clear of either edge (at an
    /// end of the content the viewport's scroll stops short, and that edge has no fade).
    pub(super) fn reveal(&self, id: TabId) -> Reveal {
        let [x, _, w, _] = self.active.get();
        if self.active_id.get() != Some(id) || w <= 0.0 {
            return Reveal::Pending;
        }
        let first = self.first.get()[0];
        let (left, right) = (x - first, x - first + w);
        let offset = self.offset();
        let view = self.end.get()[0] + 1.0 - self.strip.get()[0] - self.trailing.get();
        let margin = self.fade.get();
        let to = if left - margin < offset {
            left - margin
        } else if right + margin > offset + view {
            right + margin - view
        } else {
            return Reveal::Clear;
        };
        Reveal::To(to.max(0.0))
    }
}
