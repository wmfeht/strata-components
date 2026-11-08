//! The marquee machine on a plain [`Surface`] and a virtual clock. Each test names the
//! feature-map bullet (or the decision) it checks. Rows are 28 pt tall and ids are the rows'
//! first indices, so `[4, 5]` reads as "the fifth and sixth rows as first listed".

use std::time::Duration;

use rune::{Mods, Spring};

use super::*;
use crate::collection::selection::{BandMode, Changed, CursorGone, Selection, SyncRule};
use crate::contract::{
    CollectionGeometry, EntryId, EntryKind, EntrySource, ListEntry, ListSource, Point, Rect,
    UniformRows,
};

const ROW: f32 = 28.0;
const WIDTH: f32 = 400.0;
const VIEW: f32 = 560.0;
const FRAME: f64 = 1.0 / 60.0;

struct Fake {
    source: ListSource,
    geometry: UniformRows,
    visible: Rect,
    now: f64,
    sel: Selection,
    out: Vec<Outcome>,
    /// Every selection change, as (clock, changed).
    applied: Vec<(f64, Changed)>,
}

impl Surface for Fake {
    fn source(&self) -> &dyn EntrySource {
        &self.source
    }

    fn geometry(&self) -> &dyn CollectionGeometry {
        &self.geometry
    }

    fn visible(&self) -> Rect {
        self.visible
    }

    fn now(&self) -> f64 {
        self.now
    }

    fn selection(&self) -> &Selection {
        &self.sel
    }

    fn apply(
        &mut self,
        op: &mut dyn FnMut(&mut Selection, &dyn EntrySource) -> Changed,
    ) -> Changed {
        let changed = op(&mut self.sel, &self.source);
        if changed.any() {
            self.applied.push((self.now, changed));
        }
        changed
    }

    fn emit(&mut self, outcome: Outcome) {
        self.out.push(outcome);
    }
}

fn entry(raw: u64) -> ListEntry {
    ListEntry::new(EntryId::new(raw), format!("e{raw}"), EntryKind::File, "")
}

/// `n` rows in a viewport scrolled to the top.
fn fake(n: u64) -> Fake {
    Fake {
        source: ListSource::new((0..n).map(entry).collect()),
        geometry: UniformRows::new(n as usize, ROW, WIDTH).viewport_height(VIEW),
        visible: Rect::new(0.0, 0.0, WIDTH, VIEW),
        now: 0.0,
        sel: Selection::new(),
        out: Vec::new(),
        applied: Vec::new(),
    }
}

fn machine() -> MarqueeMachine {
    MarqueeMachine::new(MarqueeHandle::default())
}

/// The content point in the middle of row `row`'s height, in its blank space.
fn at_row(row: usize) -> Point {
    Point::new(300.0, row as f32 * ROW + ROW / 2.0)
}

/// The content point at viewport y `y`, as the viewport shows now.
fn at_view(f: &Fake, y: f32) -> Point {
    Point::new(300.0, f.visible.y + y)
}

fn selected(f: &Fake) -> Vec<u64> {
    f.sel
        .selected_ids(&f.source)
        .into_iter()
        .map(EntryId::get)
        .collect()
}

fn ctrl() -> Mods {
    Mods {
        ctrl: true,
        ..Mods::default()
    }
}

fn shift() -> Mods {
    Mods {
        shift: true,
        ..Mods::default()
    }
}

fn glides(out: &[Outcome]) -> Vec<f32> {
    out.iter()
        .filter_map(|o| match o {
            Outcome::Glide { y, .. } => Some(*y),
            _ => None,
        })
        .collect()
}

fn stops(out: &[Outcome]) -> usize {
    out.iter()
        .filter(|o| matches!(o, Outcome::StopGlide { .. }))
        .count()
}

/// A viewport glide as Rune steps one (`runtime/scroll/offset.rs` at the pinned rev): a new
/// target keeps the speed unless it would overshoot (`retarget_v`), then the spring's exact
/// solution moves the offset.
#[derive(Default)]
struct Glide {
    v: f32,
    to: Option<f32>,
}

