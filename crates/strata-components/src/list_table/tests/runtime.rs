//! The table in a headless runtime: what the scripts can't hold or see. Modifier-free gestures
//! the scripts check stay there; these check state the tree doesn't show (a width mid-drag, the
//! cursor shape, base units), two tables over one layout, and assistive technology's press.

use rune::runtime::ElementInfo;
use rune::{Cursor, Cx, El, Env, Runtime, col};
use strata_kit::look::Look;
use strata_kit::shape::LookParams;

use super::super::*;
use crate::collection::element::{Collection, CollectionState};
use crate::contract::{EntryId, EntryKind, ListEntry, ListSource};

const W: f32 = 900.0;
const H: f32 = 500.0;

struct Pane {
    listing: ListSource,
    coll: CollectionState,
    table: ListTableState,
    widths: ColumnWidths,
}

struct Host {
    panes: Vec<Pane>,
    layout: ColumnLayout,
    look: Look,
    sort: Option<SortShown>,
    sorts: Vec<SortIntent>,
    layouts: Vec<LayoutChange>,
    widths_heard: Vec<ColumnWidths>,
    shown: bool,
}

fn types(i: u64) -> &'static str {
    match i {
        7 => "A very long type description that only one row has",
        40 => "An even longer type description, far below the rows on screen ever",
        _ => "Text",
    }
}

fn columns() -> Vec<Column<Host>> {
    vec![
        Column::primary("name", "Name")
            .width(160.0)
            .min(160.0)
            .sortable(),
        Column::new("mode", "Mode", |_: &Host, _| {
            Cell::text("-rw-r--r--  644").mono()
        })
        .width(160.0)
        .min(80.0),
        Column::new("size", "Size", |_: &Host, _| Cell::text("1 kB"))
            .width(90.0)
            .min(70.0)
            .sortable(),
        Column::new("type", "Type", |_: &Host, rcx| {
            Cell::text(types(rcx.entry.id.get()))
        })
        .width(120.0)
        .min(80.0)
        .max(1000.0)
        .sortable(),
        Column::new("modified", "Modified", |_: &Host, _| {
            Cell::text("2026-01-01 00:00")
        })
        .width(150.0)
        .min(110.0)
        .sortable(),
    ]
}

fn pane(n: u64) -> Pane {
    let mut coll = CollectionState::new();
    coll.controllers.pointer = Some(Box::new(crate::collection::pointer::PointerIntent::new()));
    Pane {
        listing: ListSource::new(
            (0..n)
                .map(|i| {
                    ListEntry::new(
                        EntryId::new(i + 1),
                        format!("file {i:03}.txt"),
                        EntryKind::File,
                        "",
                    )
                })
                .collect(),
        ),
        coll,
        table: ListTableState::new(),
        widths: ColumnWidths::new(),
    }
}

impl Host {
    fn new(panes: usize, look: Look) -> Self {
        Self {
            panes: (0..panes).map(|_| pane(60)).collect(),
            layout: ColumnLayout::new(columns().iter().map(|c| c.id().clone())),
            look,
            sort: Some(SortShown {
                column: ColumnId::new("name"),
                dir: SortDir::Ascending,
            }),
            sorts: Vec::new(),
            layouts: Vec::new(),
            widths_heard: Vec::new(),
            shown: true,
        }
    }
}

fn table(i: usize) -> ListTable<Host> {
    ListTable::new(
        &format!("t{i}"),
        columns(),
        move |s: &Host| ListParts {
            source: &s.panes[i].listing,
            collection: &s.panes[i].coll,
            table: &s.panes[i].table,
            layout: &s.layout,
            widths: &s.panes[i].widths,
            sort: s.sort.clone(),
            look: &s.look,
        },
        move |s: &mut Host| &mut s.panes[i].table,
    )
    .on_sort(|s: &mut Host, intent, _| s.sorts.push(intent))
    .on_layout(|s: &mut Host, change, _| {
        s.layout = change.layout.clone();
        s.layouts.push(change);
    })
    .on_widths(move |s: &mut Host, widths, _| {
        s.panes[i].widths = widths.clone();
        s.widths_heard.push(widths);
    })
}

