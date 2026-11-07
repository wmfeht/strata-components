//! The gallery's state, scenes and view, shared by the binary and the golden tests.

use rune::{Cx, El, col, text};

#[derive(Default)]
pub struct Gallery {
    pub dark: bool,
}

/// A named state the gallery opens in with `--scene <name>`. Each is also an a11y golden.
pub type Scene = (&'static str, fn(&mut Gallery));

pub const SCENES: &[Scene] = &[
    ("empty-light", |g| g.dark = false),
    ("empty-dark", |g| g.dark = true),
];

pub fn view(g: &Gallery, _: &mut Cx<Gallery>) -> El {
    let kit = strata_kit::fallback(g.dark);
    col()
        .key("gallery")
        .w_full()
        .h_full()
        .center()
        .fill(kit.c.bg_base)
        .child(
            text("Strata components")
                .key("title")
                .font(kit.ty.ui_medium)
                .color(kit.c.text_base),
        )
}
