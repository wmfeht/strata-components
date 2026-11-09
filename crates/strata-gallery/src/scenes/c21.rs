//! The gallery page for C21 Context menus: its scenes, demo state, view and key and poll
//! hooks. Scene names start with `c21-` and end in `-light` or `-dark`.
//!
//! Work package: `docs/work/phase-1-leaves/C21-context-menus.md`.
//!
//! The page is a stand-in pane over a fake Documents folder (40 entries), with a search field
//! and a rename field over it. The page plays Strata's part: it builds each menu's model from
//! the target (an item, a multi-selection, the background), as Strata's policy would, and
//! writes what it heard to the status line (`c21-status`).
//!
//! - A right-click on an entry or on empty space opens its menu at the press (C1b's intent).
//! - Menu or Shift+F10 in the list opens it at the cursor's entry (C21's `MenuKeys` around
//!   C1c's keyboard), or the background menu with the cursor unselected.
//! - The search field has C21's text menu through its own `on_secondary_press`.
//! - The rename field stands in for C5's editor until C5 lands: its secondary press sends the
//!   shape C5's `RenameIntent::FieldMenu { id, at }` has, and the page answers it as a host
//!   answers C5 (`menu_opened`, then `menu_closed` from the menu's `on_close`).

use rune::{Cx, El, KeyEvent, PointerPress, TextEdit, Tx, col, row, text, text_input};
use strata_components::collection::element::{Collection, CollectionState, Controllers, Intent};
use strata_components::context_menu::{
    AnchorEdge, ContextMenu, ContextMenuState, FieldMenu, FieldMenuState, Item, MenuIntent,
    MenuKeys, MenuModel, MenuTarget, Section, keyboard_anchor,
};
use strata_components::contract::{EntryId, EntryKind, EntrySource, ListEntry, ListSource, Point};
use strata_kit::icons::IconName;

use crate::{Gallery, Scene};

/// The list's key: its rows are `row-{id}`.
pub const SCOPE: &str = "c21-list";
/// The context menu's key: its surface is `c21-menu-menu`, its rows `c21-menu-item{n}`.
pub const MENU: &str = "c21-menu";
/// The field menu's key: `c21-field-menu-menu`, rows `c21-field-menu-item{n}`.
pub const FIELD_MENU: &str = "c21-field-menu";

/// The page's text fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Search,
    /// The stand-in for C5's rename editor.
    Rename,
}

/// What C5's editor sends for its field menu (`RenameIntent::FieldMenu { id, at }`), as the
/// stand-in field sends it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StandIn {
    FieldMenu { id: EntryId, at: Point },
}

/// C21's demo state, held in `Gallery::c21`.
pub struct Demo {
    pub listing: ListSource,
    pub coll: CollectionState,
    pub clock: f64,
    pub menu: ContextMenuState,
    pub fields: FieldMenuState<Field>,
    pub search: TextEdit,
    pub rename: TextEdit,
    /// The stand-in's `menu_opened` flag: the field losing the keyboard to its menu ends
    /// nothing while it is set.
    pub rename_menu: bool,
    /// What the page heard last.
    pub status: String,
    /// A keyboard open the scene asks for, run once the list is laid out.
    pub open_by_keys: bool,
    /// Menus opened since the scene began.
    pub opens: u32,
}

impl Default for Demo {
    fn default() -> Self {
        let mut coll = CollectionState::new();
        install(&mut coll.controllers);
        Self {
            listing: ListSource::new(entries()),
            coll,
            clock: 0.0,
            menu: ContextMenuState::new(),
            fields: FieldMenuState::new(),
            search: TextEdit::new(""),
            rename: TextEdit::new("Notes.md"),
            rename_menu: false,
            status: String::new(),
            open_by_keys: false,
            opens: 0,
        }
    }
}

fn install(controllers: &mut Controllers) {
    controllers.pointer = Some(Box::new(
        strata_components::collection::pointer::PointerIntent::new(),
    ));
    controllers.keyboard = Some(Box::new(MenuKeys::new(
        strata_components::collection::keyboard::Keyboard::default(),
        AnchorEdge::Center,
    )));
}

