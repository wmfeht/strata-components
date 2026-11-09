//! The gallery page for C22 Folder peek: its scenes, demo state, view and key and poll hooks. Scene
//! names start with `c22-` and end in `-light` or `-dark`.
//!
//! Work package: `docs/work/phase-1-leaves/C22-folder-peek.md`.
//!
//! A Home folder in three views: List (C4's ListTable), Icons (a C1e grid stand-in until C3's
//! IconsGrid lands) and Columns (C2's ColumnsView, which never peeks). Resting the pointer on a
//! folder in List or Icons peeks it; the page stands in for Strata's lister, answering on the
//! virtual clock (Slow after 2 s, Locked with an error, Archive capped at 64 entries). The
//! header line `c22-heard` says what the host last heard.
//!
//! | Key | Does |
//! | --- | --- |
//! | Ctrl+1, Ctrl+2, Ctrl+3 | List, Icons, Columns |
//! | Ctrl+P | Folder peeking on or off |
//! | `i` | The keyboard peek at the cursor's folder (Strata's 10xer `i`) |
//! | Escape | Closes an open peek first |

use std::ops::Range;
use std::time::Duration;

use rune::{Cx, El, Key, KeyEvent, Tx, col, icon, row, text};
use strata_components::collection::element::{
    Collection, CollectionState, ContentSlot, Controllers, Intent, Lines, Roles, RowMarks,
};
use strata_components::collection::keyboard::Keyboard;
use strata_components::collection::marquee::Marquee;
use strata_components::collection::pointer::PointerIntent;
use strata_components::collection::selection::Listing;
use strata_components::columns::{
    ColumnSources, ColumnsParts, ColumnsPartsMut, ColumnsState, ColumnsView, Focus, Opening,
};
use strata_components::contract::{
    CollectionGeometry, Direction, EntryId, EntryKind, EntrySource, IndexRanges, ListEntry,
    ListSource, Point, Rect, ThumbKey,
};
use strata_components::folder_peek::{
    Change, FolderPeek, More, Origin, PeekContents, PeekState, PeekStatus,
};
use strata_components::list_table::{
    ColumnLayout, ColumnWidths, ListParts, ListTable, ListTableState,
};
use strata_components::scroll::Autoscroll;
use strata_components::thumbnails::{Outcome, Settings, Step, ThumbnailStore, ViewerId, Viewport};
use strata_kit::icons::IconName;
use strata_kit::look::Look;

use crate::{Gallery, Scene};

/// The List collection's key.
pub const LIST: &str = "c22-list";

/// The Icons stand-in's key.
pub const ICONS: &str = "c22-icons";

/// The Columns strip's key.
pub const COLUMNS: &str = "c22-columns";

/// The stand-in's tile size.
const TILE: (f32, f32) = (120.0, 96.0);

/// How long the fake lister takes, and Slow's.
const LIST_DELAY: f64 = 0.1;
const SLOW_DELAY: f64 = 2.0;

/// When a scene opens its peek.
const SCENE_OPEN: f64 = 0.25;

/// Strata's peek load cap (`PEEK_MAX_ENTRIES`): Archive stops there.
const CAP: usize = 64;

/// The views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    List,
    Icons,
    Columns,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::List => "List",
            Mode::Icons => "Icons",
            Mode::Columns => "Columns",
        }
    }
}

/// Home's folders and files, folders first.
const HOME: &[(&str, bool)] = &[
    ("Projects", true),
    ("Music", true),
    ("Photos", true),
    ("Empty", true),
    ("Locked", true),
    ("Slow", true),
    ("Archive", true),
    ("Downloads", true),
    ("notes.md", false),
    ("budget.ods", false),
    ("holiday.jpg", false),
    ("README", false),
];

fn home() -> Vec<ListEntry> {
    HOME.iter()
        .enumerate()
        .map(|(i, (name, folder))| {
            let (kind, icon) = if *folder {
                (EntryKind::Folder, "folder")
            } else {
                (EntryKind::File, "file")
            };
            ListEntry::new(EntryId::new(1 + i as u64), *name, kind, icon)
        })
        .collect()
}

