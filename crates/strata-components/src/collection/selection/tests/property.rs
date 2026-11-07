//! Random operations, re-sorts, removals, inserts, renames, streamed reloads and restores,
//! checked step by step against a plain model of the same rules: membership at every index,
//! the count, the cursor, the anchor, the fill and every reported change. After every step
//! over a complete listing, the cursor and the anchor name listed entries or nothing.
//!
//! Deterministic: each seed replays the same run. A failure names its seed and the steps that
//! led to it.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

use super::*;

const SEEDS: u64 = 256;
const STEPS: usize = 200;
const MAX_LEN: usize = 48;

/// splitmix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number below `n`, which is at least 1.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// True `percent` times in a hundred.
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelGesture {
    None,
    Extend(Extend),
    Band(BandMode),
}

/// The rules, written the slow and obvious way.
#[derive(Clone, Debug)]
struct Model {
    sel: BTreeSet<EntryId>,
    cursor: Option<EntryId>,
    hint: Option<usize>,
    anchor: Option<EntryId>,
    provisional: bool,
    gesture: ModelGesture,
    /// The selection a gesture started from.
    base: BTreeSet<EntryId>,
}

impl Model {
    fn new() -> Self {
        Self {
            sel: BTreeSet::new(),
            cursor: None,
            hint: None,
            anchor: None,
            provisional: false,
            gesture: ModelGesture::None,
            base: BTreeSet::new(),
        }
    }

    fn place(&mut self, s: &ListSource, i: usize) {
        self.cursor = Some(s.id_at(i));
        self.hint = Some(i);
    }

    fn pick(&mut self, s: &ListSource, i: usize, provisional: bool) {
        let id = s.id_at(i);
        self.gesture = ModelGesture::None;
        self.sel = BTreeSet::from([id]);
        self.provisional = provisional;
        self.place(s, i);
        self.anchor = Some(id);
    }

    fn toggle(&mut self, s: &ListSource, i: usize) {
        let id = s.id_at(i);
        self.gesture = ModelGesture::None;
        self.provisional = false;
        if !self.sel.remove(&id) {
            self.sel.insert(id);
        }
        self.place(s, i);
        self.anchor = Some(id);
    }

    fn extend(&mut self, s: &ListSource, i: usize, mode: Extend) {
        let id = s.id_at(i);
        let a = match self.anchor.and_then(|a| s.index_of(a)) {
            Some(a) => a,
            None => {
                self.anchor = Some(id);
                i
            }
        };
        if self.gesture != ModelGesture::Extend(mode) {
            self.base = match mode {
                Extend::Replace => BTreeSet::new(),
                Extend::Add | Extend::Remove => self.sel.clone(),
            };
            self.gesture = ModelGesture::Extend(mode);
        }
        let range: BTreeSet<EntryId> = (a.min(i)..=a.max(i)).map(|j| s.id_at(j)).collect();
        self.sel = match mode {
            Extend::Replace => range,
            Extend::Add => self.base.union(&range).copied().collect(),
            Extend::Remove => self.base.difference(&range).copied().collect(),
        };
        self.provisional = false;
        self.place(s, i);
    }

    fn move_cursor(&mut self, s: &ListSource, i: usize, rule: AnchorRule) {
        let id = s.id_at(i);
        self.gesture = ModelGesture::None;
        if self.provisional && self.cursor != Some(id) {
            self.provisional = false;
            self.sel.clear();
        }
        self.place(s, i);
        if rule == AnchorRule::Move {
            self.anchor = Some(id);
        }
    }

    fn select_all(&mut self, s: &ListSource) {
        if s.is_empty() {
            return;
        }
        self.gesture = ModelGesture::None;
        self.sel = listed(s).collect();
        self.provisional = false;
        if self.cursor.is_none() {
            self.place(s, 0);
        }
        if self.anchor.is_none() {
            self.anchor = self.cursor;
        }
    }

    fn invert(&mut self, s: &ListSource) {
        self.gesture = ModelGesture::None;
        self.provisional = false;
        self.sel = listed(s).filter(|id| !self.sel.contains(id)).collect();
    }

