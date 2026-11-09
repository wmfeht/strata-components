//! A vertical wheel over the tabs scrolls the strip sideways (`browser/tabs`: "When the strip
//! is wider than the window, the wheel scrolls it horizontally").
//!
//! Rune sends a vertical wheel only to a vertical viewport, so over a sideways-only strip it
//! scrolls nothing (U-C6-5), and it has no way to read a viewport's offset or scroll one
//! relatively (U-C7-2). So this is a workaround, and fragile:
//!
//! - every tab takes the wheel through `on_wheel` (from `TabItem::decorate`), which Rune asks
//!   before the strip's viewport (probe 2 in `strata-spikes/c7-probes`); a wheel over the gap
//!   between two tabs reaches none of them and scrolls nothing;
//! - the offset is read from where two hidden markers drew: one at the strip's origin (an
//!   absolute child of rune-kit's strip root, which starts its scroll viewport there) and one at
//!   tab 0's. It leans on rune-kit's layout: no padding before the viewport, and tab 0 first in
//!   its list. While a tab dragged in from elsewhere holds the gap before tab 0, the read is off
//!   by that gap;
//! - a mostly sideways gesture is left to the viewport, which scrolls it itself.

use rune::{El, Tx, WheelEvent, el};

use super::state::WheelProbe;

/// The marker at the strip's origin, for its root.
pub(super) fn strip_marker(probe: &WheelProbe) -> El {
    marker("wheel-origin").drawn_rect_into(&probe.strip)
}

/// The marker at tab 0's origin, for its slot.
pub(super) fn first_marker(probe: &WheelProbe) -> El {
    marker("wheel-first").drawn_rect_into(&probe.first)
}

fn marker(key: &str) -> El {
    el().key(key)
        .absolute(0.0, 0.0)
        .size(1.0, 1.0)
        .a11y_hidden()
}

/// `tab`, scrolling the strip's viewport `scroll` sideways under a vertical wheel.
pub(super) fn sideways<S: 'static>(tab: El, probe: &WheelProbe, scroll: String) -> El {
    let probe = probe.clone();
    tab.on_wheel(move |_: &mut S, ev: WheelEvent, tx: &mut Tx| {
        if ev.dy.abs() <= ev.dx.abs() {
            return false;
        }
        let offset = probe.strip.get()[0] - probe.first.get()[0];
        // A wheel turned down (content moving up, `dy` negative) shows what is to the right.
        let to = (offset - ev.dy).max(0.0);
        tx.scroll_to_instant(scroll.clone(), to, 0.0);
        true
    })
}