impl Glide {
    fn aim(&mut self, spring: Spring, offset: f32, to: f32) {
        let d = offset - to;
        let most = spring.omega() * d.abs();
        if self.v * d < 0.0 && self.v.abs() > most {
            self.v = -d.signum() * most;
        }
        self.to = Some(to);
    }

    fn step(&mut self, spring: Spring, offset: f32, max: f32, dt: f32) -> f32 {
        let Some(to) = self.to else {
            return offset;
        };
        let to = to.clamp(0.0, max);
        let (d, v) = spring.sample(offset - to, self.v, 0.0, dt);
        self.v = v;
        if d.abs() < 0.1 && v.abs() < 4.0 {
            self.to = None;
            self.v = 0.0;
            return to;
        }
        to + d
    }
}

/// One frame as the element runs it: the host's poll ticks the machine, then Rune steps the
/// glide the machine asked for. Returns the tick's result.
fn frame(f: &mut Fake, m: &mut MarqueeMachine, glide: &mut Glide) -> bool {
    f.now += FRAME;
    let changed = m.tick(f);
    let spring = m.settings().get().glide;
    for o in std::mem::take(&mut f.out) {
        if let Outcome::Glide { y, .. } | Outcome::StopGlide { y, .. } = o {
            glide.aim(spring, f.visible.y, y);
        }
    }
    let max = f.geometry.len as f32 * ROW - VIEW;
    f.visible.y = glide.step(spring, f.visible.y, max, FRAME as f32);
    changed
}

#[test]
fn a_band_selects_every_row_it_crosses_mounted_or_not() {
    // browser/selection/marquee: A drag from blank pane space draws a band from the press point and selects every entry it crosses.
    let mut f = fake(10_000);
    let mut m = machine();
    m.begin(at_row(0), Mods::default(), &mut f);
    m.moved(at_view(&f, 300.0), &mut f);
    assert_eq!(selected(&f), (0..=10).collect::<Vec<_>>());
    // The view jumps to the end (the scrollbar, End): rows 11 to 9,979 were never shown.
    f.visible.y = 10_000.0 * ROW - VIEW;
    m.moved(at_view(&f, VIEW - 1.0), &mut f);
    m.end(&mut f);
    assert_eq!(f.sel.len(), 10_000, "a range by index, not by mounted rows");
    assert!(!m.is_active());
}

#[test]
fn a_plain_band_replaces_the_selection_and_moving_back_unselects() {
    // browser/selection/marquee: A plain marquee replaces the selection with the entries under the band.
    let mut f = fake(20);
    f.sel.select_only(&f.source, 1);
    f.sel.toggle(&f.source, 12);
    let mut m = machine();
    m.begin(at_row(5), Mods::default(), &mut f);
    m.moved(at_row(8), &mut f);
    assert_eq!(selected(&f), [5, 6, 7, 8]);
    m.moved(at_row(6), &mut f);
    assert_eq!(selected(&f), [5, 6]);
    m.end(&mut f);
    assert_eq!(selected(&f), [5, 6]);
}

#[test]
fn a_ctrl_band_toggles_against_the_selection_at_press() {
    // browser/selection/marquee: A Ctrl-marquee toggles each crossed entry relative to the selection at press.
    for mods in [
        ctrl(),
        Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
    ] {
        let mut f = fake(20);
        f.sel.select_only(&f.source, 5);
        f.sel.toggle(&f.source, 6);
        f.sel.toggle(&f.source, 15);
        let mut m = machine();
        m.begin(at_row(4), mods, &mut f);
        m.moved(at_row(7), &mut f);
        assert_eq!(selected(&f), [4, 7, 15], "Ctrl wins over Shift (D5)");
        // Moving the band back off 6 and 7 puts them back as they were at the press.
        m.moved(at_row(5), &mut f);
        assert_eq!(selected(&f), [4, 6, 15]);
        m.end(&mut f);
        assert_eq!(selected(&f), [4, 6, 15]);
    }
}

#[test]
fn a_shift_band_adds_to_the_selection_at_press() {
    // browser/selection/marquee: A Shift-marquee adds the crossed entries to the selection at press.
    let mut f = fake(20);
    f.sel.select_only(&f.source, 1);
    f.sel.toggle(&f.source, 5);
    let mut m = machine();
    m.begin(at_row(4), shift(), &mut f);
    m.moved(at_row(6), &mut f);
    assert_eq!(selected(&f), [1, 4, 5, 6]);
    m.moved(at_row(4), &mut f);
    assert_eq!(selected(&f), [1, 4, 5], "5 was selected at the press");
    m.end(&mut f);
    assert_eq!(selected(&f), [1, 4, 5]);
}