fn entries() -> Vec<ListEntry> {
    let folders = ["Projects", "Receipts", "Music", "Holidays"];
    let mut out: Vec<ListEntry> = folders
        .iter()
        .map(|n| (n.to_string(), EntryKind::Folder, "folder"))
        .chain((1..=36).map(|i| {
            let (name, icon) = match i {
                1 => ("Notes.md".to_owned(), "file_text"),
                2 => ("budget.ods".to_owned(), "file"),
                3 => ("main.rs".to_owned(), "file_code"),
                _ => (format!("scan-{i:02}.pdf"), "file"),
            };
            (name, EntryKind::File, icon)
        }))
        .enumerate()
        .map(|(i, (n, k, icon))| ListEntry::new(EntryId::new(1 + i as u64), n, k, icon))
        .collect();
    out.truncate(40);
    out
}

impl Demo {
    fn name_of(&self, id: EntryId) -> String {
        self.listing
            .index_of(id)
            .map(|i| self.listing.entry(i).name.to_owned())
            .unwrap_or_default()
    }

    /// Strata's part: the menu for `target`, as its policy would build it.
    fn model_for(&self, target: &MenuTarget) -> MenuModel {
        let selected = self.coll.selection.selected_ids(&self.listing);
        match target {
            MenuTarget::Entry(_) if selected.len() > 1 => {
                let names: Vec<String> =
                    selected.iter().take(3).map(|&i| self.name_of(i)).collect();
                let more = if selected.len() > 3 { ", …" } else { "" };
                MenuModel::new()
                    .header_line(format!("{} items selected", selected.len()))
                    .header_line(format!("{}{more}", names.join(", ")))
                    .section(creation())
                    .section(Section::new([open_with()]))
                    .section(Section::new([
                        Item::new("cut", "Cut")
                            .icon(IconName::Cut)
                            .shortcut("Ctrl+X"),
                        Item::new("copy", "Copy")
                            .icon(IconName::Copy)
                            .shortcut("Ctrl+C"),
                    ]))
                    .section(removal())
            }
            MenuTarget::Entry(id) => {
                let folder = self
                    .listing
                    .index_of(*id)
                    .is_some_and(|i| self.listing.entry(i).kind == EntryKind::Folder);
                let mut opening = vec![
                    Item::new("open", "Open")
                        .icon(IconName::Open)
                        .shortcut("Enter"),
                    open_with(),
                ];
                if folder {
                    opening
                        .push(Item::new("terminal", "Open in Terminal").icon(IconName::Terminal));
                }
                MenuModel::new()
                    .header_line(self.name_of(*id))
                    .header_line("~/Documents")
                    .section(creation())
                    .section(Section::new(opening))
                    .section(Section::new([Item::submenu(
                        "send",
                        "Send to",
                        [Section::new([
                            Item::new("send-desktop", "Desktop").icon(IconName::Desktop),
                            Item::new("send-mail", "Mail recipient"),
                            Item::new("send-archive", "Compressed archive").icon(IconName::Archive),
                        ])],
                    )]))
                    .section(Section::new([
                        Item::new("cut", "Cut")
                            .icon(IconName::Cut)
                            .shortcut("Ctrl+X"),
                        Item::new("copy", "Copy")
                            .icon(IconName::Copy)
                            .shortcut("Ctrl+C"),
                        Item::new("rename", "Rename")
                            .icon(IconName::Rename)
                            .shortcut("F2 / Ctrl+R"),
                    ]))
                    .section(Section::new([Item::new("properties", "Properties")
                        .icon(IconName::Properties)
                        .shortcut("Alt+Enter")]))
                    .section(removal())
            }
            _ => MenuModel::new()
                .section(Section::new([
                    Item::new("new-folder", "New Folder")
                        .icon(IconName::FolderPlus)
                        .shortcut("Ctrl+Shift+N"),
                    Item::new("new-file", "New File").icon(IconName::Plus),
                    Item::new("paste", "Paste")
                        .icon(IconName::Paste)
                        .enabled(false),
                ]))
                .section(Section::new([
                    Item::new("terminal", "Open in Terminal").icon(IconName::Terminal)
                ]))
                .section(Section::new([
                    Item::new("select-all", "Select All").shortcut("Ctrl+A"),
                    Item::new("refresh", "Refresh")
                        .icon(IconName::Refresh)
                        .shortcut("F5"),
                    Item::new("hidden", "Show Hidden Files")
                        .shortcut("Ctrl+H")
                        .checked(false),
                ]))
                .section(Section::new([
                    Item::new("properties", "Properties").icon(IconName::Properties)
                ])),
        }
    }