fn base(i: usize) -> Collection<Host> {
    Collection::new(
        format!("list{i}"),
        move |s: &Host| (&s.panes[i].listing, &s.panes[i].coll),
        move |s: &mut Host| {
            let p = &mut s.panes[i];
            (&p.listing, &mut p.coll)
        },
        |s: &Host| &s.look,
        |_: &Host| 1,
    )
}

fn view(s: &Host, _: &mut Cx<Host>) -> El {
    let mut layer = Vec::new();
    let mut page = rune::row().size(W, H);
    if s.shown {
        let w = W / s.panes.len() as f32;
        for i in 0..s.panes.len() {
            page = page.child(
                col()
                    .key(format!("pane{i}"))
                    .w(w)
                    .h_full()
                    .child(table(i).build(s, base(i), &mut layer)),
            );
        }
    }
    rune_kit::root(page, layer)
}

fn runtime(host: Host) -> Runtime<Host> {
    let mut rt = Runtime::new(host, view, (W, H), Env::new(false, 1.0));
    rt.frame(0.0);
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<Host>) {
    for _ in 0..10 {
        rt.frame(1.0 / 60.0);
    }
}

fn element(rt: &Runtime<Host>, path: &str) -> Option<ElementInfo> {
    rt.elements()
        .into_iter()
        .find(|e| !e.exiting && (e.path == path || e.path.ends_with(&format!("/{path}"))))
}

fn rect(rt: &Runtime<Host>, path: &str) -> [f32; 4] {
    element(rt, path)
        .unwrap_or_else(|| panic!("no element {path}"))
        .visible
}

/// Presses at `(x, y)`, moves by `dx` in `steps`, then releases.
fn drag(rt: &mut Runtime<Host>, x: f32, y: f32, dx: f32, steps: usize) {
    rt.pointer_down(x, y);
    rt.frame(1.0 / 60.0);
    for k in 1..=steps {
        rt.pointer_move(x + dx * k as f32 / steps as f32, y);
        rt.frame(1.0 / 60.0);
    }
    rt.pointer_up(x + dx, y);
    settle(rt);
}

fn heading(rt: &Runtime<Host>, pane: usize, id: &str) -> [f32; 4] {
    rect(rt, &format!("t{pane}-headings/{}", heading_key(id)))
}

/// list: "Dragging a heading's right edge resizes that column and follows the pointer without
/// an initial jump" (its width mid-drag, which the tree doesn't show; `resize.json` checks the
/// host hears it on release).
#[test]
fn a_resize_follows_the_pointer_from_the_drawn_width_and_the_host_hears_it_once() {
    let mut rt = runtime(Host::new(1, Look::fallback(false)));
    let [x, y, w, h] = heading(&rt, 0, "size");
    let (px, py) = (x + w - 3.0, y + h / 2.0);
    rt.pointer_down(px, py);
    rt.frame(1.0 / 60.0);
    rt.pointer_move(px + 12.0, py);
    rt.frame(1.0 / 60.0);
    let (col, live) = rt.state.panes[0].table.resizing().expect("resizing");
    assert_eq!(col.as_str(), "size");
    assert_eq!(live, 90.0 + 12.0, "from the drawn 90 pt, no jump");
    rt.pointer_move(px + 30.0, py);
    rt.frame(1.0 / 60.0);
    assert!(
        rt.state.widths_heard.is_empty(),
        "nothing until the release"
    );
    rt.pointer_up(px + 30.0, py);
    settle(&mut rt);
    assert_eq!(rt.state.widths_heard.len(), 1);
    assert_eq!(
        rt.state.panes[0].widths.get(&ColumnId::new("size")),
        Some(120.0)
    );
    assert_eq!(
        rt.state.panes[0].widths.get(&ColumnId::new("name")),
        None,
        "list: Name is saved only after it is resized itself"
    );
    assert!(
        rt.state.layouts.is_empty() && rt.state.sorts.is_empty(),
        "never a reorder or a sort"
    );
}