#[test]
fn alt_only_forces_the_marquee_and_replaces() {
    // browser/selection/marquee: Holding Alt starts a marquee even when the press lands on an entry.
    let alt = Mods {
        alt: true,
        ..Mods::default()
    };
    assert_eq!(mode_for(alt), BandMode::Replace);
    let ctrl_alt = Mods {
        alt: true,
        ..ctrl()
    };
    assert_eq!(mode_for(ctrl_alt), BandMode::Toggle);
}

#[test]
fn the_selection_survives_pointer_movement_after_release() {
    // browser/selection/marquee: The selection survives pointer movement after release.
    let mut f = fake(20);
    let mut m = machine();
    m.begin(at_row(2), Mods::default(), &mut f);
    m.moved(at_row(4), &mut f);
    m.end(&mut f);
    assert!(!m.moved(at_row(12), &mut f));
    assert!(!m.tick(&mut f));
    assert_eq!(selected(&f), [2, 3, 4]);
}

#[test]
fn the_band_never_moves_the_cursor_or_the_preview() {
    // browser/selection/marquee: Marquee selection never opens or retargets quick preview, including with single-click previews on.
    let mut f = fake(20);
    f.sel.select_only(&f.source, 2);
    let (cursor, anchor) = (f.sel.cursor(), f.sel.anchor());
    let mut m = machine();
    m.begin(at_row(4), ctrl(), &mut f);
    m.moved(at_row(9), &mut f);
    m.end(&mut f);
    assert_eq!(selected(&f), [2, 4, 5, 6, 7, 8, 9]);
    // D3: the cursor and the anchor stay; previews follow the cursor, so they stay too.
    assert_eq!((f.sel.cursor(), f.sel.anchor()), (cursor, anchor));
    assert!(f.applied.iter().all(|(_, c)| !c.cursor && !c.anchor));
}

#[test]
fn escape_puts_back_the_selection_the_band_started_from() {
    // D2: Escape, or a gesture cancelled without a release, restores the selection at press.
    let mut f = fake(20);
    f.sel.select_only(&f.source, 1);
    f.sel.toggle(&f.source, 2);
    let mut m = machine();
    m.begin(at_row(0), ctrl(), &mut f);
    m.moved(at_row(5), &mut f);
    assert_eq!(selected(&f), [0, 3, 4, 5]);
    assert!(m.cancel(&mut f));
    assert_eq!(selected(&f), [1, 2]);
    assert!(!m.cancel(&mut f), "with no band open the key goes on");

    // A plain band cleared the selection at its press, and goes back to that.
    m.begin(at_row(6), Mods::default(), &mut f);
    m.moved(at_row(8), &mut f);
    assert!(m.cancel(&mut f));
    assert!(f.sel.is_empty());
}

#[test]
fn another_selecting_operation_ends_the_band() {
    // C1a: any other selecting operation ends a band; the marquee drops its band with it.
    let mut f = fake(20);
    let mut m = machine();
    m.begin(at_row(2), Mods::default(), &mut f);
    m.moved(at_row(4), &mut f);
    f.out.clear();
    f.sel.select_all(&f.source);
    assert!(m.tick(&mut f), "the band is hidden");
    assert!(!m.is_active());
    assert_eq!(f.out, [Outcome::Repaint]);
    assert!(
        !m.cancel(&mut f),
        "Escape then clears the selection, not the band"
    );
    assert_eq!(f.sel.len(), 20);
}

#[test]
fn a_sync_during_the_band_applies_it_again_over_the_new_listing() {
    // C1a hands this to C1d: after a re-sort mid-band, the band covers the rows now under it.
    let mut f = fake(20);
    let mut m = machine();
    m.begin(at_row(2), Mods::default(), &mut f);
    m.moved(at_row(5), &mut f);
    assert_eq!(selected(&f), [2, 3, 4, 5]);
    f.source.sort_by(|a, b| b.id.cmp(&a.id));
    f.sel
        .sync(&f.source, SyncRule::complete(CursorGone::Neighbor));
    m.tick(&mut f);
    // Rows 2 to 5 now hold ids 17 to 14.
    assert_eq!(selected(&f), [17, 16, 15, 14]);
}

