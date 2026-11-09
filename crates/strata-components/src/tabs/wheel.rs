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
//! - the offset is read from where hidden markers drew (`probe.rs`, with its caveats);
//! - a mostly sideways gesture is left to the viewport, which scrolls it itself.

use rune::{El, Tx, WheelEvent};

use super::probe::Probe;

/// `tab`, scrolling the strip's viewport `scroll` sideways under a vertical wheel.
pub(super) fn sideways<S: 'static>(tab: El, probe: &Probe, scroll: String) -> El {
    let probe = probe.clone();
    tab.on_wheel(move |_: &mut S, ev: WheelEvent, tx: &mut Tx| {
        if ev.dy.abs() <= ev.dx.abs() {
            return false;
        }
        let offset = probe.offset();
        // A wheel turned down (content moving up, `dy` negative) shows what is to the right.
        let to = (offset - ev.dy).max(0.0);
        tx.scroll_to_instant(scroll.clone(), to, 0.0);
        true
    })
}