/// Owner ruling (Q4): at twice the text size the columns are drawn twice as wide, and a resize
/// hands the host base units.
#[test]
fn at_a_larger_text_size_a_resize_hands_the_host_base_units() {
    let big = Look::fallback(false).with_params(&LookParams {
        text_size: 26,
        ..LookParams::default()
    });
    let mut rt = runtime(Host::new(1, big));
    let [x, y, w, h] = heading(&rt, 0, "size");
    assert_eq!(w, 180.0, "90 base points at twice the size");
    drag(&mut rt, x + w - 3.0, y + h / 2.0, 40.0, 4);
    assert_eq!(
        rt.state.panes[0].widths.get(&ColumnId::new("size")),
        Some(110.0),
        "40 pt drawn is 20 base points"
    );
}

/// list: "Double-clicking a heading's right edge fits the column to its widest content" (as
/// Strata measures it: the heading and the cells on screen, not rows out of view).
#[test]
fn a_double_press_on_the_edge_fits_the_column_to_the_widest_cell_on_screen() {
    let mut rt = runtime(Host::new(1, Look::fallback(false)));
    let [x, y, w, h] = heading(&rt, 0, "type");
    let (px, py) = (x + w - 3.0, y + h / 2.0);
    rt.pointer_down(px, py);
    rt.pointer_up(px, py);
    rt.frame(0.1);
    rt.pointer_down(px, py);
    rt.pointer_up(px, py);
    settle(&mut rt);
    let fitted = rt.state.panes[0]
        .widths
        .get(&ColumnId::new("type"))
        .expect("fitted");
    let look = Look::fallback(false);
    let measure = |t: &str| rune::text::measure(t, &[], look.kit.ty.ui_compact, None).0;
    let on_screen = measure(types(7)) + 2.0 * look.space.sm;
    assert!(
        (fitted - on_screen).abs() <= 1.0,
        "{fitted} fits the widest cell on screen ({on_screen})"
    );
    assert!(
        fitted < measure(types(40)),
        "a row out of view isn't measured"
    );
    let shown = rt.state.panes[0].coll.visible_range();
    assert!(shown.contains(&6) && !shown.contains(&39), "{shown:?}");
}

/// column-layout: "Pressing a List heading and dragging it horizontally shows a grabbing
/// cursor and a 3 px accent inset line at the drop point. The headings do not move during the
/// drag", with "The line sits on the left edge of the heading the column would precede", "For
/// a drop at the end, the line sits on the last heading's right edge", "A drop on the
/// column's current position shows no line" and "Moving the pointer outside the heading row
/// hides the line" (the cursor, the line's place and that headings keep theirs; the PNG golden
/// `c4-reorder` shows the line).
#[test]
fn a_heading_drag_grabs_marks_the_drop_point_and_moves_no_heading() {
    let mut rt = runtime(Host::new(1, Look::fallback(false)));
    let [x, y, w, h] = heading(&rt, 0, "mode");
    let before: Vec<[f32; 4]> = ["name", "mode", "size", "type", "modified"]
        .iter()
        .map(|c| heading(&rt, 0, c))
        .collect();
    let (px, py) = (x + w / 2.0, y + h / 2.0);
    rt.pointer_down(px, py);
    rt.frame(1.0 / 60.0);
    rt.pointer_move(px + 6.0, py);
    rt.frame(1.0 / 60.0);
    assert!(
        rt.state.panes[0].table.reordering().is_none(),
        "below 10 pt"
    );
    assert!(element(&rt, "reorder-line").is_none());
    // Over Size's left half: Mode's own place, no line.
    rt.pointer_move(px + 20.0, py);
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.cursor(), Cursor::Grabbing);
    assert!(element(&rt, "reorder-line").is_none(), "its own place");
    // Into Type's left half.
    let [tx, _, tw, _] = heading(&rt, 0, "type");
    rt.pointer_move(tx + 10.0, py);
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    let line = element(&rt, "reorder-line").expect("a line");
    assert!(line.path.contains("head-type/"), "{}", line.path);
    assert!(line.visible[0] < tx + tw / 2.0, "on Type's left edge");
    // Out of the strip, into the rows: no line.
    rt.pointer_move(tx + 10.0, py + 120.0);
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert!(
        element(&rt, "reorder-line").is_none(),
        "outside the heading row"
    );
    // Past the last heading.
    let [mx, _, mw, _] = heading(&rt, 0, "modified");
    rt.pointer_move(mx + mw - 2.0, py);
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    let line = element(&rt, "reorder-line").expect("a line");
    assert!(line.path.contains("head-modified/"), "{}", line.path);
    assert!(
        line.visible[0] > mx + mw / 2.0,
        "on the last heading's right edge"
    );
    let after: Vec<[f32; 4]> = ["name", "mode", "size", "type", "modified"]
        .iter()
        .map(|c| heading(&rt, 0, c))
        .collect();
    assert_eq!(before, after, "the headings don't move");
    rt.key(rune::KeyEvent {
        key: rune::Key::Escape,
        mods: rune::Mods::default(),
    });
    settle(&mut rt);
    rt.pointer_up(mx + mw - 2.0, py);
    settle(&mut rt);
    assert!(element(&rt, "reorder-line").is_none());
    assert!(rt.state.layouts.is_empty());
}

