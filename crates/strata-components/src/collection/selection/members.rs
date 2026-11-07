//! Which ids are selected: an id set plus an optional overlay of index ranges.
//!
//! A range operation over 100,000 entries can't afford to hash every id it covers (that alone
//! takes over a millisecond). So a range lands as an overlay: the index ranges it covers, with a
//! plain copy of their ids. While the source keeps its order, membership is a range check on
//! the index. [`Members::freeze`] (on every sync) hashes the copied ids, because after a re-sort
//! or a live update the indices no longer name the same entries, and the copy is all that
//! still says which entries the range held.

use std::collections::HashSet;
use std::hash::{BuildHasherDefault, Hasher};

use crate::contract::{EntryId, EntrySource, IndexRanges};

/// A set of entry ids, hashed with [`IdHasher`].
pub(super) type IdSet = HashSet<EntryId, BuildHasherDefault<IdHasher>>;

/// A hasher for [`EntryId`]s: splitmix64's finaliser over the raw value. Hosts often number
/// ids in sequence, which the standard SipHash handles well but slowly; this mixes as well for
/// a single `u64` at a fraction of the cost.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // EntryId hashes through `write_u64`; this only runs for other keys, byte by byte.
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_u64(&mut self, x: u64) {
        let mut z = (self.0 ^ x).wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        self.0 = z ^ (z >> 31);
    }
}

/// How an overlay combines with the base set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Op {
    /// Selected if in the base or the overlay.
    Add,
    /// Selected if in the base and not the overlay.
    Remove,
    /// Selected if in exactly one of them.
    Toggle,
}

impl Op {
    fn apply(self, in_base: bool, in_overlay: bool) -> bool {
        match self {
            Self::Add => in_base || in_overlay,
            Self::Remove => in_base && !in_overlay,
            Self::Toggle => in_base != in_overlay,
        }
    }
}

#[derive(Clone, Debug)]
struct Overlay {
    op: Op,
    ids: OverlayIds,
}

#[derive(Clone, Debug)]
enum OverlayIds {
    /// Index ranges into the source as it was when they were captured, with their ids in
    /// index order. Valid until the source changes, which a sync marks.
    Live {
        ranges: IndexRanges,
        ids: Vec<EntryId>,
        source_len: usize,
    },
    /// The ids alone, after the source changed.
    Frozen(IdSet),
}

impl Overlay {
    fn contains_at<S: EntrySource + ?Sized>(&self, source: &S, index: usize, id: EntryId) -> bool {
        match &self.ids {
            OverlayIds::Live {
                ranges, source_len, ..
            } => {
                debug_assert_eq!(
                    source.len(),
                    *source_len,
                    "the source changed without a Selection::sync"
                );
                ranges.contains(index)
            }
            OverlayIds::Frozen(set) => set.contains(&id),
        }
    }

    fn contains<S: EntrySource + ?Sized>(&self, source: &S, id: EntryId) -> bool {
        match &self.ids {
            OverlayIds::Live { ranges, .. } => {
                source.index_of(id).is_some_and(|i| ranges.contains(i))
            }
            OverlayIds::Frozen(set) => set.contains(&id),
        }
    }

    fn for_each_id(&self, mut f: impl FnMut(EntryId)) {
        match &self.ids {
            OverlayIds::Live { ids, .. } => ids.iter().copied().for_each(&mut f),
            OverlayIds::Frozen(set) => set.iter().copied().for_each(&mut f),
        }
    }
}

/// The selected ids: `base`, combined with `overlay` by its [`Op`].
#[derive(Clone, Debug, Default)]
pub(super) struct Members {
    base: IdSet,
    overlay: Option<Overlay>,
    /// How many ids are selected, kept exact by every change.
    len: usize,
}

impl Members {
    pub(super) fn len(&self) -> usize {
        self.len
    }

    /// Whether the entry at `index` is selected.
    pub(super) fn contains_at<S: EntrySource + ?Sized>(&self, source: &S, index: usize) -> bool {
        let id = source.id_at(index);
        let in_base = !self.base.is_empty() && self.base.contains(&id);
        match &self.overlay {
            None => in_base,
            Some(ov) => ov.op.apply(in_base, ov.contains_at(source, index, id)),
        }
    }

    /// Whether `id` is selected. An id the source doesn't list can still be selected while a
    /// listing loads.
    pub(super) fn contains<S: EntrySource + ?Sized>(&self, source: &S, id: EntryId) -> bool {
        let in_base = self.base.contains(&id);
        match &self.overlay {
            None => in_base,
            Some(ov) => ov.op.apply(in_base, ov.contains(source, id)),
        }
    }

    /// Whether the overlay holds index ranges into the source as it is.
    pub(super) fn is_live(&self) -> bool {
        matches!(
            self.overlay,
            Some(Overlay {
                ids: OverlayIds::Live { .. },
                ..
            })
        )
    }

    /// Selects nothing. Returns whether anything was selected.
    pub(super) fn clear(&mut self) -> bool {
        let changed = self.len > 0;
        self.base.clear();
        self.overlay = None;
        self.len = 0;
        changed
    }