    fn clear(&mut self) {
        self.gesture = ModelGesture::None;
        self.provisional = false;
        self.sel.clear();
        self.anchor = self.cursor;
    }

    fn discard_provisional(&mut self) {
        if self.provisional {
            self.gesture = ModelGesture::None;
            self.provisional = false;
            self.sel.clear();
        }
    }

    fn begin_band(&mut self, mode: BandMode) {
        if mode == BandMode::Replace {
            self.sel.clear();
        }
        self.base = self.sel.clone();
        self.gesture = ModelGesture::Band(mode);
        self.provisional = false;
    }

    fn band(&mut self, s: &ListSource, crossed: &[Range<usize>]) {
        let ModelGesture::Band(mode) = self.gesture else {
            unreachable!("the driver bands only inside a band");
        };
        let crossed: BTreeSet<EntryId> = crossed
            .iter()
            .flat_map(Clone::clone)
            .map(|i| s.id_at(i))
            .collect();
        self.sel = match mode {
            BandMode::Replace | BandMode::Add => self.base.union(&crossed).copied().collect(),
            BandMode::Toggle => self.base.symmetric_difference(&crossed).copied().collect(),
        };
    }

    fn end_band(&mut self) {
        if matches!(self.gesture, ModelGesture::Band(_)) {
            self.gesture = ModelGesture::None;
        }
    }

    fn cancel_band(&mut self) {
        if matches!(self.gesture, ModelGesture::Band(_)) {
            self.gesture = ModelGesture::None;
            self.sel = self.base.clone();
        }
    }

    fn sync(&mut self, s: &ListSource, rule: SyncRule) {
        if rule.listing == Listing::Loading {
            if let Some(i) = self.cursor.and_then(|c| s.index_of(c)) {
                self.hint = Some(i);
            }
            return;
        }
        let is_listed = |id: &EntryId| s.index_of(*id).is_some();
        self.sel.retain(is_listed);
        self.base.retain(is_listed);
        match self.cursor {
            Some(c) => match s.index_of(c) {
                Some(i) => self.hint = Some(i),
                None => {
                    self.provisional = false;
                    let n = s.len();
                    let neighbour = match rule.cursor_gone {
                        CursorGone::Clear => None,
                        _ if n == 0 => None,
                        _ => Some(self.hint.unwrap_or(0).min(n - 1)),
                    };
                    match neighbour {
                        Some(i) => {
                            self.place(s, i);
                            if rule.cursor_gone == CursorGone::NeighborSelected {
                                self.gesture = ModelGesture::None;
                                self.sel.insert(s.id_at(i));
                            }
                        }
                        None => (self.cursor, self.hint) = (None, None),
                    }
                }
            },
            None => self.hint = None,
        }
        if self.cursor.is_none() {
            self.anchor = None;
        } else if self.anchor.and_then(|a| s.index_of(a)).is_none() {
            self.anchor = self.cursor;
        }
    }

    fn relocate(&mut self, from: EntryId, to: EntryId) {
        self.gesture = ModelGesture::None;
        if self.sel.remove(&from) {
            self.sel.insert(to);
        }
        for held in [&mut self.cursor, &mut self.anchor] {
            if *held == Some(from) {
                *held = Some(to);
            }
        }
    }

    fn restore(&mut self, p: &Position) {
        self.gesture = ModelGesture::None;
        self.sel = p.selected.iter().copied().collect();
        self.provisional =
            p.provisional && self.sel.len() == 1 && p.cursor.is_some_and(|c| self.sel.contains(&c));
        self.cursor = p.cursor;
        self.hint = p.cursor.and(p.cursor_index);
        self.anchor = p.anchor.and(p.cursor);
    }

    fn selected_in_order(&self, s: &ListSource) -> Vec<EntryId> {
        listed(s).filter(|id| self.sel.contains(id)).collect()
    }

    fn targets(&self, s: &ListSource) -> Targets {
        let filled = self.selected_in_order(s);
        if !self.provisional && !filled.is_empty() {
            return Targets::Filled(filled);
        }
        match self.cursor {
            Some(c) if s.index_of(c).is_some() => Targets::Cursor(c),
            _ => Targets::Nothing,
        }
    }

