//! [`OutsideWheel`]: an open panel's (Sort by, Appearance, Thumbnail size) outside layer, which
//! a wheel tick closes the panel through and then scrolls the listing under the pointer, as
//! Strata's `ui::scrolling::popover::dismiss_on_outside_scroll` does.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use rune::{El, Tx, WheelEvent, el};

use super::Target;
use crate::contract::Point;

type CloseFn<S> = Rc<dyn Fn(&mut S, &mut Tx)>;
type ListingsFn<S> = Rc<dyn Fn(&S, Point) -> Vec<Target>>;

/// The outside of an open panel: a press there closes it, and a wheel tick there closes it and
/// scrolls the listing under the pointer, and nothing else, by the tick's own step.
///
/// The host mounts [`OutsideWheel::scrim`] in its root layer under the panel, in place of the
/// panel's own outside-press scrim (rune-kit `Popover::on_close`'s), and wraps the panel's
/// surface in [`OutsideWheel::surface`]:
///
/// ```ignore
/// let outside = OutsideWheel::new(|s: &mut App, _| s.sort_open = false)
///     .listings(|s: &App, at| s.listing_targets_at(at)); // the List pane: its rows and its sideways scroll
/// layer.push(outside.scrim("sort-scrim"));
/// let panel = OutsideWheel::<App>::surface(panel_body);
/// ```
///
/// - Rune hands a wheel delta to the topmost element's `on_wheel` first, so the scrim hears
///   every tick outside the panel; it takes the tick, so nothing under it scrolls by itself.
/// - The listing the tick scrolls is whichever of the host's [`Target`]s are under the pointer
///   ([`OutsideWheel::listings`]): over the sidebar or the chrome the host gives none, and the
///   tick only closes the panel. Each target moves by the tick's delta on the axes it scrolls,
///   as Rune's wheel would have moved it, and with Shift held a vertical tick moves it
///   sideways (Strata swaps the axes then).
/// - The rest of a touchpad gesture whose first delta closed the panel scrolls as usual.
///
/// Used by: hosts (X1's and Strata's panels), the gallery's `c24-panel` scene.
pub struct OutsideWheel<S> {
    close: CloseFn<S>,
    listings: Option<ListingsFn<S>>,
}

impl<S> Clone for OutsideWheel<S> {
    fn clone(&self) -> Self {
        Self {
            close: self.close.clone(),
            listings: self.listings.clone(),
        }
    }
}

impl<S: 'static> OutsideWheel<S> {
    /// The outside of a panel that `close` closes.
    pub fn new(close: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        Self {
            close: Rc::new(close),
            listings: None,
        }
    }

    /// The listing viewports under a window point, which a tick there scrolls: none over the
    /// sidebar or chrome. Without it a tick only closes the panel.
    pub fn listings(mut self, f: impl Fn(&S, Point) -> Vec<Target> + 'static) -> Self {
        self.listings = Some(Rc::new(f));
        self
    }

    /// The layer over the window, under the panel, keyed `key`. Mount it in a root layer that
    /// fills the window from its top-left (its points are window points).
    pub fn scrim(&self, key: impl Into<String>) -> El {
        let (press, wheel) = (self.clone(), self.clone());
        el().key(key)
            .role("none")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .on_press_tx(move |s: &mut S, tx: &mut Tx| (press.close)(s, tx))
            .on_wheel(move |s: &mut S, e: WheelEvent, tx: &mut Tx| {
                wheel.tick(s, e, tx);
                true
            })
    }

    /// The panel's surface `panel`: a tick over it that no scroll viewport inside it takes goes
    /// no further, so a panel that doesn't scroll never scrolls the listing under it (Rune
    /// scrolls the innermost viewport at the pointer, under any overlay: draft U-C24b-2).
    pub fn surface(panel: El) -> El {
        panel.on_wheel(|_: &mut S, _: WheelEvent, tx: &mut Tx| {
            tx.live_only();
            true
        })
    }

    fn tick(&self, s: &mut S, e: WheelEvent, tx: &mut Tx) {
        let (mut dx, mut dy) = (e.dx, e.dy);
        if e.mods.shift && dx == 0.0 {
            (dx, dy) = (dy, 0.0);
        }
        let targets = self
            .listings
            .as_ref()
            .map(|f| f(s, Point::new(e.x, e.y)))
            .unwrap_or_default();
        (self.close)(s, tx);
        for t in targets {
            let p = t.feed.get();
            let along = |on: bool, i: usize, d: f32| {
                if on {
                    (p.offset[i] - d).clamp(0.0, p.max[i].max(0.0))
                } else {
                    p.offset[i]
                }
            };
            let (x, y) = (along(t.x, 0, dx), along(t.y, 1, dy));
            if (x, y) != (p.offset[0], p.offset[1]) {
                tx.scroll_to_instant(t.path.clone(), x, y);
            }
        }
    }
}
