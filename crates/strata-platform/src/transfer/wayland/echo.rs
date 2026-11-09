//! Whether a data-device selection write took.
//!
//! wlroots ignores `wl_data_device.set_selection` with a serial older than the current
//! selection's, silently: no error, no event (S2 Q5). A write that took is echoed to our own
//! device as a `selection` event, at once, while we have keyboard focus. So after a write made
//! with focus, P0 sends `wl_display.sync`: the compositor answers it after handling the write,
//! so a `done` with no `selection` event since means the write was dropped. Without focus no
//! echo comes either way, and the write isn't judged.

/// The writes waiting for their sync, by source id, each with whether a `selection` event has
/// arrived since.
#[derive(Debug, Default)]
pub(super) struct Echoes {
    waiting: Vec<(u64, bool)>,
}

impl Echoes {
    /// A write by `source` went out with a sync after it.
    pub(super) fn arm(&mut self, source: u64) {
        self.waiting.push((source, false));
    }

    /// The data device heard a `selection` event: every write waiting has had its answer.
    pub(super) fn selection(&mut self) {
        for (_, seen) in &mut self.waiting {
            *seen = true;
        }
    }

    /// The sync after `source`'s write came back: true when the write was dropped, as no
    /// `selection` event came between them.
    pub(super) fn done(&mut self, source: u64) -> bool {
        match self.waiting.iter().position(|(s, _)| *s == source) {
            Some(i) => !self.waiting.swap_remove(i).1,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests;