#[test]
fn the_speed_grows_with_the_distance_past_the_edge_line() {
    // browser/selection/marquee: Holding the pointer near a viewport edge scrolls the listing and extends the selection each frame.
    let e = EdgeScroll::STRATA;
    assert_eq!(edge_speed(VIEW / 2.0, VIEW, e), 0.0);
    assert_eq!(edge_speed(e.edge, VIEW, e), 0.0);
    let half = edge_speed(VIEW - e.edge / 2.0, VIEW, e);
    let full = edge_speed(VIEW, VIEW, e);
    assert!(half > 0.0 && (full - 2.0 * half).abs() < 1e-3);
    assert_eq!(full, e.max_speed);
    assert_eq!(
        edge_speed(VIEW + 200.0, VIEW, e),
        e.max_speed,
        "beyond the edge it holds"
    );
    assert_eq!(
        edge_speed(-50.0, VIEW, e),
        -e.max_speed,
        "the top edge scrolls up"
    );
    assert_eq!(
        edge_speed(5.0, 2.0 * e.edge, e),
        0.0,
        "too short a view never scrolls"
    );
    assert_eq!(edge_speed(f32::NAN, VIEW, e), 0.0);
}

#[test]
fn a_glide_aimed_by_the_lead_settles_at_the_speed_and_stops_without_overshoot() {
    let spring = rune::tok::motion::GLIDE;
    let step = Duration::from_secs_f64(FRAME);
    for speed in [100.0, 750.0, 1500.0] {
        let mut glide = Glide::default();
        let mut y = 0.0;
        let mut at_1s = 0.0;
        for i in 0..120 {
            glide.aim(spring, y, y + glide_lead(speed, spring, step));
            y = glide.step(spring, y, f32::MAX, FRAME as f32);
            if i == 59 {
                at_1s = y;
            }
        }
        let measured = (y - at_1s) / 1.0;
        assert!(
            (measured - speed).abs() < 0.01 * speed,
            "{speed} pt/s glided at {measured}"
        );
        let target = y + stop_lead(speed, spring);
        glide.aim(spring, y, target);
        let mut furthest = y;
        for _ in 0..60 {
            y = glide.step(spring, y, f32::MAX, FRAME as f32);
            furthest = furthest.max(y);
        }
        assert!(furthest <= target + 0.01, "no overshoot at {speed} pt/s");
        assert!((y - target).abs() < 0.5, "at rest at {speed} pt/s");
    }
}

#[test]
fn holding_at_the_bottom_edge_glides_and_extends_the_selection() {
    // browser/selection/marquee: Holding the pointer near a viewport edge scrolls the listing and extends the selection each frame.
    let mut f = fake(100_000);
    let mut m = machine();
    let mut glide = Glide::default();
    m.begin(at_row(1), Mods::default(), &mut f);
    m.moved(at_view(&f, VIEW - 2.0), &mut f);
    assert!(f.out.contains(&Outcome::WakeAfter(Duration::ZERO)));
    let mut counts = Vec::new();
    for _ in 0..120 {
        frame(&mut f, &mut m, &mut glide);
        counts.push(f.sel.len());
    }
    let scrolled = f.visible.y;
    assert!(scrolled > 1000.0, "it glided down ({scrolled} pt in 2 s)");
    assert!(counts.windows(2).all(|w| w[1] >= w[0]));
    assert!(counts[119] > counts[30] && counts[30] > counts[0]);
    // D1: while it scrolls, the band is applied at most every 100 ms.
    let during: Vec<f64> = f
        .applied
        .iter()
        .map(|(t, _)| *t)
        .filter(|t| *t > 0.0)
        .collect();
    assert!(during.windows(2).all(|w| w[1] - w[0] >= 0.1 - 1e-9));
    // The release applies the band where it ends: rows 1 to the one under the pointer.
    let pointer_row = ((f.visible.y + VIEW - 2.0) / ROW) as usize;
    m.end(&mut f);
    assert_eq!(f.sel.len(), pointer_row);
    assert!(stops(&f.out) >= 1, "the release stops the glide");
}

