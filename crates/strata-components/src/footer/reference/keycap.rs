//! TEMPORARY: a copy of C18's keycap chip (`filter_bar/keycap.rs` on feat/c18-filter-bar), until
//! C18 merges; then the reference draws `crate::filter_bar::keycap` and this file goes.
//!
//! Strata's keycap look (`.sidebar-keycap`, `src/style.css`): the text colour on the base fill,
//! a 1 px border, small corners, bold small type. Not rune-kit's `Keybind`, which upper-cases a
//! single character ("g" draws G; Rune draft U-C18-1).

use rune::{El, row, text};
use strata_kit::look::Look;

/// The chip for `key` ("Ctrl+Shift+T", "g", "1–9"). Decorative: the row it sits in names the
/// keys for assistive technology.
pub(super) fn keycap(look: &Look, key: &str) -> El {
    let k = &look.kit;
    row()
        .px(look.space.xs)
        .center()
        .radius(k.radius.sm)
        .fill(k.c.bg_base)
        .border(1.0, k.c.border_base)
        .a11y_hidden()
        .child(
            text(key.to_owned())
                .font(k.ty.small_medium)
                .color(k.c.text_base),
        )
}
