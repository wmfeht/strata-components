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
//! - a mostly sideways gesture is left to the viewport, which scrolls it itself, except in the
//!   frame a reveal's jump went out (`state.rs`), which it would undo;
//! - any wheel over a tab cancels a reveal still to land, so the user's scroll wins. A wheel over
//!   the gap between two tabs reaches no tab and doesn't.

use rune::{El, Tx, WheelEvent};

use super::probe::Probe;

/// `tab`, scrolling the strip's viewport `scroll` sideways under a vertical wheel.
pub(super) fn sideways<S: 'static>(tab: El, probe: &Probe, scroll: String) -> El {
    let probe = probe.clone();
    tab.on_wheel(move |_: &mut S, ev: WheelEvent, tx: &mut Tx| {
        // The user's scroll wins over a reveal still to land (`state.rs`), whichever way.
        probe.note_wheel();
        let sideways = ev.dy.abs() <= ev.dx.abs();
        // A sideways gesture is the viewport's, unless a reveal's jump went out this frame: the
        // jump would land after it and undo it, so this wheel jumps instead, from where the
        // screen shows the strip, and lands last (review 2).
        if sideways && !probe.revealing() {
            return false;
        }
        // A wheel turned down (content moving up, `dy` negative) shows what is to the right;
        // one moving content right (`dx` positive) shows what is to the left.
        let delta = if sideways { ev.dx } else { ev.dy };
        let to = (probe.offset() - delta).max(0.0);
        tx.scroll_to_instant(scroll.clone(), to, 0.0);
        true
    })
}
