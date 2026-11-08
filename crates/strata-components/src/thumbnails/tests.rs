//! The store and its queue: eviction order, the budget, what is protected, request order
//! after scrolling each way, cancellation, failures, stops, the on/off switch, relocation,
//! steps and pruning; and in a headless runtime, that thumbnails land with no view run.

mod perf;
mod runtime;

use super::*;
use crate::contract::{EntryId, EntryKind, ListEntry, ListSource, ThumbKey};

const S4: Step = Step::new(4);
const S8: Step = Step::new(8);

/// `n` image entries keyed `1000 + i`, every entry index in `folders` a folder with no key.
fn source(n: usize, folders: &[usize]) -> ListSource {
    ListSource::new(
        (0..n)
            .map(|i| {
                let id = EntryId::new(i as u64 + 1);
                if folders.contains(&i) {
                    ListEntry::new(id, format!("folder {i}"), EntryKind::Folder, "folder")
                } else {
                    let mut e =
                        ListEntry::new(id, format!("img {i}"), EntryKind::File, "file_image");
                    e.thumbnail = Some(key(i));
                    e
                }
            })
            .collect(),
    )
}

fn key(i: usize) -> ThumbKey {
    ThumbKey::new(1000 + i as u64)
}

fn index(k: ThumbKey) -> usize {
    (k.get() - 1000) as usize
}

fn store(budget: usize) -> ThumbnailStore {
    ThumbnailStore::new(Settings {
        budget_bytes: budget,
        ..Settings::default()
    })
}

fn image(step: Step) -> Outcome {
    let px = step.px();
    Outcome::Ready {
        width: px,
        height: px,
        rgba: vec![255; (px * px * 4) as usize],
    }
}

/// Pops every request, in order, completing each with an image at its step.
fn serve(st: &mut ThumbnailStore, now: f64) -> Vec<usize> {
    let mut order = Vec::new();
    while let Some(r) = st.pop(now) {
        order.push(index(r.key));
        st.complete(r.ticket, image(r.step), now);
    }
    order
}

/// Pops every request without completing any.
fn drain(st: &mut ThumbnailStore, now: f64) -> Vec<Request> {
    std::iter::from_fn(|| st.pop(now)).collect()
}

fn ready(st: &ThumbnailStore, i: usize) -> bool {
    matches!(st.state(key(i)), State::Ready { .. })
}

fn view(r: std::ops::Range<usize>) -> Viewport {
    Viewport::new(r, S4)
}

#[test]
fn eviction_takes_the_least_recently_shown_first() {
    // Room for four 4 px images; no margin, so only what is shown loads.
    let mut st = ThumbnailStore::new(Settings {
        budget_bytes: 4 * 64,
        margin: 0.0,
        ..Settings::default()
    });
    let src = source(20, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..2));
    serve(&mut st, 0.0);
    st.sync(v, &src, view(2..4));
    serve(&mut st, 0.0);
    // 0..4 held. Building row 0 again (drawn) makes it the most recent.
    let _ = st.thumb(key(0));
    st.sync(v, &src, view(4..5));
    st.sync(v, &src, view(5..6));
    serve(&mut st, 0.0);
    // Protected: 4 (previous sync) and 5. One had to go: 1, the least recently shown.
    assert!(!ready(&st, 1), "1 evicted first");
    assert!(ready(&st, 0) && ready(&st, 2) && ready(&st, 3));
    st.sync(v, &src, view(6..7));
    serve(&mut st, 0.0);
    assert!(!ready(&st, 2), "then 2");
    assert!(ready(&st, 0), "row 0 was drawn after 2 and 3");
}

#[test]
fn an_evicted_image_empties_its_handle_in_place_and_shows_the_icon() {
    let mut st = ThumbnailStore::new(Settings {
        budget_bytes: 64,
        margin: 0.0,
        ..Settings::default()
    });
    let src = source(4, &[]);
    let v = st.viewer();
    let before = st.thumb(key(0));
    st.sync(v, &src, view(0..1));
    serve(&mut st, 0.0);
    assert_eq!(before.pixels().size(), (4, 4));
    assert_eq!(
        before.icon_alpha().get(),
        0.0,
        "the icon fades under a held image"
    );
    st.sync(v, &src, view(1..2));
    st.sync(v, &src, view(2..3));
    serve(&mut st, 0.0);
    let after = st.thumb(key(0));
    assert_eq!(after.epoch(), before.epoch(), "the same handles");
    assert_eq!(before.pixels().size(), (0, 0), "emptied in place");
    assert_eq!(before.icon_alpha().get(), 1.0);
}

