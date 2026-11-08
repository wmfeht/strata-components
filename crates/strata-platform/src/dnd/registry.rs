//! A window's drop targets, by element key, and how a hit test's path finds one.
//!
//! Rune lets a shim neither read an element's key nor add a second handler of a kind (draft
//! U-P1-1, not filed), so targets are registered by the key the app gives
//! [`Dnd::drop_target`](super::Dnd::drop_target), and found from `Tx::element_at`'s path,
//! whose keyed segments name the element and its ancestors. A key must be unique in its
//! window. Entries are replaced each time the view builds the target; one whose element is
//! gone is never hit, and the oldest go once there are more than [`CAP`].

use std::collections::HashMap;

use rune::Live;

use super::api::DropTarget;

/// The most targets kept per window: well above what a view shows at once, so a target
/// built in a memoized subtree that hasn't been rebuilt in a while is still found.
pub(crate) const CAP: usize = 16_384;

pub(crate) struct Entry<S> {
    pub(crate) target: DropTarget<S>,
    /// Where the element was last drawn, `[x, y, w, h]` in window points.
    pub(crate) rect: Live<[f32; 4]>,
    stamp: u64,
}

pub(crate) struct Registry<S> {
    entries: HashMap<String, Entry<S>>,
    next: u64,
}

impl<S> Default for Registry<S> {
    fn default() -> Self {
        Registry {
            entries: HashMap::new(),
            next: 0,
        }
    }
}

impl<S: 'static> Registry<S> {
    /// Registers `target` for `key`; the rect the element reports into.
    pub(crate) fn put(&mut self, key: &str, target: DropTarget<S>) -> Live<[f32; 4]> {
        self.next += 1;
        let stamp = self.next;
        if let Some(e) = self.entries.get_mut(key) {
            e.target = target;
            e.stamp = stamp;
            return e.rect.clone();
        }
        let rect = Live::new([0.0; 4]);
        self.entries.insert(
            key.to_owned(),
            Entry {
                target,
                rect: rect.clone(),
                stamp,
            },
        );
        if self.entries.len() > CAP {
            self.evict();
        }
        rect
    }

    fn evict(&mut self) {
        let mut stamps: Vec<u64> = self.entries.values().map(|e| e.stamp).collect();
        stamps.sort_unstable();
        // Drop the oldest eighth at once, so a full registry doesn't sort on every put.
        let cut = stamps[stamps.len() / 8];
        self.entries.retain(|_, e| e.stamp > cut);
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Entry<S>> {
        self.entries.get(key)
    }

    /// The keys of `path` that have targets, innermost first.
    pub(crate) fn along<'p>(&self, path: &'p str) -> Vec<&'p str> {
        keyed_segments(path)
            .rev()
            .filter(|k| self.entries.contains_key(*k))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

/// A path's keyed segments, outermost first: `#n` stands for an element without a key.
fn keyed_segments(path: &str) -> impl DoubleEndedIterator<Item = &str> {
    path.split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
}

#[cfg(test)]
mod tests;