/// What the fake lister answers for folder `name`: the rows, the status and the more count.
fn listing(name: &str) -> (Vec<ListEntry>, PeekStatus, Option<More>) {
    let entry = |i: usize, name: String, folder: bool| {
        let (kind, icon) = if folder {
            (EntryKind::Folder, "folder")
        } else {
            (EntryKind::File, "file_text")
        };
        ListEntry::new(EntryId::new(10_000 + i as u64), name, kind, icon)
    };
    let names = |list: &[(&str, bool)]| -> Vec<ListEntry> {
        list.iter()
            .enumerate()
            .map(|(i, (n, f))| entry(i, n.to_string(), *f))
            .collect()
    };
    match name {
        "Projects" => {
            let mut rows = names(&[("src", true), ("docs", true), ("assets", true)]);
            rows.extend((rows.len()..20).map(|i| entry(i, format!("file-{i:02}.txt"), false)));
            (rows, PeekStatus::Ready, None)
        }
        "Music" => (
            names(&[
                ("Albums", true),
                ("playlist.m3u", false),
                (
                    "A very long recording name from the summer festival, remastered.flac",
                    false,
                ),
            ]),
            PeekStatus::Ready,
            None,
        ),
        "Photos" => {
            let rows = (0..4)
                .map(|i| {
                    let mut e = entry(i, format!("IMG_{:04}.jpg", 101 + i), false);
                    e.icon = "file_image".into();
                    e.thumbnail = Some(ThumbKey::new(500 + i as u64));
                    e
                })
                .collect();
            (rows, PeekStatus::Ready, None)
        }
        "Locked" => (
            Vec::new(),
            PeekStatus::Error("Permission denied".into()),
            None,
        ),
        "Slow" => (
            names(&[("a.txt", false), ("b.txt", false)]),
            PeekStatus::Ready,
            None,
        ),
        "Archive" => {
            // Strata stops at 64; the folder holds more, so the count is a floor.
            let rows = (0..CAP)
                .map(|i| entry(i, format!("backup-{i:03}.tar"), false))
                .collect();
            (rows, PeekStatus::Ready, Some(More::AtLeast(CAP - 8)))
        }
        "Downloads" => (names(&[("setup.iso", false)]), PeekStatus::Ready, None),
        _ => (Vec::new(), PeekStatus::Ready, None),
    }
}

/// The host's listing of the peeked folder.
#[derive(Debug, Default)]
pub struct Peeked {
    pub name: String,
    pub status: PeekStatus,
    pub rows: ListSource,
    pub more: Option<More>,
    /// When the fake lister answers, on the page's clock.
    pub due: Option<f64>,
}

/// One column: Home.
#[derive(Debug, Default)]
pub struct OneColumn {
    pub source: ListSource,
}

impl ColumnSources for OneColumn {
    fn len(&self) -> usize {
        1
    }
    fn source(&self, _: usize) -> &dyn EntrySource {
        &self.source
    }
    fn version(&self, _: usize) -> u64 {
        1
    }
    fn title(&self, _: usize) -> &str {
        "Home"
    }
    fn open_child(&self, _: usize) -> Option<EntryId> {
        None
    }
}

/// C22's demo state, held in `Gallery::c22`.
pub struct Demo {
    pub mode: Mode,
    pub home: ListSource,
    pub list: CollectionState,
    pub icons: CollectionState,
    pub table: ListTableState,
    pub layout: ColumnLayout,
    pub widths: ColumnWidths,
    pub columns: ColumnsState,
    pub column: OneColumn,
    pub peek: PeekState,
    pub peeked: Peeked,
    pub store: ThumbnailStore,
    pub viewer: ViewerId,
    pub clock: f64,
    /// The pane's width, when narrower than the page.
    pub pane: Option<f32>,
    /// What the host last heard.
    pub heard: String,
    /// A peek a scene opens once its view is laid out.
    pub open_at: Option<(&'static str, Origin)>,
}

impl Default for Demo {
    fn default() -> Self {
        Self::new(&Look::fallback(false))
    }
}

fn install(c: &mut Controllers) {
    c.pointer = Some(Box::new(PointerIntent::new()));
    c.keyboard = Some(Box::<Keyboard>::default());
    c.marquee = Some(Box::new(Marquee::new()));
}

impl Demo {
    fn new(look: &Look) -> Self {
        let home = ListSource::new(home());
        let (mut list, mut icons) = (CollectionState::new(), CollectionState::new());
        install(&mut list.controllers);
        install(&mut icons.controllers);
        let column = OneColumn {
            source: ListSource::new(self::home()),
        };
        let mut columns = ColumnsState::new(COLUMNS, Autoscroll::new());
        columns.push(
            Opening::new(&column.source, Listing::Complete),
            Focus::Column,
            look,
            &mut Tx::default(),
        );
        let mut store = ThumbnailStore::new(Settings::default());
        let viewer = store.viewer();
        Self {
            mode: Mode::List,
            home,
            list,
            icons,
            table: ListTableState::new(),
            layout: ColumnLayout::new(super::c4::columns().iter().map(|c| c.id().clone())),
            widths: ColumnWidths::new(),
            columns,
            column,
            peek: PeekState::new(),
            peeked: Peeked::default(),
            store,
            viewer,
            clock: 0.0,
            pane: None,
            heard: String::new(),
            open_at: None,
        }
    }

