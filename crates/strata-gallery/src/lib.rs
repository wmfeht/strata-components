//! The gallery's state, pages and view, shared by the binary and the golden tests.
//!
//! The gallery has one page per component (and per contract or look work package that shows
//! something). Each page lives in `scenes/<id>.rs` with its scenes, its demo state and its
//! view; `docs/testing.md` describes the layout and the goldens built from it.

use rune::{Cx, El, col};
use rune_kit::Kit;

pub mod scenes;

/// The window size every scene renders at, headless and in a window.
pub const SIZE: (f32, f32) = (960.0, 640.0);

/// A named state the gallery opens in with `--scene <name>`. Each golden scene also has an
/// accessibility and a PNG golden.
pub type Scene = (&'static str, fn(&mut Gallery));

/// A page's view.
pub type View = fn(&Gallery, &mut Cx<Gallery>) -> El;

/// The gallery's state: the open page, light or dark, and one demo state per page.
#[derive(Default)]
pub struct Gallery {
    pub dark: bool,
    /// The open page's id, or `None` for the index.
    pub page: Option<&'static str>,
    pub w1: scenes::w1::Demo,
    pub w2: scenes::w2::Demo,
    pub k1: scenes::k1::Demo,
    pub k2: scenes::k2::Demo,
    pub k3: scenes::k3::Demo,
    pub k4: scenes::k4::Demo,
    pub c1: scenes::c1::Demo,
    pub c2: scenes::c2::Demo,
    pub c3: scenes::c3::Demo,
    pub c4: scenes::c4::Demo,
    pub c5: scenes::c5::Demo,
    pub c6: scenes::c6::Demo,
    pub c7: scenes::c7::Demo,
    pub c8: scenes::c8::Demo,
    pub c9: scenes::c9::Demo,
    pub c10: scenes::c10::Demo,
    pub c10a: scenes::c10a::Demo,
    pub c11: scenes::c11::Demo,
    pub c12: scenes::c12::Demo,
    pub c13: scenes::c13::Demo,
    pub c14a: scenes::c14a::Demo,
    pub c14b: scenes::c14b::Demo,
    pub c15: scenes::c15::Demo,
    pub c16: scenes::c16::Demo,
    pub c17: scenes::c17::Demo,
    pub c18: scenes::c18::Demo,
    pub c19: scenes::c19::Demo,
    pub c20a: scenes::c20a::Demo,
    pub c20b: scenes::c20b::Demo,
    pub c20c: scenes::c20c::Demo,
    pub c20d: scenes::c20d::Demo,
    pub c21: scenes::c21::Demo,
    pub c22: scenes::c22::Demo,
    pub c23: scenes::c23::Demo,
    pub c24: scenes::c24::Demo,
    pub c25: scenes::c25::Demo,
    pub c26: scenes::c26::Demo,
}

impl Gallery {
    /// Opens the page `page` in light or dark. Every scene except the index's calls this.
    pub fn open(&mut self, page: &'static str, dark: bool) {
        self.page = Some(page);
        self.dark = dark;
    }

    /// The kit pages draw with.
    pub fn kit(&self) -> Kit {
        strata_kit::fallback(self.dark)
    }
}

/// The open page on the gallery's background.
pub fn view(g: &Gallery, cx: &mut Cx<Gallery>) -> El {
    let page = scenes::open_page(g);
    col()
        .key("gallery")
        .w_full()
        .h_full()
        .fill(g.kit().c.bg_base)
        .child((page.view)(g, cx))
}
