//! The gallery's page registry: the index, then one page per component and per contract or
//! look work package that shows something.
//!
//! W0 declares every page here once. A work package edits only its own `scenes/<id>.rs`,
//! which provides `Demo` (its field in [`Gallery`]), `SCENES`, `REVIEW` and `view`.

use rune::{El, text};

use crate::{Gallery, Scene, View};

pub mod c1;
pub mod c10;
pub mod c10a;
pub mod c11;
pub mod c12;
pub mod c13;
pub mod c14a;
pub mod c14b;
pub mod c15;
pub mod c16;
pub mod c17;
pub mod c18;
pub mod c19;
pub mod c2;
pub mod c20a;
pub mod c20b;
pub mod c20c;
pub mod c20d;
pub mod c21;
pub mod c22;
pub mod c23;
pub mod c24;
pub mod c25;
pub mod c26;
pub mod c3;
pub mod c4;
pub mod c5;
pub mod c6;
pub mod c7;
pub mod c8;
pub mod c9;
pub mod gallery;
pub mod k1;
pub mod k2;
pub mod k3;
pub mod k4;
pub mod w1;
pub mod w2;

/// One gallery page.
pub struct Page {
    /// The page id: the prefix of its scene names and its golden and script folder name.
    pub id: &'static str,
    /// The work package that owns the page, as its docs name it (`C1e`).
    pub wp: &'static str,
    pub name: &'static str,
    /// The work package doc, relative to the repository root.
    pub doc: Option<&'static str>,
    /// Golden scenes: each has an accessibility and a PNG golden.
    pub scenes: &'static [Scene],
    /// Review scenes: reachable with `--scene`, kept out of the goldens.
    pub review: &'static [Scene],
    pub view: View,
}

macro_rules! page {
    ($id:ident, $wp:literal, $name:literal, $doc:literal) => {
        Page {
            id: stringify!($id),
            wp: $wp,
            name: $name,
            doc: Some(concat!("docs/work/", $doc)),
            scenes: $id::SCENES,
            review: $id::REVIEW,
            view: $id::view,
        }
    };
}

/// The index page, open when `Gallery::page` is `None`.
pub const INDEX: Page = Page {
    id: "gallery",
    wp: "W0",
    name: "Index",
    doc: None,
    scenes: gallery::SCENES,
    review: gallery::REVIEW,
    view: gallery::view,
};