#[test]
fn the_budget_is_never_exceeded_unless_rows_on_screen_alone_exceed_it() {
    // A fixed pseudo-random run of syncs on two viewers, pops, results of every kind, step
    // changes, relocations, toggles and budget cuts. After each, the bytes add up, stay in
    // budget unless the protected images alone are over it, and no image that was protected
    // before the operation and still is was evicted by it.
    let mut rng = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = |m: u64| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng % m
    };
    let src = source(400, &[3, 50, 51]);
    let mut st = store(40 * 64);
    let viewers = [st.viewer(), st.viewer()];
    let mut out: Vec<Ticket> = Vec::new();
    let mut now = 0.0;
    for step in 0..6000 {
        now += 0.05;
        let protected_before: Vec<ThumbKey> = {
            let inner = st.inner.borrow();
            inner
                .protected
                .iter()
                .copied()
                .filter(|k| ready(&st, index(*k)))
                .collect()
        };
        let op = next(10);
        let mut relocated = false;
        match op {
            0..=2 => {
                let v = viewers[next(2) as usize];
                let start = next(380) as usize;
                let len = 1 + next(30) as usize;
                let s = if next(4) == 0 { S8 } else { S4 };
                st.sync(
                    v,
                    &src,
                    Viewport::new(start..start + len, s).per_line(1 + next(3) as usize),
                );
            }
            3..=4 => out.extend(st.pop(now).map(|r| r.ticket)),
            5..=6 if !out.is_empty() => {
                let t = out.swap_remove(next(out.len() as u64) as usize);
                let outcome = match next(8) {
                    0 => Outcome::Failed,
                    1 => Outcome::Stopped,
                    2 => Outcome::Ready {
                        width: 3,
                        height: 3,
                        rgba: vec![0; 5],
                    },
                    _ => image(t.step()),
                };
                st.complete(t, outcome, now);
            }
            7 => {
                let _ = st.thumb(key(next(400) as usize));
            }
            8 => match next(4) {
                0 => {
                    st.set_enabled(next(2) == 0);
                }
                1 => {
                    // Replaces what the target held by design: not an eviction.
                    relocated = st.relocate(key(next(400) as usize), key(next(400) as usize));
                }
                2 => st.requeue_stopped(),
                _ => {
                    let budget = (8 + next(60) as usize) * 64;
                    st.set_settings(Settings {
                        budget_bytes: budget,
                        ..st.settings()
                    });
                }
            },
            _ => {
                let _ = st.take_cancelled();
            }
        }
        let inner = st.inner.borrow();
        let sum: usize = inner
            .entries
            .values()
            .filter_map(|e| e.held)
            .map(|h| h.bytes())
            .sum();
        assert_eq!(sum, inner.bytes, "op {step}: bytes add up");
        assert_eq!(
            inner.lru.len(),
            inner.entries.values().filter(|e| e.held.is_some()).count()
        );
        let protected = inner.protected_bytes();
        assert!(
            inner.bytes <= inner.settings.budget_bytes || inner.bytes == protected,
            "op {step}: {} bytes over a {} budget with {protected} protected",
            inner.bytes,
            inner.settings.budget_bytes
        );
        drop(inner);
        if relocated {
            continue;
        }
        for k in protected_before {
            let still = st.inner.borrow().protected.contains(&k);
            let there = st.inner.borrow().entries.contains_key(&k);
            if still && there {
                assert!(
                    ready(&st, index(k)),
                    "op {step} ({op}): protected {k:?} evicted"
                );
            }
        }
    }
}

#[test]
fn rows_on_screen_are_never_evicted_and_win_over_the_budget() {
    // Room for two images, ten on screen: all ten load and stay.
    let mut st = store(2 * 64);
    let src = source(40, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..10));
    let order = serve(&mut st, 0.0);
    assert_eq!(
        order,
        (0..10).collect::<Vec<_>>(),
        "no margin while the screen fills the budget"
    );
    assert!((0..10).all(|i| ready(&st, i)));
    assert_eq!(st.stats().bytes, 10 * 64);
    // Scrolled by one: the previous sync's rows stay protected for a sync (a grid's rows
    // come a frame late after a reflow, U-C1f-5), so nothing shown last time goes yet.
    st.sync(v, &src, view(10..20));
    assert!(
        (0..10).all(|i| ready(&st, i)),
        "the previous sync's rows stay"
    );
    serve(&mut st, 0.0);
    st.sync(v, &src, view(11..21));
    assert!(
        (0..10).all(|i| !ready(&st, i)),
        "two syncs on, the old rows go"
    );
    assert!((10..20).all(|i| ready(&st, i)));
    assert!(st.stats().bytes <= st.stats().protected_bytes);
}

