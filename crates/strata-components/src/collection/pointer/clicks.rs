//! Click runs: which click of a pair a press is, timed by the settings' interval rather than
//! Rune's fixed one.

use super::machine::Button;
use crate::contract::{EntryId, Point};

/// The last press, for the next one to pair with.
#[derive(Clone, Copy, Debug)]
struct Last {
    button: Button,
    target: Option<EntryId>,
    window: Point,
    at: f64,
    count: u32,
}

/// Counts presses into pairs, as GTK does for Strata: a press is the second of a pair when it
/// comes within the interval and the distance of a first press with the same button on the
/// same entry. Window coordinates, so a press stays paired while the content scrolls under it
/// (Strata's `columns/double_click.rs`). A pair ends the run: a third press starts a new one.
#[derive(Clone, Debug, Default)]
pub(super) struct Clicks {
    last: Option<Last>,
}

impl Clicks {
    /// Counts a press: 1, or 2 when it completes a pair.
    pub(super) fn press(
        &mut self,
        button: Button,
        target: Option<EntryId>,
        window: Point,
        now: f64,
        interval: f64,
        distance: f32,
    ) -> u32 {
        let pairs = self.last.is_some_and(|l| {
            l.count == 1
                && l.button == button
                && l.target == target
                && now - l.at < interval
                && (window.x - l.window.x).abs() <= distance
                && (window.y - l.window.y).abs() <= distance
        });
        let count = if pairs { 2 } else { 1 };
        self.last = Some(Last {
            button,
            target,
            window,
            at: now,
            count,
        });
        count
    }

    /// The last press dragged or was cancelled: only clicks pair, so it is no half of one.
    pub(super) fn break_run(&mut self) {
        self.last = None;
    }
}
