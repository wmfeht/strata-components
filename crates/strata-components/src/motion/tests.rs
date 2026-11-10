//! The effects in a headless runtime: a list of keyed rows beside a target, the layer over
//! both, and a host that removes rows at the events, as Strata will.

mod perf;

use std::cell::Cell;
use std::rc::Rc;

use rune::{Cx, El, Env, Runtime, col, el, row, text};
use strata_kit::icons::IconName;
use strata_kit::look::Look;

use super::*;

const W: f32 = 520.0;
const H: f32 = 400.0;
const ROW_H: f32 = 26.0;
const DT: f64 = 1.0 / 60.0;

struct Host {
    fx: Effects,
    look: Look,
    rows: Vec<u64>,
    vanished: Vec<u64>,
    /// The target is mounted, at this y in the side panel.
    target: bool,
    target_y: f32,
    /// The side panel's own height: a target below it is scrolled out of it.
    side_h: f32,
    /// Rows the host removes at an effect's event.
    leaving: Option<(EffectId, Vec<u64>)>,
    events: Vec<(f64, Event)>,
    t: f64,
    /// Answer Launched with an instant commit.
    instant: bool,
    /// Flights name the list as their rows' viewport.
    rows_in: bool,
    /// The list's height: rows below it are laid out but scrolled out of it.
    list_h: f32,
    ghost: bool,
    clicks: Vec<u64>,
    views: Rc<Cell<usize>>,
}

impl Host {
    fn new(n: u64) -> Self {
        Self {
            fx: Effects::new(),
            look: Look::fallback(false),
            rows: (0..n).collect(),
            vanished: Vec::new(),
            target: true,
            target_y: 100.0,
            side_h: H,
            leaving: None,
            events: Vec::new(),
            t: 0.0,
            instant: false,
            rows_in: false,
            list_h: H,
            ghost: true,
            clicks: Vec::new(),
            views: Rc::default(),
        }
    }

    fn icon(&self) -> &'static rune::Icon {
        self.look.icon(IconName::File)
    }

    fn kinds(&self) -> Vec<&'static str> {
        self.events
            .iter()
            .map(|(_, e)| match e {
                Event::Launched(_) => "launched",
                Event::Landed(_) => "landed",
                Event::Done(_) => "done",
            })
            .collect()
    }

    fn at(&self, kind: &str) -> f64 {
        let i = self.kinds().iter().position(|k| *k == kind).unwrap();
        self.events[i].0
    }
}

fn row_el(s: &Host, i: u64) -> El {
    if s.vanished.contains(&i) {
        return el().key(format!("row-{i}")).w_full().h(ROW_H).inert();
    }
    row()
        .key(format!("row-{i}"))
        .w_full()
        .h(ROW_H)
        .gap(6.0)
        .center_y()
        .on_press(move |s: &mut Host| s.clicks.push(i))
        .child(
            el().key("icon")
                .size(16.0, 16.0)
                .fill(s.look.kit.c.icon_base),
        )
        .child(text(format!("entry {i}")))
}

fn ghost(_: &Host, token: u64, look: &Look) -> El {
    row()
        .w_full()
        .h_full()
        .gap(6.0)
        .center_y()
        .fill(rune_kit::TableStyle::from_kit(&look.kit).selected)
        .child(el().size(16.0, 16.0).fill(look.kit.c.icon_base))
        .child(text(format!("entry {token}")))
}

fn view(s: &Host, cx: &mut Cx<Host>) -> El {
    s.views.set(s.views.get() + 1);
    let mut side = el().key("side").w(120.0).h(s.side_h).clip_soft(0.0);
    if s.target {
        let target = el()
            .key("trash")
            .size(24.0, 24.0)
            .fill(s.look.kit.c.icon_muted);
        side = side.child(
            el().absolute(40.0, s.target_y)
                .child(dress_target(&s.fx, "trash", target)),
        );
    }
    let list = col()
        .key("list")
        .w(W - 120.0)
        .h(s.list_h)
        .clip_soft(0.0)
        .children(s.rows.iter().map(|&i| row_el(s, i)));
    let mut layer = MotionLayer::new(|s: &Host| &s.fx);
    if s.ghost {
        layer = layer.ghost(ghost);
    }
    row()
        .size(W, H)
        .child(side)
        .child(list)
        .child(layer.build(s, &s.look, cx))
}