#[test]
fn after_scrolling_down_the_screen_loads_in_reading_order_then_the_margin_below() {
    let mut st = store(1 << 20);
    let src = source(100, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..8));
    serve(&mut st, 0.0);
    st.sync(v, &src, view(40..48));
    assert_eq!(st.direction(v), Direction::Down);
    let order = serve(&mut st, 0.0);
    // 8 rows: a 25% margin is 2 rows a side. Below (ahead) nearest first, then above.
    assert_eq!(order, vec![40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 39, 38]);
}

#[test]
fn after_scrolling_up_the_margin_above_loads_first_nearest_first() {
    let mut st = store(1 << 20);
    let src = source(100, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(60..68));
    serve(&mut st, 0.0);
    st.sync(v, &src, view(20..28));
    assert_eq!(st.direction(v), Direction::Up);
    let requests = drain(&mut st, 0.0);
    let order: Vec<usize> = requests.iter().map(|r| index(r.key)).collect();
    assert_eq!(order, vec![20, 21, 22, 23, 24, 25, 26, 27, 19, 18, 28, 29]);
    let tiers: Vec<Tier> = requests.iter().map(|r| r.tier).collect();
    assert_eq!(
        tiers[8..],
        [Tier::Ahead, Tier::Ahead, Tier::Behind, Tier::Behind]
    );
}

#[test]
fn scrolling_re_ranks_queued_work_toward_the_new_viewport() {
    let mut st = store(1 << 20);
    let src = source(200, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..8));
    let first = st.pop(0.0).expect("row 0");
    assert_eq!(index(first.key), 0);
    // Scrolled before the rest was asked for: the next request is the new viewport's top.
    st.sync(v, &src, view(100..108));
    assert_eq!(st.pop(0.0).map(|r| index(r.key)), Some(100));
}

#[test]
fn a_grid_margin_is_whole_lines_nearest_line_first() {
    let mut st = store(1 << 20);
    let src = source(200, &[]);
    let v = st.viewer();
    // Four lines of four on screen: a margin of one line a side.
    st.sync(v, &src, view(100..116).per_line(4));
    serve(&mut st, 0.0);
    st.sync(v, &src, view(80..96).per_line(4));
    let order: Vec<usize> = drain(&mut st, 0.0).iter().map(|r| index(r.key)).collect();
    let mut want: Vec<usize> = (80..96).collect();
    want.extend([76, 77, 78, 79]); // the line above, in reading order
    // The line below (96..100) was already the earlier view's margin above: held.
    assert_eq!(order, want);
}

#[test]
fn nothing_past_the_margin_is_asked_for() {
    let mut st = store(1 << 20);
    let src = source(1000, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(500..520));
    let asked: Vec<usize> = serve(&mut st, 0.0);
    assert_eq!(asked.iter().min(), Some(&495));
    assert_eq!(asked.iter().max(), Some(&524));
    assert_eq!(asked.len(), 30);
}

#[test]
fn scrolling_away_cancels_pending_work_and_drops_its_late_result() {
    let mut st = store(1 << 20);
    let src = source(1000, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..10));
    let out = drain(&mut st, 0.0);
    assert_eq!(out.len(), 13);
    st.sync(v, &src, view(500..510));
    let cancelled = st.take_cancelled();
    assert_eq!(
        cancelled.len(),
        13,
        "every request for rows past the new margin"
    );
    assert!(st.take_cancelled().is_empty(), "taken once");
    let late = st.complete(out[0].ticket, image(S4), 0.1);
    assert_eq!(late, Landed::Dropped(Why::Cancelled));
    assert_eq!(
        st.state(key(0)),
        State::Empty,
        "the late result is not held"
    );
    // Scrolled back: asked again.
    st.sync(v, &src, view(0..10));
    assert_eq!(st.pop(0.2).map(|r| index(r.key)), Some(0));
}

#[test]
fn a_rebound_row_draws_only_its_own_entry() {
    // A filter replaces the listing under the same rows: requests for entries gone from the
    // viewer are cancelled, and each key's handles stay its own.
    let mut st = store(1 << 20);
    let mut src = source(100, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..10));
    let out = drain(&mut st, 0.0);
    src.retain(|e| e.id.get() % 2 == 0);
    st.resync(v, &src, 2);
    let cancelled: Vec<usize> = st.take_cancelled().iter().map(|t| index(t.key())).collect();
    assert!(
        cancelled.iter().all(|i| i % 2 == 0),
        "only keys no longer near: {cancelled:?}"
    );
    for r in out {
        st.complete(r.ticket, image(S4), 0.1);
    }
    for i in [1, 3, 5] {
        assert_eq!(
            st.thumb(key(i)).pixels().size(),
            (4, 4),
            "entry {i} drew its own image"
        );
    }
}

