//! The element in a headless runtime: input routing to the default click and to installed
//! controllers, effects and intents, what the host hears, memoized rows, virtualization,
//! position restore and what assistive technology reads.

mod fixture;
mod perf;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rune::runtime::ElementInfo;
use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, col};
use strata_kit::look::Look;

use self::fixture::FastSource;
use super::*;
use crate::collection::selection::{CursorGone, Listing, SyncRule};

const W: f32 = 500.0;
const H: f32 = 400.0;
const SCOPE: &str = "list";

#[derive(Default)]
struct Log {
    pointer: Vec<PointerInput>,
    marquee: Vec<&'static str>,
}

struct Host {
    src: FastSource,
    coll: CollectionState,
    look: Look,
    version: u64,
    cursors: Vec<Option<EntryId>>,
    selections: usize,
    intents: Vec<String>,
    last_intent: Option<Intent>,
    /// Rows built (label slot calls).
    built: Rc<Cell<usize>>,
    per_line: usize,
    app_keys: usize,
}

impl Host {
    fn new(n: usize) -> Self {
        Self {
            src: FastSource::new(n),
            coll: CollectionState::new(),
            look: Look::fallback(false),
            version: 1,
            cursors: Vec::new(),
            selections: 0,
            intents: Vec::new(),
            last_intent: None,
            built: Rc::new(Cell::new(0)),
            per_line: 1,
            app_keys: 0,
        }
    }
}

/// N entries per line, for the grid roles.
struct PerLine(usize);

impl Lines for PerLine {
    fn per_line(&self) -> usize {
        self.0
    }

    fn geometry(&self, len: usize, h: f32, w: f32, vh: f32) -> Box<dyn CollectionGeometry> {
        OnePerLine.geometry(len.div_ceil(self.0), h, w, vh)
    }
}

use crate::contract::CollectionGeometry;

fn collection(s: &Host) -> Collection<Host> {
    let built = s.built.clone();
    let mut c = Collection::new(
        SCOPE,
        |s: &Host| (&s.src, &s.coll),
        |s: &mut Host| (&s.src, &mut s.coll),
        |s: &Host| &s.look,
        |s: &Host| s.version,
    )
    .name("Files")
    .label(Slot::new(move |_: &Host, rcx| {
        built.set(built.get() + 1);
        rune::text(rcx.entry.name.to_owned())
    }))
    .on_cursor(|s: &mut Host, id, _| s.cursors.push(id))
    .on_selection(|s: &mut Host, _| s.selections += 1)
    .on_intent(|s: &mut Host, intent, _| {
        s.intents.push(format!("{intent:?}"));
        s.last_intent = Some(intent);
    });
    if s.per_line > 1 {
        c = c.lines(PerLine(s.per_line)).roles(Roles::GRID_CELLS);
    }
    c
}

fn view(s: &Host, _: &mut Cx<Host>) -> El {
    col().size(W, H).child(collection(s).build(s))
}