    fn name_of(&self, id: EntryId) -> String {
        self.home
            .index_of(id)
            .map(|i| self.home.entry(i).name.to_owned())
            .unwrap_or_default()
    }

    fn id_of(&self, name: &str) -> Option<EntryId> {
        (0..self.home.len())
            .find(|&i| self.home.entry(i).name == name)
            .map(|i| self.home.id_at(i))
    }

    /// The host's side of a peek opening or closing: start or drop the listing.
    fn relist(&mut self, change: Change, tx: &mut Tx) {
        match change {
            Change::None => {}
            Change::Opened => {
                let Some(target) = self.peek.target() else {
                    return;
                };
                let name = self.name_of(target.folder);
                let delay = if name == "Slow" {
                    SLOW_DELAY
                } else {
                    LIST_DELAY
                };
                self.heard = format!("Peek {name}");
                self.peeked = Peeked {
                    name,
                    due: Some(self.clock + delay),
                    ..Peeked::default()
                };
                tx.wake_after(Duration::from_secs_f64(delay));
            }
            Change::Closed => {
                self.heard = "Peek closed".into();
                self.peeked = Peeked::default();
            }
        }
    }

    fn heard(&mut self, intent: Intent) {
        if let Intent::BeginDrag { ids, .. } = intent {
            self.heard = format!("Dragging {} items", ids.len());
            // The host's part of "starting a drag closes the peek".
            if self.peek.close() {
                self.peeked = Peeked::default();
            }
        }
    }

    fn coll(&self) -> &CollectionState {
        match self.mode {
            Mode::Icons => &self.icons,
            _ => &self.list,
        }
    }
}

/// The page's peek.
pub fn peek() -> FolderPeek<Gallery> {
    FolderPeek::new(
        |g: &Gallery| &g.c22.peek,
        |g: &mut Gallery| &mut g.c22.peek,
        |g: &Gallery| {
            let p = &g.c22.peeked;
            PeekContents::new(&p.name, &p.status, &p.rows).more(p.more)
        },
        |g: &Gallery| g.look(),
    )
    .thumbnails(|g: &Gallery| &g.c22.store)
    .on_change(|g: &mut Gallery, change, tx| g.c22.relist(change, tx))
    .on_open(|g: &mut Gallery, target, _| {
        g.c22.heard = format!("Open {}", g.c22.name_of(target.folder));
    })
}

fn marks(scope: &'static str) -> impl Fn(&Gallery, EntryId) -> RowMarks {
    move |g: &Gallery, id| {
        let mut m = RowMarks::default();
        m.peek_anchor = g.c22.peek.is_anchor(scope, id);
        m
    }
}

/// The List collection, before the table adds its parts.
fn list_base() -> Collection<Gallery> {
    Collection::new(
        LIST,
        |g: &Gallery| (&g.c22.home, &g.c22.list),
        |g: &mut Gallery| (&g.c22.home, &mut g.c22.list),
        |g: &Gallery| g.look(),
        |_: &Gallery| 1,
    )
    .name("Home")
    .clock(|g: &Gallery| g.c22.clock)
    .marks(marks(LIST))
    .on_intent(|g: &mut Gallery, intent, _| g.c22.heard(intent))
}

fn table() -> ListTable<Gallery> {
    ListTable::new(
        LIST,
        super::c4::columns(),
        |g: &Gallery| ListParts {
            source: &g.c22.home,
            collection: &g.c22.list,
            table: &g.c22.table,
            layout: &g.c22.layout,
            widths: &g.c22.widths,
            sort: None,
            look: g.look(),
        },
        |g: &mut Gallery| &mut g.c22.table,
    )
}

/// The List collection as the view builds it: the table's parts and the peek's rows.
fn list_collection(g: &Gallery) -> Collection<Gallery> {
    table().collection(g, peek().rows(list_base()))
}

/// Tiles `TILE` wide, as many to a line as fit: the Icons stand-in's packing.
struct Tiles;

impl Lines for Tiles {
    fn per_line(&self, width: f32) -> usize {
        ((width / TILE.0) as usize).max(1)
    }