#[test]
fn a_failed_render_is_not_asked_for_again_for_thirty_seconds() {
    let mut st = store(1 << 20);
    let src = source(10, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..1));
    let r = st.pop(0.0).expect("asked");
    assert_eq!(st.complete(r.ticket, Outcome::Failed, 1.0), Landed::Failed);
    assert_eq!(st.state(key(0)), State::Failed { until: 31.0 });
    assert_eq!(
        st.thumb(key(0)).icon_alpha().get(),
        1.0,
        "it keeps its icon"
    );
    let nothing: Vec<usize> = drain(&mut st, 30.9).iter().map(|r| index(r.key)).collect();
    assert!(!nothing.contains(&0));
    assert_eq!(
        st.pop(31.0).map(|r| index(r.key)),
        Some(0),
        "asked again after 30 s"
    );
}

#[test]
fn a_stopped_render_is_not_failed_and_is_asked_again_once_the_operation_ends() {
    let mut st = store(1 << 20);
    let src = source(10, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..1));
    let r = st.pop(0.0).expect("asked");
    assert_eq!(
        st.complete(r.ticket, Outcome::Stopped, 0.0),
        Landed::Stopped
    );
    assert_eq!(st.state(key(0)), State::Stopped);
    assert!(
        drain(&mut st, 100.0).iter().all(|r| index(r.key) != 0),
        "not before it ends"
    );
    st.requeue_stopped();
    assert_eq!(st.pop(100.0).map(|r| index(r.key)), Some(0));
}

#[test]
fn turning_thumbnails_off_cancels_and_asks_nothing_and_on_restores_without_decoding() {
    let mut st = store(1 << 20);
    let src = source(40, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..8));
    serve(&mut st, 0.0);
    st.sync(v, &src, view(8..16));
    let pending = drain(&mut st, 0.0);
    assert!(!pending.is_empty());
    let version = st.version(key(0));
    assert!(st.set_enabled(false));
    assert!(!st.set_enabled(false), "no change, no commit");
    assert_ne!(st.version(key(0)), version, "rows build again, as icons");
    assert_eq!(st.take_cancelled().len(), pending.len());
    assert!(st.pop(0.0).is_none());
    st.sync(v, &src, view(0..8));
    assert!(st.pop(0.0).is_none(), "a scroll asks nothing while off");
    assert!(st.set_enabled(true));
    assert!((0..8).all(|i| ready(&st, i)), "held images draw again");
    let asked: Vec<usize> = drain(&mut st, 0.0).iter().map(|r| index(r.key)).collect();
    assert!(
        asked.iter().all(|i| *i >= 8),
        "nothing on screen decodes again: {asked:?}"
    );
}

#[test]
fn a_relocated_key_keeps_its_handles_and_image() {
    let mut st = store(1 << 20);
    let src = source(10, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..2));
    serve(&mut st, 0.0);
    let before = st.thumb(key(0));
    let renamed = ThumbKey::new(77);
    assert!(st.relocate(key(0), renamed));
    let after = st.thumb(renamed);
    assert_eq!(after.epoch(), before.epoch());
    assert_eq!(after.pixels().size(), (4, 4));
    assert!(matches!(st.state(renamed), State::Ready { .. }));
    assert_eq!(st.state(key(0)), State::Empty);
    assert!(!st.relocate(key(0), renamed), "nothing left to move");
    assert_eq!(st.stats().bytes, 3 * 64, "0, 1 and the margin's 2");
}