fn runtime(host: Host) -> Runtime<Host> {
    let mut rt = Runtime::new(host, view, (W, H), Env::new(false, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    rt
}

fn settle(rt: &mut Runtime<Host>) {
    for _ in 0..40 {
        rt.frame(1.0 / 60.0);
    }
}

fn element(rt: &Runtime<Host>, key: &str) -> Option<ElementInfo> {
    rt.elements()
        .into_iter()
        .find(|e| !e.exiting && e.key.as_deref() == Some(key))
}

/// Presses and releases at the middle of `key`'s visible part.
fn click(rt: &mut Runtime<Host>, key: &str) {
    let e = element(rt, key).unwrap_or_else(|| panic!("no element {key}"));
    let [x, y, w, h] = e.visible;
    click_at(rt, x + w / 2.0, y + h / 2.0);
}

/// Presses and releases on `key`'s name, at the start of its row.
fn click_label(rt: &mut Runtime<Host>, key: &str) {
    let e = element(rt, key).unwrap_or_else(|| panic!("no element {key}"));
    let [x, y, _, h] = e.visible;
    click_at(rt, x + 40.0, y + h / 2.0);
}

fn click_at(rt: &mut Runtime<Host>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

fn press(rt: &mut Runtime<Host>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1.0 / 60.0);
}

fn id(i: u64) -> EntryId {
    EntryId::new(i)
}

fn rows(rt: &Runtime<Host>) -> Vec<ElementInfo> {
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting && e.role == "option")
        .collect()
}

// The default pointer.

#[test]
fn a_press_selects_only_that_entry_and_a_press_on_empty_space_clears() {
    let mut rt = runtime(Host::new(5));
    click(&mut rt, "row-1");
    click(&mut rt, "row-3");
    let s = &rt.state;
    assert_eq!(s.coll.selection.selected_ids(&s.src), [id(3)]);
    assert_eq!(s.coll.selection.cursor(), Some(id(3)));
    assert_eq!(
        s.cursors,
        [Some(id(1)), Some(id(3))],
        "the host hears each cursor move"
    );
    // Below the last of five rows.
    click_at(&mut rt, W / 2.0, H - 10.0);
    let s = &rt.state;
    assert!(s.coll.selection.is_empty());
    assert_eq!(
        s.coll.selection.cursor(),
        Some(id(3)),
        "clearing keeps the cursor"
    );
    assert_eq!(s.selections, 3);
}

#[test]
fn a_press_after_scrolling_hits_the_row_under_the_pointer() {
    // Rune rebases a virtual stack's on_drag y; the element must not use it.
    let mut rt = runtime(Host::new(10_000));
    for _ in 0..80 {
        rt.wheel(W / 2.0, H / 2.0, -300.0);
        rt.frame(1.0 / 60.0);
    }
    settle(&mut rt);
    assert!(rt.state.coll.scroll_offset() > 10_000.0);
    let target = rows(&rt)
        .into_iter()
        .find(|e| e.visible[1] > 100.0 && e.visible[1] < H - 60.0 && e.visible[3] > 20.0)
        .expect("a row in view");
    let key = target.key.clone().unwrap();
    click(&mut rt, &key);
    let cursor = rt.state.coll.selection.cursor().unwrap();
    assert_eq!(row_key(cursor), key);
}

// Controllers.

struct Pointer(Rc<RefCell<Log>>);

impl PointerController for Pointer {
    fn pointer(&mut self, input: &PointerInput, cx: &mut Ctx<'_>) -> Flow {
        self.0.borrow_mut().pointer.push(*input);
        if input.phase == PointerPhase::Down
            && let Some(i) = input.hit.index
        {
            if input.button == rune::PointerButton::Secondary {
                let id = cx.source.id_at(i);
                cx.emit(Effect::Intent(Intent::ContextMenu {
                    target: Some(id),
                    anchor: crate::contract::Rect::new(input.window.x, input.window.y, 0.0, 0.0),
                }));
            } else if input.mods.shift {
                cx.emit(Effect::BeginMarquee {
                    at: input.point,
                    mods: input.mods,
                });
            } else {
                cx.apply(|sel, src| sel.toggle(src, i));
                let id = cx.source.id_at(i);
                cx.emit(Effect::Intent(Intent::Activate {
                    id,
                    how: Activation::Click,
                }));
            }
        }
        Flow::Handled
    }
}

struct Keys;

impl KeyController for Keys {
    fn key(&mut self, input: &KeyInput<'_>, cx: &mut Ctx<'_>) -> Flow {
        match input.event.key {
            Key::End => {
                let last = cx.source.len() - 1;
                cx.apply(|sel, src| sel.select_only(src, last));
                Flow::Handled
            }
            _ => Flow::Pass,
        }
    }
}

struct Band(Rc<RefCell<Log>>, bool);

impl MarqueeController for Band {
    fn begin(&mut self, _at: Point, _mods: Mods, _cx: &mut Ctx<'_>) {
        self.1 = true;
        self.0.borrow_mut().marquee.push("begin");
    }
    fn moved(&mut self, _at: Point, _cx: &mut Ctx<'_>) {
        self.0.borrow_mut().marquee.push("moved");
    }
    fn end(&mut self, _cx: &mut Ctx<'_>) {
        self.1 = false;
        self.0.borrow_mut().marquee.push("end");
    }
    fn cancel(&mut self, _cx: &mut Ctx<'_>) -> bool {
        let was = std::mem::replace(&mut self.1, false);
        if was {
            self.0.borrow_mut().marquee.push("cancel");
        }
        was
    }
    fn active(&self) -> bool {
        self.1
    }
    fn band(&self) -> Option<crate::contract::Rect> {
        None
    }
}

fn with_controllers(n: usize) -> (Runtime<Host>, Rc<RefCell<Log>>) {
    let log = Rc::new(RefCell::new(Log::default()));
    let mut host = Host::new(n);
    host.coll.controllers.pointer = Some(Box::new(Pointer(log.clone())));
    host.coll.controllers.keyboard = Some(Box::new(Keys));
    host.coll.controllers.marquee = Some(Box::new(Band(log.clone(), false)));
    (runtime(host), log)
}

#[test]
fn an_installed_pointer_controller_replaces_the_default_and_its_intents_reach_the_host() {
    let (mut rt, log) = with_controllers(20);
    click_label(&mut rt, "row-2");
    click_label(&mut rt, "row-4");
    let s = &rt.state;
    assert_eq!(
        s.coll.selection.selected_ids(&s.src),
        [id(2), id(4)],
        "the controller toggles where the default would replace"
    );
    assert_eq!(s.intents.len(), 2);
    assert!(s.intents[1].starts_with("Activate { id: EntryId(4)"));
    let log = log.borrow();
    let downs: Vec<_> = log
        .pointer
        .iter()
        .filter(|p| p.phase == PointerPhase::Down)
        .collect();
    assert_eq!(downs.len(), 2);
    assert_eq!(downs[1].hit.index, Some(4));
    assert_eq!(
        downs[1].hit.region,
        Region::Name,
        "a press on the name is on the name"
    );
    assert!(log.pointer.iter().any(|p| p.phase == PointerPhase::Up));
}

#[test]
fn a_press_on_blank_row_space_is_in_the_gutter() {
    let (mut rt, log) = with_controllers(20);
    let row = element(&rt, "row-3").unwrap();
    let [x, y, w, h] = row.visible;
    click_at(&mut rt, x + w - 30.0, y + h / 2.0);
    let log = log.borrow();
    let down = log
        .pointer
        .iter()
        .find(|p| p.phase == PointerPhase::Down)
        .unwrap();
    assert_eq!(
        down.hit,
        Hit {
            index: Some(3),
            region: Region::Gutter
        }
    );
}

#[test]
fn a_secondary_press_reaches_the_controller_and_its_menu_intent_the_host() {
    let (mut rt, _log) = with_controllers(20);
    let e = element(&rt, "row-5").unwrap();
    let [ex, ey, ew, eh] = e.visible;
    let (x, y) = (ex + ew / 2.0, ey + eh / 2.0);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.intents.len(), 1);
    assert!(rt.state.intents[0].starts_with("ContextMenu { target: Some(EntryId(5))"));
}