fn poll(s: &mut Host, t: f64, tx: &mut rune::Tx) -> bool {
    s.t = t;
    let events = s.fx.tick(t, tx);
    let changed = !events.is_empty();
    for e in events {
        s.events.push((t, e));
        let owner = s.leaving.as_ref().map(|(id, _)| *id);
        match e {
            Event::Launched(id) if owner == Some(id) => {
                let gone = s.leaving.as_ref().unwrap().1.clone();
                if !s.vanished.is_empty() {
                    continue;
                }
                s.rows.retain(|r| !gone.contains(r));
                if s.instant {
                    tx.instant();
                }
            }
            Event::Landed(_) => s.vanished.clear(),
            Event::Done(id) if owner == Some(id) => {
                let gone = s.leaving.take().unwrap().1;
                if s.fx.dissolving() || s.vanished.iter().any(|v| gone.contains(v)) {
                    s.rows.retain(|r| !gone.contains(r));
                    s.vanished.clear();
                }
            }
            _ => {}
        }
    }
    changed
}

fn runtime(host: Host) -> Runtime<Host> {
    let mut rt = Runtime::new(host, view, (W, H), Env::new(false, 1.0));
    rt.poll = Some(poll);
    rt.frame(0.0);
    rt.frame(DT);
    rt
}

fn icons(s: &Host, rows: &[u64], part: &str) -> Vec<(String, &'static rune::Icon)> {
    rows.iter()
        .map(|i| (format!("row-{i}{part}"), s.icon()))
        .collect()
}

/// Flies `rows`' icons into the target, as the host does when Move to Trash ends quickly.
fn trash(rt: &mut Runtime<Host>, rows: &[u64]) -> Option<EffectId> {
    let mut out = None;
    rt.update(|s, tx| {
        let mut flight = Flight::into_target("trash", icons(s, rows, "/icon")).target_in("side");
        if s.rows_in {
            flight = flight.rows_in("list");
        }
        let Host { fx, look, .. } = s;
        out = fx.fly(flight, look, tx);
        match out {
            Some(id) => s.leaving = Some((id, rows.to_vec())),
            None => s.rows.retain(|r| !rows.contains(r)),
        }
    });
    rt.frame(DT);
    out
}

fn dissolve(rt: &mut Runtime<Host>, rows: &[u64]) -> Option<EffectId> {
    let mut out = None;
    rt.update(|s, tx| {
        let d = Dissolve::rows(rows.iter().map(|&i| (format!("row-{i}"), i))).within("list");
        let Host { fx, look, .. } = s;
        out = fx.dissolve(d, look, tx);
        match out {
            Some(id) => {
                s.leaving = Some((id, rows.to_vec()));
                s.vanished.extend_from_slice(rows);
            }
            None if s.leaving.is_none() => s.rows.retain(|r| !rows.contains(r)),
            None => {}
        }
    });
    rt.frame(DT);
    out
}

/// Steps from wake to wake, with no frames between, until `done` or `limit` seconds pass.
fn wake_to_wake(rt: &mut Runtime<Host>, limit: f64, done: impl Fn(&Host) -> bool) {
    let end = rt.state.t + limit;
    while !done(&rt.state) && rt.state.t < end {
        let Some(due) = rt.wake_deadline() else { break };
        rt.frame((due - rt.state.t).max(0.001));
    }
}