#[test]
fn a_new_step_replaces_the_image_in_place_after_first_loads() {
    let mut st = ThumbnailStore::new(Settings {
        margin: 0.0,
        ..Settings::default()
    });
    let src = source(40, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..4));
    serve(&mut st, 0.0);
    let handle = st.thumb(key(0));
    // Zoomed in, and scrolled so 4 and 5 show for the first time.
    st.sync(v, &src, Viewport::new(2..6, S8));
    let requests = drain(&mut st, 0.0);
    let order: Vec<usize> = requests.iter().map(|r| index(r.key)).collect();
    assert_eq!(order[..4], [4, 5, 2, 3], "first loads before new steps");
    assert!(requests.iter().all(|r| r.step == S8));
    assert_eq!(
        handle.pixels().size(),
        (4, 4),
        "the old image draws meanwhile"
    );
    st.sync(v, &src, Viewport::new(0..4, S8));
    let r = drain(&mut st, 0.0);
    let zero = r.iter().find(|r| index(r.key) == 0).expect("row 0 at 8 px");
    st.complete(zero.ticket, image(S8), 0.0);
    assert_eq!(handle.pixels().size(), (8, 8), "replaced in place");
    assert_eq!(st.thumb(key(0)).epoch(), handle.epoch());
}

#[test]
fn two_viewers_load_both_screens_first_at_the_larger_step() {
    let mut st = store(1 << 20);
    let src = source(200, &[]);
    let (a, b) = (st.viewer(), st.viewer());
    st.sync(a, &src, Viewport::new(0..4, S4));
    st.sync(b, &src, Viewport::new(100..104, S8));
    st.sync(b, &src, Viewport::new(2..6, S8));
    let requests = drain(&mut st, 0.0);
    let order: Vec<usize> = requests.iter().map(|r| index(r.key)).collect();
    assert_eq!(
        order[..6],
        [0, 1, 2, 3, 4, 5],
        "both screens, each key once"
    );
    let two = requests.iter().find(|r| index(r.key) == 2).expect("2");
    assert_eq!(two.step, S8, "shown by both: the larger step");
    st.drop_viewer(b);
    assert!(st.take_cancelled().iter().all(|t| index(t.key()) >= 4));
}

#[test]
fn bad_images_fail_the_key_without_a_panic() {
    let mut st = store(1 << 20);
    let src = source(10, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..4));
    let bad = [
        Outcome::Ready {
            width: 0,
            height: 4,
            rgba: Vec::new(),
        },
        Outcome::Ready {
            width: 4,
            height: 4,
            rgba: vec![0; 3],
        },
        Outcome::Ready {
            width: 16,
            height: 2,
            rgba: vec![0; 128],
        },
        Outcome::Ready {
            width: u32::MAX,
            height: u32::MAX,
            rgba: Vec::new(),
        },
    ];
    for outcome in bad {
        let r = st.pop(0.0).expect("asked");
        assert_eq!(
            st.complete(r.ticket, outcome, 0.0),
            Landed::Dropped(Why::Invalid)
        );
        assert!(matches!(st.state(r.key), State::Failed { .. }));
    }
    assert_eq!(st.stats().bytes, 0);
}

#[test]
fn entries_without_a_thumbnail_key_are_never_asked_for() {
    let mut st = store(1 << 20);
    let src = source(10, &[0, 2, 4]);
    let v = st.viewer();
    st.sync(v, &src, view(0..6));
    let asked = serve(&mut st, 0.0);
    assert_eq!(asked, vec![1, 3, 5, 6, 7]);
}

#[test]
fn an_unchanged_viewport_returns_at_once() {
    let mut st = store(1 << 20);
    let src = source(10, &[]);
    let v = st.viewer();
    st.sync(v, &src, view(0..4));
    let r = st.pop(0.0).expect("asked");
    st.sync(v, &src, view(0..4));
    assert_eq!(
        st.pop(0.0).map(|r| index(r.key)),
        Some(1),
        "the queue carried on"
    );
    assert!(st.take_cancelled().is_empty());
    st.complete(r.ticket, image(S4), 0.0);
}

#[test]
fn pruning_keeps_handles_near_any_viewer_and_asks_for_one_build() {
    let mut st = store(1 << 20);
    st.set_prune_slack(50);
    let src = source(2000, &[]);
    let v = st.viewer();
    assert!(!st.sync(v, &src, view(0..10)));
    let near = st.thumb(key(40)); // within five viewports of 20..30
    let far = st.thumb(key(900));
    // Many empty handles, as a long scroll builds rows that never load.
    for i in 0..200 {
        let _ = st.thumb(key(1500 + i));
    }
    assert!(
        st.sync(v, &src, view(20..30)),
        "a prune asks the host for one build"
    );
    assert!(!st.sync(v, &src, view(30..40)), "and only once");
    assert_eq!(
        st.thumb(key(40)).epoch(),
        near.epoch(),
        "kept: near the viewport"
    );
    let fresh = st.thumb(key(900));
    assert_ne!(
        fresh.epoch(),
        far.epoch(),
        "pruned, and seen again with new handles"
    );
    assert_ne!(st.version(key(900)), 0);
}
