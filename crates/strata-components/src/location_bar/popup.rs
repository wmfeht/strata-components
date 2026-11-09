//! The suggestion list: anchored under the entry at its width, taking no focus, on rune-kit's
//! public popover parts. Not rune-kit's `PopupList`, whose rows can't draw matched ranges
//! (Rune draft U-C6-4).

use rune::{Color, El, GrowthSpec, ScrollSpec, Tx, col, icon, row, text};
use rune_kit::{Placement, popover};

use super::build::{Ctx, field_key, option_key, route};
use crate::contract::MatchedText;

/// The most rows the list shows before it scrolls (Strata's 280 px over 32 px rows).
const ROWS_SHOWN: f32 = 8.0;

pub(super) fn view<S: 'static>(cx: &Ctx<'_, S>, width: f32, layer: &mut Vec<El>) {
    let look = cx.look;
    let k = &look.kit;
    let shown = cx.state.suggestions();
    let n = shown.len();
    let active = cx.state.highlighted();
    let row_h = look.row.list;
    let access = cx.access;
    let mut rows = col()
        .key("rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtualize(row_h)
        .reveal_selected(true);
    for (i, c) in shown.iter().enumerate() {
        let is_active = active == Some(i);
        let emit = cx.emit.clone();
        let label = MatchedText::new(c.label.clone(), c.matched.iter().cloned())
            .font(k.ty.ui_compact)
            .truncate(true)
            .build(k);
        rows = rows.child(
            row()
                .key(option_key(cx.key, i))
                .role("option")
                .label(c.label.clone())
                .description(c.hint.clone())
                .position_in_set(i + 1, n)
                .focus_selected(is_active)
                .w_full()
                .h(row_h)
                .px(look.space.sm)
                .gap(look.space.sm)
                .items_center()
                .radius(k.radius.sm)
                .fill(if is_active {
                    k.c.hover
                } else {
                    Color::TRANSPARENT
                })
                .hover(k.c.hover)
                .on_hover(move |s: &mut S| access(s).set_highlighted(Some(i)))
                .on_press_tx(move |s: &mut S, tx: &mut Tx| {
                    let polled = access(s).choose(i);
                    route(&emit, s, polled, tx);
                })
                .child(
                    icon(look.icon(c.icon), look.icon_size.md)
                        .color(k.c.icon_muted)
                        .a11y_hidden(),
                )
                .child(row().grow(1.0).min_w(0.0).a11y_hidden().child(label))
                .child(
                    text(c.hint.clone())
                        .font(k.ty.small)
                        .color(k.c.text_weak)
                        .truncate()
                        .shrink()
                        .a11y_hidden(),
                ),
        );
    }
    let viewport = col()
        .key(format!("{}-suggestions", cx.key))
        .role("listbox")
        .label(cx.labels.suggestions.clone())
        .w_full()
        .h((n as f32 * row_h).min(ROWS_SHOWN * row_h))
        .min_h(row_h.min(n as f32 * row_h))
        .shrink()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_edge_fade(look.space.lg)
        .scroll_margin(look.space.lg)
        .clip_soft(0.0)
        .child(rows);
    let content = col()
        .key(format!("{}-suggestions-panel", cx.key))
        .role("none")
        .w(width)
        .padding(look.space.xxs)
        // The entry keeps the keyboard while the pointer picks a row.
        .preserve_focus_on_press(true)
        .on_press(|_: &mut S| {})
        .instant_changes()
        .child(viewport);
    layer.push(popover::anchored(
        k,
        popover::raised(k, content, k.radius.md),
        &field_key(cx.key),
        Placement::BottomStart,
        look.space.xs,
    ));
}