    /// The list asked for a menu (C1b's press, or C21's keys).
    fn heard(&mut self, intent: Intent) {
        if let Intent::ContextMenu { target, anchor } = intent {
            let target = target.map_or(MenuTarget::Background, MenuTarget::Entry);
            self.opens += 1;
            self.status = format!("Menu {} on {}", self.opens, self.target_name(&target));
            let model = self.model_for(&target);
            self.menu
                .open(target, Point::new(anchor.x, anchor.y), model);
        }
    }

    fn target_name(&self, target: &MenuTarget) -> String {
        match target {
            MenuTarget::Entry(id) => self.name_of(*id),
            _ => "the background".to_owned(),
        }
    }

    fn chose(&mut self, intent: MenuIntent) {
        let MenuIntent::Activate { target, action } = intent;
        self.status = format!("Chose {action} on {}", self.target_name(&target));
    }

    fn dismissed(&mut self, target: MenuTarget) {
        self.status = format!(
            "Dismissed menu {} on {}",
            self.opens,
            self.target_name(&target)
        );
    }

    /// The stand-in field asked for its menu: what a host does for C5's `FieldMenu` intent,
    /// besides opening the menu (`RenameState::menu_opened`).
    fn stand_in(&mut self, intent: StandIn) {
        let StandIn::FieldMenu { id, .. } = intent;
        self.rename_menu = true;
        self.status = format!("Field menu on the rename of {}", self.name_of(id));
    }
}

fn creation() -> Section {
    Section::new([
        Item::new("new-folder", "New Folder")
            .icon(IconName::FolderPlus)
            .shortcut("Ctrl+Shift+N"),
        Item::new("new-file", "New File").icon(IconName::Plus),
    ])
}

fn open_with() -> Item {
    Item::new("open-with", "Open With…")
        .enabled(false)
        .description("Looking for compatible applications…")
}

fn removal() -> Section {
    Section::new([
        Item::new("trash", "Move to Trash")
            .icon(IconName::Trash)
            .shortcut("Delete"),
        Item::new("delete", "Permanently delete")
            .icon(IconName::Close)
            .shortcut("Shift+Del")
            .danger(true),
    ])
}

// --- Scenes ---

fn scene(g: &mut Gallery, dark: bool) -> &mut Demo {
    g.open("c21", dark);
    g.c21 = Demo::default();
    &mut g.c21
}

/// Selects entries `from..=to` (indices), the cursor on `to`.
fn select(d: &mut Demo, from: usize, to: usize) {
    use strata_components::collection::selection::Extend;
    d.coll.selection.select_only(&d.listing, from);
    if to != from {
        d.coll.selection.extend_to(&d.listing, to, Extend::Replace);
    }
    d.coll.note_selection(&d.listing);
}

fn open_at(d: &mut Demo, target: MenuTarget, x: f32, y: f32) {
    let model = d.model_for(&target);
    d.menu.open(target, Point::new(x, y), model);
}

/// Notes.md's item menu, as a right-click on it opens it.
fn item(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    select(d, 4, 4);
    open_at(d, MenuTarget::Entry(EntryId::new(5)), 320.0, 200.0);
}

/// Three files' menu.
fn multi(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    select(d, 4, 6);
    open_at(d, MenuTarget::Entry(EntryId::new(7)), 320.0, 230.0);
}

/// The background menu, nothing selected.
fn background(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    open_at(d, MenuTarget::Background, 520.0, 260.0);
}

/// Notes.md's menu with Send to open.
fn submenu(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    select(d, 4, 4);
    open_at(d, MenuTarget::Entry(EntryId::new(5)), 260.0, 120.0);
    // Send to is the fifth item: New Folder, New File, Open, Open With…, Send to.
    d.menu.open_submenu(&[4]);
}

/// Opened at the window's bottom-right corner: it flips above the press and slides left.
fn edge(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    open_at(d, MenuTarget::Background, 950.0, 630.0);
}

/// Opened mid-window, with room neither above nor below: the menu shrinks and scrolls.
fn scrolling(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    select(d, 4, 4);
    open_at(d, MenuTarget::Entry(EntryId::new(5)), 320.0, 330.0);
}