/// column-layout: "A cancelled drag, or headings unmapped mid-drag, leaves the order unchanged
/// and clears the line and cursor" (the unmapped half, with the host's `reset`; Escape is
/// `reorder-escape.json`'s).
#[test]
fn a_table_removed_mid_drag_leaves_the_order_and_comes_back_with_no_line() {
    let mut rt = runtime(Host::new(1, Look::fallback(false)));
    let [x, y, w, h] = heading(&rt, 0, "mode");
    let [tx, ..] = heading(&rt, 0, "type");
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_move(tx + 10.0, y + h / 2.0);
    settle(&mut rt);
    assert!(element(&rt, "reorder-line").is_some());
    rt.update(|s, _| {
        s.shown = false;
        s.panes[0].table.reset();
    });
    settle(&mut rt);
    rt.pointer_up(tx + 10.0, y + h / 2.0);
    settle(&mut rt);
    rt.update(|s, _| s.shown = true);
    settle(&mut rt);
    assert!(element(&rt, "reorder-line").is_none());
    assert_ne!(rt.cursor(), Cursor::Grabbing);
    assert!(rt.state.layouts.is_empty(), "the order is unchanged");
}

/// column-layout: "Every browser window and the portal file chooser share order and
/// visibility, and update live" (two tables over one layout in one process; across processes
/// is the host's).
#[test]
fn two_tables_over_one_layout_both_hide_a_column_one_of_them_hid() {
    let mut rt = runtime(Host::new(2, Look::fallback(false)));
    assert!(element(&rt, &format!("t1-headings/{}", heading_key("size"))).is_some());
    rt.update(|s, _| s.panes[0].table.open_columns_menu(MenuAt::Headings(None)));
    settle(&mut rt);
    // The menu's items after its group label: Name, Mode, then Size.
    let [x, y, w, h] = rect(&rt, "t0-headings-menu-item3");
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    settle(&mut rt);
    assert_eq!(rt.state.layouts.len(), 1);
    assert_eq!(rt.state.layouts[0].why, LayoutWhy::Visibility);
    for t in ["t0", "t1"] {
        assert!(
            element(&rt, &format!("{t}-headings/{}", heading_key("size"))).is_none(),
            "{t} hides Size"
        );
    }
    assert!(rt.state.sorts.is_empty(), "never a sort");
}

/// list: "Clicking the Name, Size, Type, or Modified heading sorts by that field" from
/// assistive technology: a heading's press action sends the sort.
#[test]
fn assistive_technology_presses_a_heading_to_sort() {
    let mut rt = runtime(Host::new(1, Look::fallback(false)));
    assert!(rt.accessibility_action(&heading_key("size"), rune::accesskit::Action::Click, None));
    settle(&mut rt);
    assert_eq!(
        rt.state.sorts,
        [SortIntent {
            column: ColumnId::new("size"),
            dir: SortDir::Ascending
        }]
    );
}