    /// Selects only `id`. Returns whether that changed anything.
    pub(super) fn set_single<S: EntrySource + ?Sized>(&mut self, source: &S, id: EntryId) -> bool {
        let changed = !(self.len == 1 && self.contains(source, id));
        self.base.clear();
        self.base.insert(id);
        self.overlay = None;
        self.len = 1;
        changed
    }

    /// Folds the overlay into the base, so the base alone says what is selected.
    pub(super) fn commit(&mut self) {
        let Some(ov) = self.overlay.take() else {
            return;
        };
        let base = &mut self.base;
        ov.for_each_id(|id| match ov.op {
            Op::Add => {
                base.insert(id);
            }
            Op::Remove => {
                base.remove(&id);
            }
            Op::Toggle => {
                if !base.remove(&id) {
                    base.insert(id);
                }
            }
        });
        debug_assert_eq!(self.base.len(), self.len, "selection count drifted");
        self.len = self.base.len();
    }

    /// Flips `id` in or out.
    pub(super) fn flip(&mut self, id: EntryId) {
        self.commit();
        if !self.base.remove(&id) {
            self.base.insert(id);
        }
        self.len = self.base.len();
    }

    /// Adds `id`. Returns whether it wasn't selected before.
    pub(super) fn insert(&mut self, id: EntryId) -> bool {
        self.commit();
        let added = self.base.insert(id);
        self.len = self.base.len();
        added
    }

    /// Moves `from`'s membership to `to`. Returns whether `from` was selected.
    pub(super) fn relocate(&mut self, from: EntryId, to: EntryId) -> bool {
        self.commit();
        let moved = self.base.remove(&from);
        if moved {
            self.base.insert(to);
        }
        self.len = self.base.len();
        moved
    }

    /// Replaces the overlay with `ranges`, combined by `op` with the base, or with nothing if
    /// `clear_base`. Returns whether the selected set changed.
    ///
    /// Callers fold an overlay from another gesture into the base first ([`Members::commit`]):
    /// the overlay this replaces belongs to the same gesture, so the base is that gesture's
    /// starting point.
    pub(super) fn set_ranges<S: EntrySource + ?Sized>(
        &mut self,
        source: &S,
        clear_base: bool,
        op: Op,
        ranges: IndexRanges,
    ) -> bool {
        let mut ids = Vec::with_capacity(ranges.count());
        ids.extend(ranges.indices().map(|i| source.id_at(i)));

        let base_after = !clear_base && !self.base.is_empty();
        let new_len = if base_after {
            let inside = ids.iter().filter(|id| self.base.contains(id)).count();
            match op {
                Op::Add => self.base.len() + (ids.len() - inside),
                Op::Remove => self.base.len() - inside,
                Op::Toggle => self.base.len() + (ids.len() - inside) - inside,
            }
        } else {
            match op {
                Op::Add | Op::Toggle => ids.len(),
                Op::Remove => 0,
            }
        };

        let changed = match self.delta_since_live(source, op, clear_base, &ranges) {
            Some(changed) => changed,
            None if new_len != self.len => true,
            None => {
                // Same count: the sets differ only if some new member wasn't selected before.
                let new_contains = |id: &EntryId| {
                    let in_base = base_after && self.base.contains(id);
                    let in_ranges = source.index_of(*id).is_some_and(|i| ranges.contains(i));
                    op.apply(in_base, in_ranges)
                };
                let base_ids = self.base.iter().filter(|_| base_after);
                base_ids
                    .chain(ids.iter())
                    .any(|id| new_contains(id) && !self.contains(source, *id))
            }
        };

        if clear_base {
            self.base.clear();
        }
        self.overlay = (!ids.is_empty()).then(|| Overlay {
            op,
            ids: OverlayIds::Live {
                ranges,
                ids,
                source_len: source.len(),
            },
        });
        self.len = new_len;
        changed
    }

    /// Whether replacing the live overlay with `ranges` changes the set, worked out from the
    /// indices the two cover differently: `None` when there's no live overlay of the same `op`
    /// over the same base to compare with.
    fn delta_since_live<S: EntrySource + ?Sized>(
        &self,
        source: &S,
        op: Op,
        clear_base: bool,
        ranges: &IndexRanges,
    ) -> Option<bool> {
        let Some(Overlay {
            op: old_op,
            ids: OverlayIds::Live {
                ranges: old_ranges, ..
            },
        }) = &self.overlay
        else {
            return None;
        };
        if *old_op != op || (clear_base && !self.base.is_empty()) {
            return None;
        }
        let delta = symmetric_difference(old_ranges, ranges);
        let in_base = |i: usize| self.base.contains(&source.id_at(i));
        Some(match op {
            Op::Toggle => !delta.is_empty(),
            Op::Add => delta.indices().any(|i| !in_base(i)),
            Op::Remove => !self.base.is_empty() && delta.indices().any(in_base),
        })
    }