/// Menu at the cursor: Notes.md selected, opened as the Menu key opens it.
fn keyboard_anchor_scene(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    select(d, 4, 4);
    d.open_by_keys = true;
}

/// The search field's text menu with a word selected.
fn field(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    d.search = TextEdit::new("report draft");
    d.search.set_selection(0, 6);
    d.status = "Field menu on search".to_owned();
    d.fields.open(Field::Search, Point::new(140.0, 44.0));
}

/// The search field's text menu with nothing selected and an empty clipboard.
fn field_empty(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    d.search = TextEdit::new("notes");
    d.search.set_caret(5);
    d.fields.open(Field::Search, Point::new(140.0, 44.0));
    d.fields.clipboard_known(false);
}

pub const SCENES: &[Scene] = &[
    ("c21-item-light", |g| item(g, false)),
    ("c21-item-dark", |g| item(g, true)),
    ("c21-multi-light", |g| multi(g, false)),
    ("c21-multi-dark", |g| multi(g, true)),
    ("c21-background-light", |g| background(g, false)),
    ("c21-background-dark", |g| background(g, true)),
    ("c21-submenu-light", |g| submenu(g, false)),
    ("c21-submenu-dark", |g| submenu(g, true)),
    ("c21-edge-light", |g| edge(g, false)),
    ("c21-edge-dark", |g| edge(g, true)),
    ("c21-scrolling-light", |g| scrolling(g, false)),
    ("c21-scrolling-dark", |g| scrolling(g, true)),
    ("c21-keyboard-anchor-light", |g| {
        keyboard_anchor_scene(g, false)
    }),
    ("c21-keyboard-anchor-dark", |g| {
        keyboard_anchor_scene(g, true)
    }),
    ("c21-field-light", |g| field(g, false)),
    ("c21-field-dark", |g| field(g, true)),
    ("c21-field-empty-light", |g| field_empty(g, false)),
    ("c21-field-empty-dark", |g| field_empty(g, true)),
];

/// The page with no menu open, Notes.md selected: where the scripts start.
fn start(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    select(d, 4, 4);
}

/// Nothing selected, in a short folder: blank space under its eight rows.
fn nothing(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    d.listing = ListSource::new(entries().into_iter().take(8).collect());
}

/// Three files selected.
fn three(g: &mut Gallery, dark: bool) {
    let d = scene(g, dark);
    select(d, 4, 6);
}

pub const REVIEW: &[Scene] = &[
    ("c21-start-light", |g| start(g, false)),
    ("c21-start-dark", |g| start(g, true)),
    ("c21-nothing-light", |g| nothing(g, false)),
    ("c21-nothing-dark", |g| nothing(g, true)),
    ("c21-three-light", |g| three(g, false)),
    ("c21-three-dark", |g| three(g, true)),
];

// --- The view ---

fn collection() -> Collection<Gallery> {
    Collection::new(
        SCOPE,
        |g: &Gallery| (&g.c21.listing, &g.c21.coll),
        |g: &mut Gallery| (&g.c21.listing, &mut g.c21.coll),
        |g: &Gallery| g.look(),
        |_: &Gallery| 1,
    )
    .name("Documents")
    .clock(|g: &Gallery| g.c21.clock)
    .on_intent(|g: &mut Gallery, intent, _| g.c21.heard(intent))
}

fn context_menu() -> ContextMenu<Gallery> {
    ContextMenu::new(
        MENU,
        |g: &Gallery| &g.c21.menu,
        |g: &mut Gallery| &mut g.c21.menu,
    )
    .on_intent(|g: &mut Gallery, intent, _| g.c21.chose(intent))
    .on_dismiss(|g: &mut Gallery, target| g.c21.dismissed(target))
}

fn edit_mut(g: &mut Gallery, f: Field) -> Option<&mut TextEdit> {
    Some(match f {
        Field::Search => &mut g.c21.search,
        Field::Rename => &mut g.c21.rename,
    })
}

fn edit(g: &Gallery, f: Field) -> Option<&TextEdit> {
    Some(match f {
        Field::Search => &g.c21.search,
        Field::Rename => &g.c21.rename,
    })
}