#[test]
fn a_marquee_takes_the_gesture_it_begins_and_escape_cancels_it() {
    let (mut rt, log) = with_controllers(40);
    let e = element(&rt, "row-2").unwrap();
    let [ex, ey, _, eh] = e.visible;
    let (x, y) = (ex + 30.0, ey + eh / 2.0);
    rt.physical_input(rune::PhysicalInput::Modifiers(Mods {
        shift: true,
        ..Mods::default()
    }));
    rt.pointer_down(x, y);
    rt.frame(1.0 / 60.0);
    rt.physical_input(rune::PhysicalInput::Modifiers(Mods::default()));
    for k in 1..4 {
        rt.pointer_move(x, y + 30.0 * k as f32);
        rt.frame(1.0 / 60.0);
    }
    rt.pointer_up(x, y + 90.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(
        log.borrow().marquee,
        ["begin", "moved", "moved", "moved", "end"]
    );
    let moves_to_pointer = log
        .borrow()
        .pointer
        .iter()
        .filter(|p| p.phase == PointerPhase::Move)
        .count();
    assert_eq!(
        moves_to_pointer, 0,
        "the band's moves don't reach the pointer controller"
    );

    rt.physical_input(rune::PhysicalInput::Modifiers(Mods {
        shift: true,
        ..Mods::default()
    }));
    rt.pointer_down(x, y);
    rt.frame(1.0 / 60.0);
    rt.physical_input(rune::PhysicalInput::Modifiers(Mods::default()));
    press(&mut rt, Key::Escape);
    assert_eq!(log.borrow().marquee.last(), Some(&"cancel"));
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

#[test]
fn a_key_the_controller_takes_moves_the_cursor_into_view_and_turns_keyboard_mode_on() {
    let (mut rt, _log) = with_controllers(5_000);
    click(&mut rt, "row-1");
    press(&mut rt, Key::Tab);
    press(&mut rt, Key::End);
    settle(&mut rt);
    let s = &rt.state;
    assert_eq!(s.coll.selection.cursor(), Some(id(4_999)));
    assert!(s.coll.keyboard_mode());
    let last = element(&rt, "row-4999").expect("the cursor's row is mounted");
    assert!(last.visible[3] > 20.0, "and shows");
    assert_eq!(rt.state.cursors.last(), Some(&Some(id(4_999))));
}

#[test]
fn a_key_no_controller_takes_goes_on_to_the_app() {
    let mut host = Host::new(5);
    host.coll.controllers.keyboard = Some(Box::new(Keys));
    let mut rt = runtime(host);
    rt.on_key = Some(|s: &mut Host, _: &KeyEvent, _: &mut rune::Tx| {
        s.app_keys += 1;
        false
    });
    click(&mut rt, "row-1");
    press(&mut rt, Key::Function(5));
    assert_eq!(rt.state.app_keys, 1);
    assert!(
        !rt.state.coll.keyboard_mode(),
        "an unhandled key leaves keyboard mode off"
    );
    press(&mut rt, Key::End);
    assert_eq!(
        rt.state.app_keys, 1,
        "a handled key stops at the collection"
    );
}

// What the host hears and what rows rebuild.

#[test]
fn a_selection_change_rebuilds_only_the_rows_whose_state_changed() {
    let mut rt = runtime(Host::new(200));
    settle(&mut rt);
    click(&mut rt, "row-3");
    settle(&mut rt);
    let before = rt.state.built.get();
    click(&mut rt, "row-6");
    settle(&mut rt);
    assert_eq!(
        rt.state.built.get() - before,
        2,
        "the row that lost the selection and the one that gained it"
    );
    let before = rt.state.built.get();
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert_eq!(
        rt.state.built.get(),
        before,
        "a commit that changes nothing rebuilds nothing"
    );
}

#[test]
fn scrolling_runs_no_view() {
    let views = Rc::new(Cell::new(0usize));
    thread_local! { static VIEWS: Cell<usize> = const { Cell::new(0) }; }
    fn counted(s: &Host, cx: &mut Cx<Host>) -> El {
        VIEWS.with(|v| v.set(v.get() + 1));
        view(s, cx)
    }
    let _ = views;
    let mut rt = Runtime::new(Host::new(20_000), counted, (W, H), Env::new(false, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    let before = VIEWS.with(Cell::get);
    for _ in 0..120 {
        rt.wheel(W / 2.0, H / 2.0, -40.0);
        rt.frame(1.0 / 60.0);
    }
    assert!(rt.state.coll.scroll_offset() > 1_000.0);
    assert_eq!(VIEWS.with(Cell::get), before);
}

#[test]
fn a_hundred_thousand_entries_build_only_what_shows() {
    let rt = runtime(Host::new(100_000));
    let built = rows(&rt).len();
    assert!(
        (5..200).contains(&built),
        "{built} rows built for a {H} pt viewport"
    );
}

// Keeping in step, and position restore.

#[test]
fn a_refresh_keeps_the_cursor_row_where_it_shows() {
    let mut rt = runtime(Host::new(3_000));
    for _ in 0..20 {
        rt.wheel(W / 2.0, H / 2.0, -200.0);
        rt.frame(1.0 / 60.0);
    }
    settle(&mut rt);
    let target = rows(&rt)
        .into_iter()
        .find(|e| e.visible[1] > 150.0)
        .and_then(|e| e.key)
        .unwrap();
    click(&mut rt, &target);
    let shown = element(&rt, &target).unwrap().visible;
    rt.update(|s, tx| {
        s.src.insert_top(25);
        s.coll
            .sync(SCOPE, &s.src, SyncRule::complete(CursorGone::Neighbor), tx);
    });
    settle(&mut rt);
    assert_eq!(element(&rt, &target).unwrap().visible, shown);
}

#[test]
fn a_position_comes_back_with_its_scroll_in_the_same_layout_else_the_cursor_shows() {
    let mut rt = runtime(Host::new(3_000));
    rt.update(|s, tx| {
        s.coll.selection.select_only(&s.src, 2_000);
        s.coll.reveal(SCOPE, &s.src, 2_000, tx);
    });
    settle(&mut rt);
    let offset = rt.state.coll.scroll_offset();
    assert!(offset > 1_000.0);
    let position = rt.state.coll.position(&rt.state.src);
    assert_eq!(position.cursor, Some(id(2_000)));

    // Leave, then come back.
    rt.update(|s, tx| {
        s.coll.enter(SCOPE, &s.src, None, Listing::Complete, tx);
    });
    settle(&mut rt);
    assert_eq!(rt.state.coll.scroll_offset(), 0.0);
    let p = position.clone();
    rt.update(move |s, tx| {
        s.coll.enter(SCOPE, &s.src, Some(&p), Listing::Complete, tx);
    });
    settle(&mut rt);
    assert_eq!(rt.state.coll.scroll_offset(), offset);
    assert_eq!(rt.state.coll.selection.cursor(), Some(id(2_000)));

    // In another layout, the offset doesn't apply: the cursor scrolls into view.
    rt.update(|s, tx| {
        s.coll.enter(SCOPE, &s.src, None, Listing::Complete, tx);
        s.coll.set_layout_key(7);
    });
    settle(&mut rt);
    let p = position.clone();
    rt.update(move |s, tx| {
        s.coll.enter(SCOPE, &s.src, Some(&p), Listing::Complete, tx);
    });
    settle(&mut rt);
    let row = element(&rt, "row-2000").expect("the cursor's row is mounted");
    assert!(row.visible[3] > 20.0, "and shows");
}

#[test]
fn a_restore_waits_for_its_listing_and_a_press_before_then_cancels_it() {
    let mut rt = runtime(Host::new(50));
    let position = {
        let s = &mut rt.state;
        s.coll.selection.select_only(&s.src, 7);
        s.coll.position(&s.src)
    };
    let p = position.clone();
    rt.update(move |s, tx| {
        s.coll.enter(SCOPE, &s.src, Some(&p), Listing::Loading, tx);
    });
    rt.frame(1.0 / 60.0);
    assert!(rt.state.coll.restore_pending());
    rt.update(|s, tx| {
        s.coll
            .sync(SCOPE, &s.src, SyncRule::complete(CursorGone::Neighbor), tx);
    });
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.coll.restore_pending());
    assert_eq!(
        rt.state.coll.selection.cursor(),
        Some(id(7)),
        "it applies once complete"
    );

    let p = position.clone();
    rt.update(move |s, tx| {
        s.coll.selection.clear();
        s.coll.enter(SCOPE, &s.src, Some(&p), Listing::Loading, tx);
    });
    rt.frame(1.0 / 60.0);
    click(&mut rt, "row-2");
    assert!(!rt.state.coll.restore_pending(), "the press cancelled it");
    rt.update(|s, tx| {
        s.coll
            .sync(SCOPE, &s.src, SyncRule::complete(CursorGone::Neighbor), tx);
    });
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.coll.selection.cursor(), Some(id(2)));
}

#[test]
fn a_re_sort_with_a_new_listing_version_shows_the_new_order() {
    let mut rt = runtime(Host::new(30));
    rt.update(|s, _| {
        s.src.reverse();
        s.version += 1;
    });
    settle(&mut rt);
    let first = rows(&rt)
        .into_iter()
        .min_by(|a, b| a.rect[1].total_cmp(&b.rect[1]))
        .and_then(|e| e.key);
    assert_eq!(first.as_deref(), Some("row-29"));
}

// Accessibility and keys.

#[test]
fn the_cursor_row_is_the_active_descendant_and_rows_say_they_are_selected() {
    let mut rt = runtime(Host::new(12));
    click(&mut rt, "row-4");
    let tree = rt.accessibility_json().to_string();
    assert!(tree.contains(r#""activeDescendant":"row-4""#), "{tree}");
    let selected: Vec<_> = rows(&rt).into_iter().filter(|e| e.item_selected).collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].key.as_deref(), Some("row-4"));
}

#[test]
fn a_grid_packs_entries_into_lines_of_cells() {
    let mut host = Host::new(10);
    host.per_line = 4;
    let rt = runtime(host);
    let all = rt.elements();
    assert_eq!(all.iter().filter(|e| e.role == "gridcell").count(), 10);
    assert_eq!(all.iter().filter(|e| e.role == "row").count(), 3);
    assert!(all.iter().any(|e| e.role == "grid"));
}

#[test]
fn part_keys_are_full_paths_under_the_collection() {
    assert_eq!(row_key(id(42)), "row-42");
    assert_eq!(row_path("list", id(42)), "list/viewport/rows/row-42");
    assert_eq!(
        part_path("list", id(42), ICON),
        "list/viewport/rows/row-42/icon"
    );
    let rt = runtime(Host::new(3));
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.path.ends_with("row-2") && e.role == "option")
    );
}