    /// Drops the overlay, leaving the base. Returns whether the selected set changed.
    pub(super) fn discard_overlay(&mut self) -> bool {
        let Some(ov) = self.overlay.take() else {
            return false;
        };
        let mut changed = false;
        ov.for_each_id(|id| {
            let in_base = self.base.contains(&id);
            changed |= ov.op.apply(in_base, true) != in_base;
        });
        self.len = self.base.len();
        changed
    }

    /// Selects exactly the listed entries that aren't selected now. Returns whether that
    /// changed anything.
    pub(super) fn invert<S: EntrySource + ?Sized>(&mut self, source: &S) -> bool {
        let mut ranges = IndexRanges::new();
        let mut ids = Vec::new();
        for i in 0..source.len() {
            if !self.contains_at(source, i) {
                ranges.push(i..i + 1);
                ids.push(source.id_at(i));
            }
        }
        let changed = source.len() > 0 || self.len > 0;
        self.base.clear();
        self.len = ids.len();
        self.overlay = (!ids.is_empty()).then(|| Overlay {
            op: Op::Add,
            ids: OverlayIds::Live {
                ranges,
                ids,
                source_len: source.len(),
            },
        });
        changed
    }

    /// Turns a live overlay's index ranges into its ids, because the source is changing.
    pub(super) fn freeze(&mut self) {
        if let Some(ov) = &mut self.overlay
            && let OverlayIds::Live { ids, .. } = &mut ov.ids
        {
            let mut set = IdSet::with_capacity_and_hasher(ids.len(), Default::default());
            set.extend(ids.iter().copied());
            ov.ids = OverlayIds::Frozen(set);
        }
    }

    /// Drops every id the source doesn't list. Returns whether any was selected.
    pub(super) fn retain_listed<S: EntrySource + ?Sized>(&mut self, source: &S) -> bool {
        self.freeze();
        let listed = |id: &EntryId| source.index_of(*id).is_some();
        self.base.retain(listed);
        if let Some(Overlay {
            ids: OverlayIds::Frozen(set),
            ..
        }) = &mut self.overlay
        {
            set.retain(listed);
        }
        let before = self.len;
        self.recount();
        self.len != before
    }

    fn recount(&mut self) {
        self.len = match &self.overlay {
            None => self.base.len(),
            Some(ov) => {
                let (mut inside, mut outside) = (0, 0);
                ov.for_each_id(|id| {
                    if self.base.contains(&id) {
                        inside += 1;
                    } else {
                        outside += 1;
                    }
                });
                match ov.op {
                    Op::Add => self.base.len() + outside,
                    Op::Remove => self.base.len() - inside,
                    Op::Toggle => self.base.len() + outside - inside,
                }
            }
        };
    }

    /// The selected ids the source lists, in its order.
    pub(super) fn listed_in_order<S: EntrySource + ?Sized>(&self, source: &S) -> Vec<EntryId> {
        if self.len == 0 {
            return Vec::new();
        }
        let n = source.len();
        // A few ids among many entries: sort them by index rather than scan the source.
        if self.overlay.is_none() && self.len.saturating_mul(16) < n {
            let mut found: Vec<(usize, EntryId)> = self
                .base
                .iter()
                .filter_map(|&id| source.index_of(id).map(|i| (i, id)))
                .collect();
            found.sort_unstable_by_key(|&(i, _)| i);
            return found.into_iter().map(|(_, id)| id).collect();
        }
        (0..n)
            .filter(|&i| self.contains_at(source, i))
            .map(|i| source.id_at(i))
            .collect()
    }

    /// Whether the selection is exactly `ids`, or `None` when that needs the source (a live
    /// overlay).
    pub(super) fn equals(&self, ids: &IdSet) -> Option<bool> {
        if self.is_live() {
            return None;
        }
        if self.len != ids.len() {
            return Some(false);
        }
        let contains = |id: &EntryId| {
            let in_base = self.base.contains(id);
            match &self.overlay {
                None => in_base,
                Some(ov) => {
                    let in_overlay = matches!(&ov.ids, OverlayIds::Frozen(set) if set.contains(id));
                    ov.op.apply(in_base, in_overlay)
                }
            }
        };
        Some(ids.iter().all(contains))
    }

    /// Selects exactly `ids`.
    pub(super) fn set_ids(&mut self, ids: IdSet) {
        self.len = ids.len();
        self.base = ids;
        self.overlay = None;
    }
}

/// The indices in exactly one of `a` and `b`.
fn symmetric_difference(a: &IndexRanges, b: &IndexRanges) -> IndexRanges {
    let mut points: Vec<usize> = a
        .ranges()
        .iter()
        .chain(b.ranges())
        .flat_map(|r| [r.start, r.end])
        .collect();
    points.sort_unstable();
    points.dedup();
    let mut out = IndexRanges::new();
    for w in points.windows(2) {
        if a.contains(w[0]) != b.contains(w[0]) {
            out.push(w[0]..w[1]);
        }
    }
    out
}