    fn position(&self, s: &ListSource) -> Position {
        Position {
            cursor: self.cursor,
            cursor_index: self.cursor.and_then(|c| s.index_of(c)).or(self.hint),
            anchor: self.anchor,
            selected: self.selected_in_order(s),
            provisional: self.provisional,
            scroll: None,
        }
    }
}

fn listed(s: &ListSource) -> impl Iterator<Item = EntryId> + '_ {
    (0..s.len()).map(|i| s.id_at(i))
}

/// The model's change between two states, as [`Changed`] reports it.
fn diff(before: &Model, after: &Model) -> Changed {
    Changed {
        selection: before.sel != after.sel || before.provisional != after.provisional,
        cursor: before.cursor != after.cursor,
        anchor: before.anchor != after.anchor,
    }
}

struct Run {
    rng: Rng,
    s: ListSource,
    sel: Selection,
    model: Model,
    next_id: u64,
    saved: Vec<Position>,
    log: Rc<RefCell<Vec<String>>>,
    /// Whether the source holds its whole listing (not mid-reload).
    complete: bool,
}

impl Run {
    fn new(seed: u64, log: Rc<RefCell<Vec<String>>>) -> Self {
        let mut rng = Rng(seed);
        let n = rng.below(25) as u64;
        Self {
            rng,
            s: ListSource::new((0..n).map(entry).collect()),
            sel: Selection::new(),
            model: Model::new(),
            next_id: n,
            saved: Vec::new(),
            log,
            complete: true,
        }
    }

    fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id + 1_000
    }

    /// Applies `op` to both, then checks they agree and that the reported change is the
    /// model's (or, when `exact` is false, covers it).
    fn op(
        &mut self,
        what: String,
        exact: bool,
        f: impl FnOnce(&mut Selection, &mut Model, &ListSource) -> Changed,
    ) {
        self.log.borrow_mut().push(what);
        let before = self.model.clone();
        let revision = self.sel.revision();
        let changed = f(&mut self.sel, &mut self.model, &self.s);
        let expected = diff(&before, &self.model);
        if exact {
            assert_eq!(changed, expected, "reported change");
        } else {
            assert!(
                (!expected.selection || changed.selection)
                    && (!expected.cursor || changed.cursor)
                    && (!expected.anchor || changed.anchor),
                "reported change {changed:?} misses {expected:?}"
            );
        }
        let bumped = self.sel.revision() != revision;
        assert_eq!(
            bumped,
            changed.any(),
            "revision follows the reported change"
        );
        self.check();
    }

    fn check(&self) {
        let (s, sel, m) = (&self.s, &self.sel, &self.model);
        for i in 0..s.len() {
            let id = s.id_at(i);
            assert_eq!(
                sel.is_selected_at(s, i),
                m.sel.contains(&id),
                "entry {id:?} at {i}"
            );
        }
        for &id in &m.sel {
            assert!(sel.contains(s, id), "{id:?} selected");
        }
        assert_eq!(sel.len(), m.sel.len(), "count");
        assert_eq!(sel.cursor(), m.cursor, "cursor");
        assert_eq!(sel.anchor(), m.anchor, "anchor");
        assert_eq!(sel.is_provisional(), m.provisional, "provisional");
        assert_eq!(sel.selected_ids(s), m.selected_in_order(s), "selected ids");
        assert_eq!(sel.targets(s), m.targets(s), "targets");
        if !self.complete {
            return;
        }
        let is_listed = |id: EntryId| s.index_of(id).is_some();
        assert!(sel.cursor().is_none_or(is_listed), "cursor on a missing id");
        assert!(sel.anchor().is_none_or(is_listed), "anchor on a missing id");
        assert!(
            sel.anchor().is_none() || sel.cursor().is_some(),
            "anchor without cursor"
        );
        assert!(
            m.sel.iter().all(|&id| is_listed(id)),
            "selected id not listed"
        );
        if sel.is_provisional() {
            assert_eq!(
                m.sel.iter().copied().collect::<Vec<_>>(),
                Vec::from_iter(m.cursor)
            );
        }
    }

    fn index(&mut self) -> Option<usize> {
        (!self.s.is_empty()).then(|| self.rng.below(self.s.len()))
    }

    fn random_op(&mut self) {
        let in_band = matches!(self.model.gesture, ModelGesture::Band(_));
        match self.rng.below(16) {
            0 | 1 => {
                if let Some(i) = self.index() {
                    self.op(format!("select_only {i}"), true, |sel, m, s| {
                        m.pick(s, i, false);
                        sel.select_only(s, i)
                    });
                }
            }
            2 => {
                if let Some(i) = self.index() {
                    self.op(format!("auto_select {i}"), true, |sel, m, s| {
                        m.pick(s, i, true);
                        sel.auto_select(s, i)
                    });
                }
            }
            3 | 4 => {
                if let Some(i) = self.index() {
                    self.op(format!("toggle {i}"), true, |sel, m, s| {
                        m.toggle(s, i);
                        sel.toggle(s, i)
                    });
                }
            }
            5..=7 => {
                if let Some(i) = self.index() {
                    let mode = self
                        .rng
                        .pick(&[Extend::Replace, Extend::Add, Extend::Remove]);
                    self.op(format!("extend_to {i} {mode:?}"), true, |sel, m, s| {
                        m.extend(s, i, mode);
                        sel.extend_to(s, i, mode)
                    });
                }
            }
            8 => {
                if let Some(i) = self.index() {
                    let rule = self.rng.pick(&[AnchorRule::Keep, AnchorRule::Move]);
                    self.op(format!("move_cursor {i} {rule:?}"), true, |sel, m, s| {
                        m.move_cursor(s, i, rule);
                        sel.move_cursor(s, i, rule)
                    });
                }
            }
            9 => self.op("select_all".into(), true, |sel, m, s| {
                m.select_all(s);
                sel.select_all(s)
            }),
            10 => self.op("invert".into(), true, |sel, m, s| {
                m.invert(s);
                sel.invert(s)
            }),
            11 => {
                if self.rng.chance(50) {
                    self.op("clear".into(), true, |sel, m, _| {
                        m.clear();
                        sel.clear()
                    });
                } else {
                    self.op("discard_provisional".into(), true, |sel, m, _| {
                        m.discard_provisional();
                        sel.discard_provisional()
                    });
                }
            }
            12 | 13 if in_band => match self.rng.below(5) {
                0 => self.op("end_band".into(), true, |sel, m, _| {
                    m.end_band();
                    sel.end_band();
                    Changed::NONE
                }),
                1 => self.op("cancel_band".into(), true, |sel, m, _| {
                    m.cancel_band();
                    sel.cancel_band()
                }),
                _ => {
                    let crossed = self.random_ranges();
                    let set = crossed
                        .iter()
                        .cloned()
                        .fold(IndexRanges::new(), |mut r, x| {
                            r.push(x);
                            r
                        });
                    self.op(format!("band {crossed:?}"), true, |sel, m, s| {
                        m.band(s, &crossed);
                        sel.band(s, &set)
                    });
                }
            },
            12 | 13 => {
                let mode = self
                    .rng
                    .pick(&[BandMode::Replace, BandMode::Toggle, BandMode::Add]);
                self.op(format!("begin_band {mode:?}"), true, |sel, m, _| {
                    m.begin_band(mode);
                    sel.begin_band(mode)
                });
            }
            14 => {
                let p = self.sel.position(&self.s);
                assert_eq!(p, self.model.position(&self.s), "position");
                self.log.borrow_mut().push("save position".into());
                self.saved.push(p);
            }
            _ => {
                if !self.saved.is_empty() && self.complete {
                    let p = self.saved[self.rng.below(self.saved.len())].clone();
                    // Until the sync below, the restored ids may name entries gone since.
                    self.complete = false;
                    self.op("restore".into(), false, |sel, m, _| {
                        m.restore(&p);
                        sel.restore(&p)
                    });
                    self.sync(Listing::Complete);
                }
            }
        }
    }

    /// Ascending, disjoint ranges within the source.
    fn random_ranges(&mut self) -> Vec<Range<usize>> {
        let n = self.s.len();
        let mut out = Vec::new();
        let mut at = 0;
        while at < n && out.len() < 3 {
            let start = at + self.rng.below(n - at + 1);
            if start >= n {
                break;
            }
            let end = start + 1 + self.rng.below(n - start);
            out.push(start..end);
            at = end + 1;
        }
        out
    }

    fn sync(&mut self, listing: Listing) {
        let cursor_gone = self.rng.pick(&[
            CursorGone::Neighbor,
            CursorGone::NeighborSelected,
            CursorGone::Clear,
        ]);
        let rule = SyncRule {
            listing,
            cursor_gone,
        };
        self.complete = listing == Listing::Complete;
        self.op(format!("sync {rule:?}"), true, |sel, m, s| {
            m.sync(s, rule);
            sel.sync(s, rule)
        });
    }

    fn random_change(&mut self) {
        let mut entries = self.s.entries().to_vec();
        match self.rng.below(5) {
            0 => {
                let keys: HashMap<EntryId, u64> =
                    entries.iter().map(|e| (e.id, self.rng.next())).collect();
                self.log.borrow_mut().push("shuffle".into());
                self.s.sort_by(|a, b| keys[&a.id].cmp(&keys[&b.id]));
            }
            1 => {
                let percent = self.rng.pick(&[10, 30, 100]);
                let gone: Vec<bool> = entries.iter().map(|_| self.rng.chance(percent)).collect();
                let mut i = 0;
                entries.retain(|_| {
                    i += 1;
                    !gone[i - 1]
                });
                self.log
                    .borrow_mut()
                    .push(format!("remove to {}", entries.len()));
                self.s.replace(entries);
            }
            2 => {
                let count = 1 + self.rng.below(4);
                for _ in 0..count {
                    if entries.len() < MAX_LEN {
                        let at = self.rng.below(entries.len() + 1);
                        let raw = self.fresh_id();
                        entries.insert(at, entry(raw));
                    }
                }
                self.log
                    .borrow_mut()
                    .push(format!("insert to {}", entries.len()));
                self.s.replace(entries);
            }
            3 if !entries.is_empty() => {
                let at = self.rng.below(entries.len());
                let from = entries[at].id;
                let to = EntryId::new(self.fresh_id());
                entries[at] = entry(to.get());
                self.s.replace(entries);
                self.op(format!("relocate {from:?} -> {to:?}"), true, |sel, m, _| {
                    m.relocate(from, to);
                    sel.relocate(from, to)
                });
            }
            _ => {
                self.reload();
                return;
            }
        }
        self.sync(Listing::Complete);
    }

    /// A streamed reload: the listing empties, comes back in batches (with operations in
    /// between), and ends without some of its entries.
    fn reload(&mut self) {
        let mut full = self.s.entries().to_vec();
        full.retain(|_| !self.rng.chance(20));
        self.log
            .borrow_mut()
            .push(format!("reload to {}", full.len()));
        self.s.replace(Vec::new());
        self.sync(Listing::Loading);
        let mut shown = 0;
        while shown < full.len() {
            shown += 1 + self.rng.below(full.len() - shown);
            self.s.replace(full[..shown].to_vec());
            self.sync(Listing::Loading);
            for _ in 0..self.rng.below(3) {
                self.random_op();
            }
        }
        self.s.replace(full);
        self.sync(Listing::Complete);
    }

    fn step(&mut self) {
        if self.rng.chance(25) {
            self.random_change();
        } else {
            self.random_op();
        }
    }
}

#[test]
fn random_operations_keep_cursor_and_anchor_on_listed_entries() {
    for seed in 0..SEEDS {
        let log = Rc::new(RefCell::new(Vec::new()));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut run = Run::new(seed, Rc::clone(&log));
            run.check();
            for _ in 0..STEPS {
                run.step();
            }
        }));
        if let Err(panic) = result {
            let log = log.borrow();
            let mut steps = String::new();
            for line in log.iter().rev().take(30).rev() {
                let _ = writeln!(steps, "  {line}");
            }
            eprintln!("seed {seed} failed; its last steps:\n{steps}");
            resume_unwind(panic);
        }
    }
}