fn field_menu() -> FieldMenu<Gallery, Field> {
    FieldMenu::new(
        FIELD_MENU,
        |g: &Gallery| &g.c21.fields,
        |g: &mut Gallery| &mut g.c21.fields,
        edit_mut,
        edit,
    )
    .on_change(|g: &mut Gallery, f, _| {
        let text = edit(g, f).map(|e| e.text.clone()).unwrap_or_default();
        g.c21.status = format!("{f:?} now reads \"{text}\"");
    })
    .on_close(|g: &mut Gallery, f| {
        // C5's `menu_closed`: the stand-in's menu flag drops.
        if f == Field::Rename {
            g.c21.rename_menu = false;
            g.c21.status = "The rename's field menu closed".to_owned();
        }
    })
}

pub fn view(g: &Gallery, cx: &mut Cx<Gallery>) -> El {
    let d = &g.c21;
    let look = g.look();
    cx.reduced_motion(!look.animations);
    let kit = g.kit();
    let ty = kit.ty.ui_compact;
    let field_box = |e: El| {
        row()
            .w(200.0)
            .h(26.0)
            .px(look.space.xs)
            .center_y()
            .radius(kit.radius.sm)
            .fill(kit.c.bg_base)
            .border(1.0, kit.c.border_base)
            .child(e.grow(1.0))
    };
    let search = text_input(&d.search, ty)
        .single_line()
        .key("c21-search")
        .label("Search")
        .color(kit.c.text_base)
        .on_edit(|g: &mut Gallery| &mut g.c21.search)
        .on_secondary_press(field_menu().hook(Field::Search));
    let rename = text_input(&d.rename, ty)
        .single_line()
        .key("c21-rename")
        .label("Rename (C5 stand-in)")
        .color(kit.c.text_base)
        .on_edit(|g: &mut Gallery| &mut g.c21.rename)
        .on_secondary_press(|g: &mut Gallery, p: PointerPress, tx: &mut Tx| {
            // The editor's own press sends its intent; the host answers it.
            let id = EntryId::new(5);
            let intent = StandIn::FieldMenu {
                id,
                at: Point::new(p.x, p.y),
            };
            g.c21.stand_in(intent);
            let StandIn::FieldMenu { at, .. } = intent;
            field_menu().open(g, Field::Rename, at, tx);
        });
    let header = row()
        .w_full()
        .h(look.frame.bar)
        .px(look.space.lg)
        .gap(look.space.lg)
        .center_y()
        .child(
            text("Documents")
                .key("c21-title")
                .font(kit.ty.ui_medium)
                .color(kit.c.text_base),
        )
        .child(field_box(search))
        .child(field_box(rename))
        .child(
            text(d.status.clone())
                .key("c21-status")
                .font(kit.ty.ui)
                .color(kit.c.text_muted),
        );
    let mut layer = Vec::new();
    let menu = context_menu().build(g, look, &mut layer);
    let fmenu = field_menu().build(g, look, &mut layer);
    let page = col()
        .key("c21-page")
        .w_full()
        .h_full()
        .child(header)
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .py(look.content.pad)
                .child(collection().build(g)),
        )
        .child(menu)
        .child(fmenu);
    rune_kit::root(page, layer)
}

pub fn on_key(_g: &mut Gallery, _key: &KeyEvent, _tx: &mut Tx) -> bool {
    false
}

pub fn poll(g: &mut Gallery, t: f64, tx: &mut Tx) -> bool {
    g.c21.clock = t;
    let mut changed = false;
    // The keyboard-anchor scene opens as MenuKeys opens, once the list has laid out.
    let vp = g.c21.coll.viewport();
    if g.c21.open_by_keys && vp.h > 0.0 {
        g.c21.open_by_keys = false;
        let geometry = collection().geometry(g);
        let d = &mut g.c21;
        let a = keyboard_anchor(
            &d.listing,
            &d.coll.selection,
            geometry.as_ref(),
            d.coll.visible_content(),
            Point::new(vp.x, vp.y),
            AnchorEdge::Center,
        );
        d.heard(Intent::ContextMenu {
            target: a.target,
            anchor: strata_components::contract::Rect::new(a.at.x, a.at.y, 0.0, 0.0),
        });
        changed = true;
    }
    changed | collection().tick(g, tx)
}
