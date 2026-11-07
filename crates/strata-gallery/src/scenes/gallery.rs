//! The gallery's index page: the title, then one row per page, which opens it. Escape on a
//! page comes back here. Its scenes are `gallery-index-light` and `gallery-index-dark`.
//!
//! The index lists only what W0 declares (each page's work package and name), never a page's
//! scenes, so its goldens don't change when a work package adds scenes.

use rune::{Cx, El, KeyEvent, Tx, col, row, text};

use strata_kit::look::Look;

use super::PAGES;
use crate::{Gallery, Scene};

pub const SCENES: &[Scene] = &[
    ("gallery-index-light", |g| {
        (g.dark, g.look) = (false, Look::fallback(false))
    }),
    ("gallery-index-dark", |g| {
        (g.dark, g.look) = (true, Look::fallback(true))
    }),
];

pub const REVIEW: &[Scene] = &[];

/// Rows per column of the index.
const COLUMN: usize = 13;

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    let kit = g.kit();
    let entry = |p: &super::Page| {
        let id = p.id;
        text(format!("{} {}", p.wp, p.name))
            .key(format!("index-{id}"))
            .role("link")
            .font(kit.ty.ui_compact)
            .color(kit.c.text_base)
            .on_press(move |g: &mut Gallery| g.page = Some(id))
    };
    let columns = PAGES
        .chunks(COLUMN)
        .map(|ps| col().gap(4.0).w(240.0).children(ps.iter().map(entry)));
    col()
        .w_full()
        .h_full()
        .center()
        .gap(24.0)
        .child(
            text("Strata components")
                .key("title")
                .font(kit.ty.ui_medium)
                .color(kit.c.text_base),
        )
        .child(
            row()
                .key("index")
                .role("navigation")
                .label("Components")
                .gap(24.0)
                .children(columns),
        )
}

pub fn on_key(_g: &mut Gallery, _key: &KeyEvent, _tx: &mut Tx) -> bool {
    false
}

pub fn poll(_g: &mut Gallery, _t: f64, _tx: &mut Tx) -> bool {
    false
}