// The seam requests from C1b's plan (N1–N7).

/// A pointer controller that records every input and key it hears, and does what `on_move`
/// says on the first move of each gesture.
struct Recorder {
    log: Rc<RefCell<Log>>,
    keys: Rc<Cell<usize>>,
    on_move: fn(&PointerInput, &mut Ctx<'_>),
    moved: bool,
}

impl PointerController for Recorder {
    fn pointer(&mut self, input: &PointerInput, cx: &mut Ctx<'_>) -> Flow {
        self.log.borrow_mut().pointer.push(*input);
        match input.phase {
            PointerPhase::Down => self.moved = false,
            PointerPhase::Move if !self.moved => {
                self.moved = true;
                (self.on_move)(input, cx);
            }
            _ => {}
        }
        Flow::Handled
    }

    fn key_seen(&mut self, _input: &KeyInput<'_>) {
        self.keys.set(self.keys.get() + 1);
    }
}

fn recorder(
    n: usize,
    on_move: fn(&PointerInput, &mut Ctx<'_>),
) -> (Runtime<Host>, Rc<RefCell<Log>>, Rc<Cell<usize>>) {
    let log = Rc::new(RefCell::new(Log::default()));
    let keys = Rc::new(Cell::new(0));
    let mut host = Host::new(n);
    host.coll.controllers.pointer = Some(Box::new(Recorder {
        log: log.clone(),
        keys: keys.clone(),
        on_move,
        moved: false,
    }));
    host.coll.controllers.keyboard = Some(Box::new(Keys));
    host.coll.controllers.marquee = Some(Box::new(Band(log.clone(), false)));
    (runtime(host), log, keys)
}

/// N1: the name, the rest of the content (the icon) and the gutter are told apart.
#[test]
fn a_press_on_the_icon_is_content_and_on_the_name_is_the_name() {
    let (mut rt, log, _) = recorder(20, |_, _| {});
    let [x, y, w, h] = element(&rt, "row-3").unwrap().visible;
    let look = Look::fallback(false);
    let icon_mid =
        strata_kit::slots::collection::row_inset(&look) + look.space.xs + look.icon_size.md / 2.0;
    for px in [x + icon_mid, x + 40.0, x + w - 30.0] {
        click_at(&mut rt, px, y + h / 2.0);
    }
    let regions: Vec<_> = log
        .borrow()
        .pointer
        .iter()
        .filter(|p| p.phase == PointerPhase::Down)
        .map(|p| p.hit.region)
        .collect();
    assert_eq!(regions, [Region::Content, Region::Name, Region::Gutter]);
    assert!(Region::Content.on_entry() && Region::Name.on_entry() && !Region::Gutter.on_entry());
}

/// N2: the default pointer finishes a plain click with `Clicked`; a drag or a modified click
/// doesn't.
#[test]
fn a_plain_click_finishes_with_a_clicked_intent_and_a_drag_does_not() {
    let mut rt = runtime(Host::new(20));
    click(&mut rt, "row-4");
    assert!(matches!(rt.state.last_intent, Some(Intent::Clicked { id }) if id == EntryId::new(4)));

    rt.state.last_intent = None;
    let [x, y, w, h] = element(&rt, "row-6").unwrap().visible;
    let (x, y) = (x + w / 2.0, y + h / 2.0);
    rt.pointer_down(x, y);
    rt.frame(1.0 / 60.0);
    rt.pointer_move(x, y + 20.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x, y + 20.0);
    rt.frame(1.0 / 60.0);
    assert!(
        rt.state.last_intent.is_none(),
        "a press that travelled isn't a click"
    );

    rt.physical_input(rune::PhysicalInput::Modifiers(Mods {
        shift: true,
        ..Mods::default()
    }));
    click_at(&mut rt, x, y);
    rt.physical_input(rune::PhysicalInput::Modifiers(Mods::default()));
    assert!(
        rt.state.last_intent.is_none(),
        "a modified click isn't a plain click"
    );
}

/// N3: the pointer controller hears every key, taken or not.
#[test]
fn the_pointer_controller_sees_every_key_the_collection_hears() {
    let (mut rt, _, keys) = recorder(20, |_, _| {});
    click(&mut rt, "row-2");
    press(&mut rt, Key::End);
    press(&mut rt, Key::Function(2));
    assert_eq!(
        keys.get(),
        2,
        "one key the keyboard controller took, one it passed"
    );
}

/// N4: drop zones on folder rows don't take presses, so the controller still hears them.
#[test]
fn presses_on_rows_with_drop_zones_reach_the_controller() {
    fn with_drops(s: &Host, _: &mut Cx<Host>) -> El {
        col()
            .size(W, H)
            .child(collection(s).on_drop(|_: &mut Host, _, _, _| {}).build(s))
    }
    let mut rt = Runtime::new(Host::new(20), with_drops, (W, H), Env::new(false, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    // Entry 10 is a folder: its row carries a drop zone.
    click(&mut rt, "row-10");
    assert_eq!(rt.state.coll.selection.cursor(), Some(id(10)));
}

/// N5: a controller can be tested with a `Ctx` and no runtime.
#[test]
fn a_controller_runs_against_a_ctx_without_a_runtime() {
    let src = FastSource::new(10);
    let geometry = OnePerLine.geometry(10, 26.0, 300.0, 200.0);
    let mut sel = crate::collection::selection::Selection::new();
    let mut cx = Ctx::new(
        &src,
        &*geometry,
        crate::contract::Rect::new(0.0, 0.0, 300.0, 200.0),
        0.0,
        &mut sel,
    );
    let hit = Hit {
        index: Some(3),
        region: Region::Name,
    };
    let down = PointerInput::new(
        PointerPhase::Down,
        rune::PointerButton::Primary,
        Point::new(40.0, 90.0),
        hit,
    );
    let up = PointerInput {
        phase: PointerPhase::Up,
        ..down
    };
    default_pointer(&down, &mut cx);
    default_pointer(&up, &mut cx);
    assert!(cx.changed().selection && cx.changed().cursor);
    assert!(
        matches!(cx.effects(), [Effect::Intent(Intent::Clicked { id })] if *id == EntryId::new(3))
    );
    let _ = KeyInput::new(&KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    drop(cx);
    assert_eq!(sel.cursor(), Some(EntryId::new(3)));
}

/// N6: a band begun by a move takes that move too.
#[test]
fn a_marquee_begun_by_a_move_hears_that_move() {
    let (mut rt, log, _) = recorder(40, |input, cx| {
        cx.emit(Effect::BeginMarquee {
            at: input.point,
            mods: input.mods,
        });
    });
    let [x, y, w, h] = element(&rt, "row-2").unwrap().visible;
    let (x, y) = (x + w - 20.0, y + h / 2.0);
    rt.pointer_down(x, y);
    rt.frame(1.0 / 60.0);
    rt.pointer_move(x, y + 40.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x, y + 40.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(log.borrow().marquee, ["begin", "moved", "end"]);
}

/// N7: a drag intent carries the pointer's window point.
#[test]
fn a_drag_intent_carries_the_window_point() {
    let (mut rt, _, _) = recorder(40, |input, cx| {
        if let Some(i) = input.hit.index {
            let ids = vec![cx.source.id_at(i)];
            cx.emit(Effect::Intent(Intent::BeginDrag {
                ids,
                window: input.window,
            }));
        }
    });
    let [x, y, _, h] = element(&rt, "row-5").unwrap().visible;
    let (x, y) = (x + 40.0, y + h / 2.0);
    rt.pointer_down(x, y);
    rt.frame(1.0 / 60.0);
    rt.pointer_move(x + 12.0, y + 3.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x + 12.0, y + 3.0);
    rt.frame(1.0 / 60.0);
    match &rt.state.last_intent {
        Some(Intent::BeginDrag { ids, window }) => {
            assert_eq!(ids, &[EntryId::new(5)]);
            assert_eq!(*window, Point::new(x + 12.0, y + 3.0));
        }
        other => panic!("no drag intent: {other:?}"),
    }
}