/// Every page but the index, in the order the index lists them.
pub const PAGES: &[Page] = &[
    page!(
        w1,
        "W1",
        "Component contracts",
        "phase-0-groundwork/W1-component-contracts.md"
    ),
    page!(
        w2,
        "W2",
        "Look contract",
        "phase-0-groundwork/W2-look-contract.md"
    ),
    page!(
        k1,
        "K1",
        "Theme mapping",
        "phase-1-leaves/K1-theme-mapping.md"
    ),
    page!(
        k2,
        "K2",
        "Shape, density and type",
        "phase-1-leaves/K2-shape-density-type.md"
    ),
    page!(
        k3,
        "K3",
        "Icon catalogs",
        "phase-1-leaves/K3-icon-catalogs.md"
    ),
    page!(
        k4,
        "K4",
        "User overrides",
        "phase-1-leaves/K4-user-overrides.md"
    ),
    page!(
        c1,
        "C1e",
        "Collection element",
        "phase-2-composites/C1e-collection-element.md"
    ),
    page!(c2, "C2", "ColumnsView", "phase-3-views/C2-columns-view.md"),
    page!(c3, "C3", "IconsGrid", "phase-3-views/C3-icons-grid.md"),
    page!(c4, "C4", "ListTable", "phase-3-views/C4-list-table.md"),
    page!(
        c5,
        "C5",
        "InlineRenameEditor",
        "phase-1-leaves/C5-inline-rename.md"
    ),
    page!(c6, "C6", "LocationBar", "phase-1-leaves/C6-location-bar.md"),
    page!(c7, "C7", "BrowserTabs", "phase-1-leaves/C7-browser-tabs.md"),
    page!(
        c8,
        "C8",
        "PlacesSidebar",
        "phase-1-leaves/C8-places-sidebar.md"
    ),
    page!(
        c9,
        "C9",
        "PreviewDrawer",
        "phase-1-leaves/C9-preview-drawer.md"
    ),
    page!(
        c10,
        "C10b",
        "CodeView",
        "phase-2-composites/C10b-code-view.md"
    ),
    page!(
        c10a,
        "C10a",
        "Highlighter",
        "phase-1-leaves/C10a-highlighter.md"
    ),
    page!(
        c11,
        "C11",
        "DocumentView",
        "phase-2-composites/C11-document-view.md"
    ),
    page!(c12, "C12", "PdfView", "phase-2-composites/C12-pdf-view.md"),
    page!(
        c13,
        "C13",
        "MediaPlayer",
        "phase-2-composites/C13-media-player.md"
    ),
    page!(
        c14a,
        "C14a",
        "Pixels viewport",
        "phase-1-leaves/C14a-pixels-viewport.md"
    ),
    page!(
        c14b,
        "C14b",
        "Viewers and archive preview",
        "phase-3-views/C14b-viewers.md"
    ),
    page!(
        c15,
        "C15",
        "StrataDialog",
        "phase-1-leaves/C15-strata-dialog.md"
    ),
    page!(
        c16,
        "C16",
        "ProgressDock",
        "phase-2-composites/C16-progress-dock.md"
    ),
    page!(
        c17,
        "C17",
        "SearchPalette",
        "phase-2-composites/C17-search-palette.md"
    ),
    page!(
        c18,
        "C18",
        "FilterBar and FooterPrompt",
        "phase-2-composites/C18-filter-bar-footer-prompt.md"
    ),
    page!(
        c19,
        "C19",
        "Footer and ShortcutReference",
        "phase-2-composites/C19-footer-shortcut-reference.md"
    ),
    page!(
        c20a,
        "C20a",
        "Slider and NumberField",
        "phase-1-leaves/C20a-slider-number-field.md"
    ),
    page!(
        c20b,
        "C20b",
        "ColorPicker",
        "phase-1-leaves/C20b-color-picker.md"
    ),
    page!(
        c20c,
        "C20c",
        "Tabbed form and settings search",
        "phase-1-leaves/C20c-tabbed-form-settings-search.md"
    ),
    page!(
        c20d,
        "C20d",
        "IconPicker",
        "phase-4-integration/C20d-icon-picker.md"
    ),
    page!(
        c21,
        "C21",
        "Context menus",
        "phase-1-leaves/C21-context-menus.md"
    ),
    page!(
        c22,
        "C22",
        "Folder peek",
        "phase-1-leaves/C22-folder-peek.md"
    ),
    page!(
        c23,
        "C23",
        "Motion effects",
        "phase-1-leaves/C23-motion-effects.md"
    ),
    page!(
        c24,
        "C24",
        "Scroll extras",
        "phase-1-leaves/C24-scroll-extras.md"
    ),
    page!(
        c25,
        "C25",
        "ThumbnailStore",
        "phase-1-leaves/C25-thumbnail-store.md"
    ),
    page!(
        c26,
        "C26",
        "MountPrompts",
        "phase-2-composites/C26-mount-prompts.md"
    ),
];

/// The index, then every page.
pub fn all() -> impl Iterator<Item = &'static Page> {
    std::iter::once(&INDEX).chain(PAGES)
}

/// The page with id `id`.
pub fn find(id: &str) -> Option<&'static Page> {
    all().find(|p| p.id == id)
}

/// Every page's golden scenes, then every page's review scenes, for the app's `--scene`.
pub fn all_scenes() -> Vec<Scene> {
    let golden = all().flat_map(|p| p.scenes.iter().copied());
    golden
        .chain(all().flat_map(|p| p.review.iter().copied()))
        .collect()
}

/// The page `g` has open: the index when no page is, or when the id is unknown.
pub fn open_page(g: &Gallery) -> &'static Page {
    g.page.and_then(find).unwrap_or(&INDEX)
}

/// What a page shows before its work package builds it.
pub(crate) fn placeholder(g: &Gallery, id: &str) -> El {
    let kit = g.kit();
    let label = find(id).map_or_else(
        || id.to_owned(),
        |p| format!("{} {}: not built yet", p.wp, p.name),
    );
    rune::col().w_full().h_full().center().child(
        text(label)
            .key(format!("{id}-placeholder"))
            .font(kit.ty.ui_medium)
            .color(kit.c.text_base),
    )
}