/// Steps a frame at a time until `done` or `limit` seconds pass.
fn frames(rt: &mut Runtime<Host>, limit: f64, done: impl Fn(&Host) -> bool) {
    let end = rt.state.t + limit;
    while !done(&rt.state) && rt.state.t < end {
        rt.frame(DT);
    }
}

fn has(rt: &Runtime<Host>, key: &str) -> bool {
    rt.elements()
        .iter()
        .any(|e| !e.exiting && e.key.as_deref() == Some(key))
}

fn keys_with(rt: &Runtime<Host>, prefix: &str) -> Vec<String> {
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| e.key)
        .filter(|k| k.starts_with(prefix))
        .collect()
}

fn middle(r: [f32; 4]) -> [f32; 2] {
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn target_middle(rt: &Runtime<Host>) -> [f32; 2] {
    middle(rt.presented("trash").unwrap().0)
}

fn launched(s: &Host) -> bool {
    s.kinds().contains(&"launched")
}

fn finished(s: &Host) -> bool {
    s.kinds().contains(&"done")
}

// The flight.

#[test]
fn a_flight_launches_flies_lands_and_cleans_up() {
    let mut rt = runtime(Host::new(5));
    let id = trash(&mut rt, &[1, 3]).expect("it plays");
    assert!(has(&rt, &launch_key(id, 0)), "the launch side is drawn");
    assert!(has(&rt, "row-1"), "the rows stay until Launched");

    wake_to_wake(&mut rt, 1.0, launched);
    let flip = rt.state.at("launched");
    assert!(
        !has(&rt, "row-1") && !has(&rt, "row-3"),
        "Launched took the rows"
    );
    assert!(has(&rt, &flyer_key(id, 0)) && has(&rt, &flyer_key(id, 1)));
    assert!(keys_with(&rt, "c23-launch").is_empty());
    rt.frame(DT);
    let views = rt.state.views.get();
    frames(&mut rt, 1.0, |s| s.t - flip > TRAVEL);
    assert_eq!(
        rt.state.views.get(),
        views,
        "no view runs while the icons fly"
    );

    wake_to_wake(&mut rt, 2.0, finished);
    assert_eq!(rt.state.kinds(), ["launched", "landed", "done"]);
    let land = rt.state.at("landed") - flip;
    assert!(
        (TRAVEL + STAGGER - 1e-3..TRAVEL + STAGGER + 0.02).contains(&land),
        "the last of two icons lands after its stagger: {land}"
    );
    let done = rt.state.at("done") - rt.state.at("landed");
    assert!((BOUNCE - 1e-3..BOUNCE + 0.02).contains(&done), "{done}");
    rt.frame(DT);
    assert!(
        keys_with(&rt, "c23-")
            .iter()
            .all(|k| k == LAYER || k.starts_with("c23-pulse"))
    );
    assert_eq!(rt.wake_deadline(), None, "at rest");
}

#[test]
fn the_target_rings_while_icons_fly_in_and_pulses_when_they_land() {
    let mut rt = runtime(Host::new(5));
    let id = trash(&mut rt, &[1]).unwrap();
    wake_to_wake(&mut rt, 1.0, launched);
    assert!(has(&rt, &format!("c23-receiving-{}", id.0)));
    assert!(!has(&rt, "c23-pulse-1"), "no pulse before landing");
    wake_to_wake(&mut rt, 1.0, |s| s.kinds().contains(&"landed"));
    rt.frame(DT);
    assert!(has(&rt, "c23-pulse-1"), "the target swells");
    assert!(has(&rt, &ring_key("trash", 1)), "and rings");
    assert!(!has(&rt, &format!("c23-receiving-{}", id.0)));
    // A second flight replays the pulse under a new key.
    wake_to_wake(&mut rt, 1.0, finished);
    rt.state.events.clear();
    trash(&mut rt, &[2]).unwrap();
    wake_to_wake(&mut rt, 2.0, |s| s.kinds().contains(&"landed"));
    rt.frame(DT);
    assert!(has(&rt, "c23-pulse-2") && !has(&rt, "c23-pulse-1"));
}

#[test]
fn flyers_close_on_the_target_and_land_in_turn() {
    let mut rt = runtime(Host::new(8));
    let id = trash(&mut rt, &[0, 3, 6]).unwrap();
    frames(&mut rt, 1.0, launched);
    let target = target_middle(&rt);
    let start: Vec<f32> = (0..3)
        .map(|i| distance(middle(rt.presented(&flyer_key(id, i)).unwrap().0), target))
        .collect();
    let mut last = start.clone();
    for _ in 0..12 {
        rt.frame(DT);
        for (i, d) in last.iter_mut().enumerate() {
            let now = distance(middle(rt.presented(&flyer_key(id, i)).unwrap().0), target);
            assert!(
                now <= *d + 0.01,
                "flyer {i} heads for the target: {now} after {d}"
            );
            *d = now;
        }
    }
    let left: Vec<f32> = (0..3).map(|i| last[i] / start[i]).collect();
    assert!(
        left[0] < left[1] && left[1] < left[2],
        "each later icon is further behind: {left:?}"
    );
}

#[test]
fn an_effect_started_before_the_tick_in_the_same_poll_still_plays() {
    let mut rt = runtime(Host::new(5));
    let mut id = None;
    rt.update(|s, tx| {
        let f = Flight::into_target("trash", icons(s, &[2], "/icon"));
        let Host { fx, look, t, .. } = s;
        id = fx.fly(f, look, tx);
        assert!(
            fx.tick(*t, tx).is_empty(),
            "nothing launches before it mounts"
        );
    });
    rt.frame(DT);
    let id = id.unwrap();
    rt.state.leaving = Some((id, vec![2]));
    frames(&mut rt, 1.0, launched);
    assert!(has(&rt, &flyer_key(id, 0)), "it flies");

    // A dissolve started the same way plays, rather than ending at once with no plan.
    rt.update(|s, tx| {
        let d = Dissolve::rows([("row-4".to_owned(), 4)]);
        let Host { fx, look, t, .. } = s;
        fx.dissolve(d, look, tx).unwrap();
        fx.tick(*t, tx);
    });
    let start = rt.state.t;
    rt.frame(DT);
    frames(&mut rt, 2.0, |s| !s.fx.dissolving());
    assert!(rt.state.t - start >= DURATION, "it played");
}

#[test]
fn at_most_seven_icons_fly_picked_evenly() {
    let mut rt = runtime(Host::new(30));
    let all: Vec<u64> = (0..14).collect();
    let id = trash(&mut rt, &all).unwrap();
    let launches = keys_with(&rt, "c23-launch");
    assert_eq!(launches.len(), MAX_FLYERS);
    let first = rt.presented(&launch_key(id, 0)).unwrap().0;
    let last = rt.presented(&launch_key(id, 6)).unwrap().0;
    let row = |i: u64| rt.presented(&format!("row-{i}")).unwrap().0;
    assert!(
        (first[1] - row(0)[1]).abs() < ROW_H,
        "the first from the first row"
    );
    assert!(
        (last[1] - row(12)[1]).abs() < ROW_H,
        "the last from row 12 of 14"
    );
}

#[test]
fn animations_off_play_nothing() {
    let mut host = Host::new(5);
    host.look.animations = false;
    let mut rt = runtime(host);
    assert_eq!(trash(&mut rt, &[1]), None);
    assert_eq!(dissolve(&mut rt, &[2]), None);
    rt.frame(DT);
    assert!(
        !has(&rt, "row-1") && !has(&rt, "row-2"),
        "the end state at once"
    );
    assert!(keys_with(&rt, "c23-").iter().all(|k| k == LAYER));
    assert_eq!(rt.wake_deadline(), None);
}

#[test]
fn reduced_motion_draws_nothing_and_each_effect_ends_at_once() {
    let mut rt = runtime(Host::new(5));
    rt.set_system_reduced_motion(true);
    rt.frame(DT);
    trash(&mut rt, &[1]).unwrap();
    dissolve(&mut rt, &[3]).unwrap();
    assert!(keys_with(&rt, "c23-").iter().all(|k| k == LAYER));
    wake_to_wake(&mut rt, 0.2, |s| {
        s.kinds().iter().filter(|k| **k == "done").count() == 2
    });
    assert_eq!(rt.state.kinds(), ["launched", "done", "done"]);
    assert!(!rt.state.fx.playing());
}

#[test]
fn a_leg_or_target_out_of_sight_flies_nothing_but_still_launches_and_ends() {
    type Setup = fn(&mut Host);
    let cases: [(&str, Setup, &[u64]); 5] = [
        ("an unmounted row", |_| {}, &[99]),
        (
            "a row scrolled out of its list",
            |s| {
                s.rows_in = true;
                s.list_h = 100.0;
            },
            &[6],
        ),
        ("the sidebar hidden", |s| s.target = false, &[1]),
        ("a target off the window", |s| s.target_y = H + 40.0, &[1]),
        (
            "a target scrolled out of its viewport",
            |s| s.side_h = 80.0,
            &[1],
        ),
    ];
    for (case, setup, rows) in cases {
        let mut host = Host::new(30);
        setup(&mut host);
        let mut rt = runtime(host);
        trash(&mut rt, rows).unwrap();
        assert!(
            keys_with(&rt, "c23-launch").is_empty(),
            "{case}: nothing launches"
        );
        wake_to_wake(&mut rt, 0.2, finished);
        assert_eq!(rt.state.kinds(), ["launched", "done"], "{case}");
        assert!(!has(&rt, "c23-pulse-1"), "{case}: no pulse");
    }
}

#[test]
fn a_target_gone_mid_flight_ends_on_time_without_a_pulse() {
    let mut rt = runtime(Host::new(5));
    let id = trash(&mut rt, &[1]).unwrap();
    frames(&mut rt, 1.0, launched);
    for _ in 0..5 {
        rt.frame(DT);
    }
    rt.update(|s, _| s.target = false);
    rt.frame(DT);
    assert!(
        !has(&rt, &flyer_key(id, 0)),
        "the icon leaves with its target"
    );
    wake_to_wake(&mut rt, 2.0, finished);
    assert_eq!(rt.state.kinds(), ["launched", "landed", "done"]);
    assert!(keys_with(&rt, "c23-ring").is_empty());
}

#[test]
fn a_moved_target_still_takes_the_icons() {
    let mut rt = runtime(Host::new(5));
    let id = trash(&mut rt, &[4]).unwrap();
    frames(&mut rt, 1.0, launched);
    for _ in 0..6 {
        rt.frame(DT);
    }
    // The window resizes and the target moves on, mid-flight.
    rt.update(|s, _| s.target_y = 300.0);
    let old = [52.0, 112.0];
    frames(&mut rt, 1.0, |s| s.t - s.at("launched") > TRAVEL - 0.03);
    let flyer = middle(rt.presented(&flyer_key(id, 0)).unwrap().0);
    let target = target_middle(&rt);
    assert!(
        distance(flyer, target) < 12.0 && distance(flyer, old) > 150.0,
        "it heads for the target's new place: {flyer:?} for {target:?}"
    );
}

#[test]
fn an_instant_flip_flies_nothing_at_the_pin() {
    // At Rune 5d421fd an instant commit turns the matched swap off: the icon shows at its
    // landing place at once. Re-check at the next bump (5a9f64e8, f7bf9e04).
    let mut host = Host::new(5);
    host.instant = true;
    let mut rt = runtime(host);
    let id = trash(&mut rt, &[4]).unwrap();
    frames(&mut rt, 1.0, launched);
    rt.frame(DT);
    let flyer = middle(rt.presented(&flyer_key(id, 0)).unwrap().0);
    assert!(
        distance(flyer, target_middle(&rt)) < 1.0,
        "the icon is already on the target: {flyer:?}"
    );
}

#[test]
fn a_restore_flies_out_onto_the_rows_and_lands_before_they_show() {
    let mut host = Host::new(6);
    host.vanished = vec![2, 4];
    let mut rt = runtime(host);
    let mut id = None;
    rt.update(|s, tx| {
        let f = Flight::out_of_target("trash", icons(s, &[2, 4], "")).target_in("side");
        let Host { fx, look, .. } = s;
        id = fx.fly(f, look, tx);
        s.leaving = Some((id.unwrap(), vec![2, 4]));
    });
    rt.frame(DT);
    let id = id.unwrap();
    let launch = middle(rt.presented(&launch_key(id, 0)).unwrap().0);
    assert!(
        distance(launch, target_middle(&rt)) < 1.0,
        "they leave from the target"
    );
    frames(&mut rt, 1.0, launched);
    rt.frame(DT);
    assert!(has(&rt, "c23-pulse-1"), "the target pulses as they leave");
    assert!(has(&rt, &ring_key("trash", 1)));
    let row = rt.presented("row-4").unwrap().0;
    frames(&mut rt, 1.0, |s| s.kinds().contains(&"landed"));
    assert!(rt.state.vanished.is_empty(), "the rows show at Landed");
    let flyer = middle(rt.presented(&flyer_key(id, 1)).unwrap().0);
    assert!(
        flyer[0] < row[0] + ROW_H && (flyer[1] - middle(row)[1]).abs() < 2.0,
        "the icon came down on its row's leading square: {flyer:?} in {row:?}"
    );
    wake_to_wake(&mut rt, 1.0, finished);
    assert_eq!(rt.state.kinds(), ["launched", "landed", "done"]);
}

#[test]
fn a_release_lifts_off_the_rows_and_shakes_the_target() {
    let mut rt = runtime(Host::new(8));
    let mut id = None;
    rt.update(|s, tx| {
        let f = Flight::release("trash", icons(s, &[6], "/icon"));
        let Host { fx, look, .. } = s;
        id = fx.fly(f, look, tx);
    });
    rt.frame(DT);
    let id = id.unwrap();
    let start = rt.presented(&launch_key(id, 0)).unwrap().0;
    frames(&mut rt, 1.0, launched);
    rt.frame(DT);
    assert!(has(&rt, "c23-pulse-1"), "the target shakes");
    assert!(keys_with(&rt, "c23-ring").is_empty(), "with no ring");
    for _ in 0..8 {
        rt.frame(DT);
    }
    let now = rt.presented(&flyer_key(id, 0)).unwrap().0;
    assert!(
        now[1] < start[1] - 30.0,
        "the icon rises: {now:?} from {start:?}"
    );
}

#[test]
fn the_layer_takes_no_clicks() {
    let mut rt = runtime(Host::new(5));
    let id = trash(&mut rt, &[1]).unwrap();
    let [x, y] = middle(rt.presented(&launch_key(id, 0)).unwrap().0);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(DT);
    assert_eq!(rt.state.clicks, [1], "the row under the icon took it");
}

#[test]
fn settling_ends_every_effect_with_the_events_it_owed() {
    let mut rt = runtime(Host::new(6));
    rt.update(|s, tx| {
        let f = Flight::into_target("trash", icons(s, &[1], "/icon"));
        let d = Dissolve::rows([("row-3".to_owned(), 3)]);
        let Host { fx, look, .. } = s;
        fx.fly(f, look, tx).unwrap();
        fx.dissolve(d, look, tx).unwrap();
    });
    rt.frame(DT);
    let events = rt.state.fx.settle();
    let kinds: Vec<_> = events
        .iter()
        .map(|e| match e {
            Event::Launched(_) => "launched",
            Event::Landed(_) => "landed",
            Event::Done(_) => "done",
        })
        .collect();
    assert_eq!(kinds, ["launched", "landed", "done", "done"]);
    assert!(!rt.state.fx.playing());
}

// The dissolve.

#[test]
fn a_dissolve_breaks_only_the_rows_that_show_and_ends() {
    let mut rt = runtime(Host::new(30));
    // Row 25 is laid out below the list's clip.
    dissolve(&mut rt, &[2, 25]).unwrap();
    {
        let d = rt.state.fx.dissolve.as_ref().unwrap();
        let plan = d.plan.borrow();
        let plan = plan.as_ref().unwrap();
        assert_eq!(plan.rows.iter().map(|r| r.0).collect::<Vec<_>>(), [0]);
        assert!(
            (80..=MAX_FRAGMENTS).contains(&plan.pieces.len()),
            "{}",
            plan.pieces.len()
        );
    }
    assert!(
        has(&rt, "row-3") && has(&rt, "row-2"),
        "the rows stay listed"
    );
    let survivor = rt.presented("row-3").unwrap().0;
    wake_to_wake(&mut rt, 2.0, finished);
    let took = rt.state.at("done");
    assert!((DURATION..DURATION + 0.06).contains(&took), "{took}");
    assert_eq!(
        rt.presented("row-3").unwrap().0,
        survivor,
        "survivors hold still while it plays"
    );
    rt.frame(DT);
    assert!(
        !has(&rt, "row-2") && !has(&rt, "row-25"),
        "Done took the rows"
    );
    assert!(keys_with(&rt, "c23-dissolve").is_empty());
    assert_eq!(rt.wake_deadline(), None);
}

#[test]
fn nothing_commits_while_a_dissolve_plays_and_its_fragments_fade() {
    let mut rt = runtime(Host::new(6));
    dissolve(&mut rt, &[1, 2]).unwrap();
    rt.frame(DT);
    rt.frame(DT);
    let views = rt.state.views.get();
    let mut opacity = 1.0;
    let mut fading = false;
    while !finished(&rt.state) {
        rt.frame(DT);
        if finished(&rt.state) {
            break;
        }
        assert_eq!(rt.state.views.get(), views, "no view runs mid-dissolve");
        let d = rt.state.fx.dissolve.as_ref().unwrap();
        let plan = d.plan.borrow();
        let now = plan.as_ref().unwrap().pieces[0].op.get();
        assert!(now <= opacity);
        fading |= now < opacity;
        opacity = now;
    }
    assert!(fading, "the fragments fade");
}

#[test]
fn a_second_dissolve_while_one_plays_plays_nothing() {
    let mut rt = runtime(Host::new(6));
    dissolve(&mut rt, &[1]).unwrap();
    assert_eq!(dissolve(&mut rt, &[3]), None);
    wake_to_wake(&mut rt, 2.0, finished);
    rt.frame(DT);
    let [x, y] = middle(rt.presented("row-4").unwrap().0);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(DT);
    assert_eq!(rt.state.clicks, [4], "the view takes clicks after");
}

#[test]
fn clicks_reach_surviving_rows_during_a_dissolve() {
    let mut rt = runtime(Host::new(6));
    dissolve(&mut rt, &[1, 2]).unwrap();
    for _ in 0..10 {
        rt.frame(DT);
    }
    assert!(rt.state.fx.dissolving());
    // Row 3 sits right under the rising fragments.
    let r = rt.presented("row-3").unwrap().0;
    rt.pointer_down(r[0] + 30.0, r[1] + 4.0);
    rt.pointer_up(r[0] + 30.0, r[1] + 4.0);
    rt.frame(DT);
    assert_eq!(rt.state.clicks, [3]);
}