#[test]
fn leaving_the_edge_or_reaching_the_end_stops_the_glide() {
    let mut f = fake(1_000);
    let mut m = machine();
    let mut glide = Glide::default();
    m.begin(at_row(1), Mods::default(), &mut f);
    m.moved(at_view(&f, VIEW), &mut f);
    for _ in 0..10 {
        frame(&mut f, &mut m, &mut glide);
    }
    f.out.clear();
    m.moved(at_view(&f, VIEW / 2.0), &mut f);
    assert_eq!(stops(&f.out), 1);
    for _ in 0..30 {
        frame(&mut f, &mut m, &mut glide);
    }
    f.out.clear();
    frame(&mut f, &mut m, &mut glide);
    assert!(glides(&f.out).is_empty() && !f.out.contains(&Outcome::WakeAfter(Duration::ZERO)));

    // Back at the edge, it glides to the end of the content and stops there.
    m.moved(at_view(&f, VIEW), &mut f);
    for _ in 0..1_500 {
        frame(&mut f, &mut m, &mut glide);
    }
    let bottom = 1_000.0 * ROW - VIEW;
    assert!((f.visible.y - bottom).abs() < 1.0);
    f.out.clear();
    frame(&mut f, &mut m, &mut glide);
    assert!(glides(&f.out).is_empty(), "nothing more to scroll");
    // The top edge doesn't scroll a view at the top.
    let mut f = fake(1_000);
    m.begin(at_row(5), Mods::default(), &mut f);
    m.moved(at_view(&f, 0.0), &mut f);
    frame(&mut f, &mut m, &mut glide);
    assert!(glides(&f.out).is_empty());
}

#[test]
fn a_still_band_asks_for_no_frames() {
    // Rune warns when a resting window keeps waking; a held, still band costs nothing.
    let mut f = fake(100);
    let mut m = machine();
    m.begin(at_row(2), Mods::default(), &mut f);
    m.moved(at_row(6), &mut f);
    f.out.clear();
    f.now += 1.0;
    assert!(!m.tick(&mut f));
    assert!(f.out.is_empty());
}

#[test]
fn wheel_scrolling_during_a_held_band_extends_it_from_the_same_anchor() {
    // browser/selection/marquee: Wheel scrolling during a held marquee extends the selection without moving the anchor.
    let mut f = fake(1_000);
    let mut m = machine();
    m.begin(at_row(3), Mods::default(), &mut f);
    m.moved(at_view(&f, 300.0), &mut f);
    assert_eq!(f.sel.len(), 8, "rows 3 to 10");
    // Wheel ticks scroll 1,000 pt over four frames under a still pointer.
    for _ in 0..4 {
        f.now += FRAME;
        f.visible.y += 250.0;
        m.tick(&mut f);
    }
    // The scroll stops; the next frame applies what waited.
    f.now += FRAME;
    m.tick(&mut f);
    let pointer_row = ((1000.0 + 300.0) / ROW) as u64;
    assert_eq!(selected(&f), (3..=pointer_row).collect::<Vec<_>>());
}

#[test]
fn rows_scrolled_out_stay_selected_and_rows_above_the_anchor_stay_unselected() {
    // browser/selection/marquee: Entries selected before they scrolled out of view stay selected; entries above the original anchor stay unselected.
    let mut f = fake(1_000);
    let mut m = machine();
    m.begin(at_row(10), Mods::default(), &mut f);
    f.visible.y = 2_000.0;
    m.moved(at_view(&f, 200.0), &mut f);
    f.now += 1.0;
    m.tick(&mut f);
    let below = ((2_000.0 + 200.0) / ROW) as u64;
    assert_eq!(selected(&f), (10..=below).collect::<Vec<_>>());
    // Back up past the anchor: the rows between the anchor and the pointer, and none below.
    f.visible.y = 0.0;
    m.moved(at_row(4), &mut f);
    f.now += 1.0;
    m.tick(&mut f);
    assert_eq!(selected(&f), (4..=10).collect::<Vec<_>>());
}