    fn geometry(&self, len: usize, h: f32, w: f32, vh: f32) -> Box<dyn CollectionGeometry> {
        Box::new(TileGrid {
            len,
            per: self.per_line(w),
            w,
            h,
            vh,
        })
    }
}

/// `per` equal cells a line, as C1e draws a grid's lines.
struct TileGrid {
    len: usize,
    per: usize,
    w: f32,
    h: f32,
    vh: f32,
}

impl TileGrid {
    fn cell_w(&self) -> f32 {
        self.w / self.per as f32
    }
}

impl CollectionGeometry for TileGrid {
    fn rect_of(&self, i: usize) -> Rect {
        let cw = self.cell_w();
        Rect::new(
            (i % self.per) as f32 * cw,
            (i / self.per) as f32 * self.h,
            cw,
            self.h,
        )
    }
    fn index_at(&self, p: Point) -> Option<usize> {
        if p.x < 0.0 || p.y < 0.0 || p.x >= self.w {
            return None;
        }
        let i = (p.y / self.h) as usize * self.per + (p.x / self.cell_w()) as usize;
        (i < self.len).then_some(i)
    }
    fn range_in(&self, r: Rect) -> IndexRanges {
        let mut out = IndexRanges::new();
        if self.len == 0 {
            return out;
        }
        let cw = self.cell_w();
        let lines = (r.y.max(0.0) / self.h) as usize..((r.y + r.h) / self.h).ceil() as usize;
        let cols = (r.x.max(0.0) / cw) as usize..(((r.x + r.w) / cw).ceil() as usize).min(self.per);
        for line in lines {
            let a = (line * self.per + cols.start).min(self.len);
            let b = (line * self.per + cols.end).min(self.len);
            if a < b {
                out.push(a..b);
            }
        }
        out
    }
    fn neighbor(&self, i: usize, d: Direction) -> Option<usize> {
        let n = match d {
            Direction::Left if !i.is_multiple_of(self.per) => i - 1,
            Direction::Right if i % self.per + 1 < self.per => i + 1,
            Direction::Up if i >= self.per => i - self.per,
            Direction::Down => i + self.per,
            _ => return None,
        };
        (n < self.len).then_some(n)
    }
    fn page_step(&self) -> usize {
        ((self.vh / self.h) as usize).max(1) * self.per
    }
}

fn icons_collection() -> Collection<Gallery> {
    let base = Collection::new(
        ICONS,
        |g: &Gallery| (&g.c22.home, &g.c22.icons),
        |g: &mut Gallery| (&g.c22.home, &mut g.c22.icons),
        |g: &Gallery| g.look(),
        |_: &Gallery| 1,
    )
    .name("Home")
    .clock(|g: &Gallery| g.c22.clock)
    .roles(Roles::GRID_CELLS)
    .row_height(|_: &Gallery, _: &Look| TILE.1)
    .lines(Tiles)
    .marks(marks(ICONS))
    .on_intent(|g: &mut Gallery, intent, _| g.c22.heard(intent))
    .content(ContentSlot::pure(|_: &Gallery, rcx, _icon, label| {
        let look = rcx.look;
        let glyph = look.icon(if rcx.entry.kind.is_folder_like() {
            IconName::Folder
        } else {
            IconName::File
        });
        col()
            .w_full()
            .h_full()
            .gap(look.space.xs)
            .center()
            .child(
                icon(glyph, look.icon_size.lg * 1.5)
                    .color(look.kit.c.icon_base)
                    .a11y_hidden(),
            )
            .child(label)
    }));
    peek().rows(base)
}

fn columns_view() -> ColumnsView<Gallery> {
    ColumnsView::new(
        |g: &Gallery| ColumnsParts {
            sources: &g.c22.column,
            state: &g.c22.columns,
            look: &g.look,
        },
        |g: &mut Gallery| ColumnsPartsMut {
            sources: &g.c22.column,
            state: &mut g.c22.columns,
            look: &g.look,
        },
    )
}

// Scenes.

fn start(g: &mut Gallery, dark: bool) -> &mut Demo {
    g.open("c22", dark);
    let look = g.look.clone();
    g.c22 = Demo::new(&look);
    g.c22.peek.set_enabled(true);
    &mut g.c22
}

fn list(g: &mut Gallery, dark: bool) {
    start(g, dark);
}

fn opened(g: &mut Gallery, dark: bool, mode: Mode, folder: &'static str) {
    let d = start(g, dark);
    d.mode = mode;
    d.open_at = Some((folder, Origin::Hover));
}

fn off(g: &mut Gallery, dark: bool) {
    g.open("c22", dark);
    let look = g.look.clone();
    g.c22 = Demo::new(&look);
}

fn narrow(g: &mut Gallery, dark: bool) {
    start(g, dark).pane = Some(240.0);
}

fn columns(g: &mut Gallery, dark: bool) {
    start(g, dark).mode = Mode::Columns;
}

fn icons(g: &mut Gallery, dark: bool) {
    start(g, dark).mode = Mode::Icons;
}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("c22", dark)`, then fills in `g.c22`.
pub const SCENES: &[Scene] = &[
    ("c22-list-light", |g| list(g, false)),
    ("c22-list-dark", |g| list(g, true)),
    ("c22-list-open-light", |g| {
        opened(g, false, Mode::List, "Projects")
    }),
    ("c22-list-open-dark", |g| {
        opened(g, true, Mode::List, "Projects")
    }),
    ("c22-icons-right-light", |g| {
        opened(g, false, Mode::Icons, "Music")
    }),
    ("c22-icons-right-dark", |g| {
        opened(g, true, Mode::Icons, "Music")
    }),
    ("c22-icons-left-light", |g| {
        opened(g, false, Mode::Icons, "Archive")
    }),
    ("c22-icons-left-dark", |g| {
        opened(g, true, Mode::Icons, "Archive")
    }),
    ("c22-thumbnails-light", |g| {
        opened(g, false, Mode::Icons, "Photos")
    }),
    ("c22-thumbnails-dark", |g| {
        opened(g, true, Mode::Icons, "Photos")
    }),
    ("c22-loading-light", |g| {
        opened(g, false, Mode::List, "Slow")
    }),
    ("c22-loading-dark", |g| opened(g, true, Mode::List, "Slow")),
    ("c22-empty-light", |g| opened(g, false, Mode::List, "Empty")),
    ("c22-empty-dark", |g| opened(g, true, Mode::List, "Empty")),
    ("c22-error-light", |g| {
        opened(g, false, Mode::List, "Locked")
    }),
    ("c22-error-dark", |g| opened(g, true, Mode::List, "Locked")),
];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[
    ("c22-off-light", |g| off(g, false)),
    ("c22-off-dark", |g| off(g, true)),
    ("c22-icons-light", |g| icons(g, false)),
    ("c22-icons-dark", |g| icons(g, true)),
    ("c22-narrow-light", |g| narrow(g, false)),
    ("c22-narrow-dark", |g| narrow(g, true)),
    ("c22-columns-light", |g| columns(g, false)),
    ("c22-columns-dark", |g| columns(g, true)),
];

fn label(key: &str, t: String, g: &Gallery) -> El {
    let kit = g.kit();
    text(t).key(key).font(kit.ty.ui).color(kit.c.text_muted)
}

pub fn view(g: &Gallery, cx: &mut Cx<Gallery>) -> El {
    let d = &g.c22;
    let look = g.look();
    cx.reduced_motion(!look.animations);
    let kit = g.kit();
    let mut layer = Vec::new();
    let header = row()
        .w_full()
        .px(look.space.lg)
        .py(look.space.xs)
        .gap(look.space.lg)
        .center_y()
        .child(
            text(format!("Home · {}", d.mode.label()))
                .key("c22-title")
                .font(kit.ty.ui_medium)
                .color(kit.c.text_base),
        )
        .child(label(
            "c22-peeking",
            format!(
                "Folder peeking {}",
                if d.peek.enabled() { "on" } else { "off" }
            ),
            g,
        ))
        .child(label("c22-heard", d.heard.clone(), g));
    let body = match d.mode {
        Mode::List => table().build(g, peek().rows(list_base()), &mut layer),
        Mode::Icons => icons_collection().build(g),
        Mode::Columns => columns_view().build(g),
    };
    if d.mode != Mode::Columns {
        peek().card(g, &mut layer);
    }
    let pane = match d.pane {
        Some(w) => col().w(w).h_full().min_h(0.0).child(body),
        None => col().w_full().h_full().min_h(0.0).child(body),
    };
    let page = col().key("c22-page").w_full().h_full().child(header).child(
        col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .py(look.content.pad)
            .child(pane),
    );
    rune_kit::root(page, layer)
}

/// The window rect of folder `id`'s row in the current view, and its viewport.
fn rects(g: &Gallery, id: EntryId) -> Option<(&'static str, Rect, Rect)> {
    let d = &g.c22;
    let index = d.home.index_of(id)?;
    let (scope, c) = match d.mode {
        Mode::List => (LIST, list_collection(g)),
        Mode::Icons => (ICONS, icons_collection()),
        Mode::Columns => return None,
    };
    let viewport = d.coll().viewport();
    (viewport.w > 0.0).then(|| (scope, c.window_rect_of(g, index), viewport))
}

pub fn on_key(g: &mut Gallery, key: &KeyEvent, tx: &mut Tx) -> bool {
    let mode = |k: &KeyEvent| {
        [('1', Mode::List), ('2', Mode::Icons), ('3', Mode::Columns)]
            .into_iter()
            .find(|(c, _)| k.is_command(*c))
            .map(|(_, m)| m)
    };
    if let Some(m) = mode(key) {
        g.c22.mode = m;
        return true;
    }
    if key.is_command('p') {
        let d = &mut g.c22;
        let on = !d.peek.enabled();
        if d.peek.set_enabled(on) {
            d.relist(Change::Closed, tx);
        }
        return true;
    }
    match &key.key {
        Key::Escape => {
            let d = &mut g.c22;
            if d.peek.close() {
                d.relist(Change::Closed, tx);
                return true;
            }
            false
        }
        Key::Text(t) if t == "i" && key.mods == rune::Mods::default() => {
            let d = &g.c22;
            let Some(id) = d.coll().selection.cursor() else {
                return true;
            };
            let folder = d
                .home
                .index_of(id)
                .is_some_and(|i| d.home.entry(i).kind.is_folder_like());
            if let (true, Some((scope, item, viewport))) = (folder, rects(g, id)) {
                let change = g.c22.peek.toggle(scope, id, item, viewport);
                g.c22.relist(change, tx);
            }
            true
        }
        _ => false,
    }
}

/// A fake thumbnail: a gradient, its hue from the key.
fn picture(px: u32, seed: u64) -> Vec<u8> {
    let hue = (seed % 7) as f32 / 7.0 * std::f32::consts::TAU;
    let base = [hue.cos(), (hue + 2.1).cos(), (hue + 4.2).cos()];
    let mut out = Vec::with_capacity((px * px * 4) as usize);
    for y in 0..px {
        let light = 0.45 + 0.45 * (1.0 - y as f32 / px as f32);
        for _ in 0..px {
            for c in base {
                out.push(((0.5 + 0.45 * c) * light * 255.0) as u8);
            }
            out.push(255);
        }
    }
    out
}

pub fn poll(g: &mut Gallery, t: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    g.c22.clock = t;
    // After the table's first layouts settle (its first width is the columns' alone): the
    // placement is worked out once, as the peek opens.
    if let Some((name, origin)) = g.c22.open_at
        && t >= SCENE_OPEN
        && let Some(id) = g.c22.id_of(name)
        && let Some((scope, item, viewport)) = rects(g, id)
    {
        let d = &mut g.c22;
        d.open_at = None;
        let change = d.peek.open(scope, id, origin, item, viewport);
        d.relist(change, tx);
        changed = true;
    }
    let look = g.look.clone();
    let d = &mut g.c22;
    if d.peeked.due.is_some_and(|due| t >= due) {
        let (rows, status, more) = listing(&d.peeked.name);
        d.peeked.due = None;
        d.peeked.rows = ListSource::new(rows);
        d.peeked.status = status;
        d.peeked.more = more;
        let step = Step::for_tile(&look, look.icon_size.md, 2.0);
        let shown: Range<usize> = 0..d.peeked.rows.len().min(8);
        d.store
            .sync(d.viewer, &d.peeked.rows, Viewport::new(shown, step));
        changed = true;
    }
    while let Some(r) = d.store.pop(t) {
        let px = r.step.px();
        let rgba = picture(px, r.key.get());
        d.store.complete(
            r.ticket,
            Outcome::Ready {
                width: px,
                height: px,
                rgba,
            },
            t,
        );
        changed = true;
    }
    changed
        | match g.c22.mode {
            Mode::List => list_collection(g).tick(g, tx),
            Mode::Icons => icons_collection().tick(g, tx),
            Mode::Columns => columns_view().tick(g, t, tx),
        }
}
