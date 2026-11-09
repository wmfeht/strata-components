//! What shows instead of pages: "Preview unavailable" and a message, with an Open button for a
//! password-protected PDF (Strata's preview message; `preview/preview-panel`, "Images and
//! PDFs"), and its keys (`preview/quick-preview`, `integration/10xer-mode/preview-keys`).
//!
//! The content box takes focus; Down or Right (`j` or `l` with [`PdfView::letters`]) moves it
//! to Open. On Open, Return opens, Left (`h`) hands the keys back to the listing
//! ([`PdfIntent::Leave`]), and Up, Down and Right (`j`, `k`, `l`) keep focus there. Moving focus
//! uses a zero-size `focus_return` stub, as C9's drawer does: Rune can't focus an element from
//! code at the pinned rev (upstream #173).
//!
//! [`PdfView::letters`]: super::PdfView::letters
//! [`PdfIntent::Leave`]: super::PdfIntent::Leave

use rune::{El, Key, KeyEvent, Transition, Tx, col, el, text};
use rune_kit::{Button, ButtonVariant};
use strata_kit::look::Look;

use super::render::Ctx;
use super::{PdfIntent, PdfState};
use crate::dialog::FOCUS_RING_WIDTH;

pub(crate) fn build<S: 'static>(
    ctx: &Ctx<S>,
    st: &PdfState,
    message: &str,
    open: bool,
    look: &Look,
) -> El {
    let k = &look.kit;
    let key = st.key().to_owned();
    let get_mut = ctx.get_mut;
    let letters = ctx.letters;
    let mut body = col()
        .key(key.clone())
        .role("group")
        .label(ctx.labels.unavailable.clone())
        .w_full()
        .h_full()
        .padding(look.space.xl)
        .gap(look.space.sm)
        .center()
        .focusable(true)
        .focus_ring(FOCUS_RING_WIDTH, -FOCUS_RING_WIDTH, k.c.border_focus)
        .child(
            text(ctx.labels.unavailable.clone())
                .key(format!("{key}-unavailable"))
                .font(look.ty.title)
                .color(k.c.text_base),
        )
        .child(
            text(message.to_owned())
                .key(format!("{key}-message"))
                .font(k.ty.ui)
                .color(k.c.text_muted)
                .wrap()
                .align(rune::Align::Center),
        );
    if !open {
        return body;
    }
    body = body.on_key(move |s: &mut S, ev: &KeyEvent, _tx: &mut Tx| {
        if !plain(ev) {
            return false;
        }
        let to_open = matches!(ev.key, Key::Down | Key::Right)
            || letters && matches!(&ev.key, Key::Text(t) if t == "j" || t == "l");
        if to_open {
            get_mut(s).focus_open_button();
        }
        to_open
    });
    let on_open = ctx.intent.clone();
    let on_key = ctx.intent.clone();
    let button = Button::new(ctx.labels.open.clone())
        .variant(ButtonVariant::Neutral)
        .on_press_tx(move |s: &mut S, tx: &mut Tx| {
            if let Some(f) = &on_open {
                f(s, PdfIntent::Open, tx);
            }
        })
        .build(k)
        .key(format!("{key}-open"))
        .mt(look.space.sm)
        // Inside the button's edge, as Strata's inset ring.
        .focus_ring(FOCUS_RING_WIDTH, -FOCUS_RING_WIDTH, k.c.border_focus)
        .on_key(move |s: &mut S, ev: &KeyEvent, tx: &mut Tx| {
            if !plain(ev) {
                return false;
            }
            let send = |s: &mut S, i: PdfIntent, tx: &mut Tx| {
                if let Some(f) = &on_key {
                    f(s, i, tx);
                }
            };
            let letter = |c: &str| letters && matches!(&ev.key, Key::Text(t) if t == c);
            match ev.key {
                Key::Enter => send(s, PdfIntent::Open, tx),
                Key::Left => send(s, PdfIntent::Leave, tx),
                Key::Up | Key::Down | Key::Right => {}
                _ if letter("h") => send(s, PdfIntent::Leave, tx),
                _ if letter("j") || letter("k") || letter("l") => {}
                _ => return false,
            }
            true
        });
    body.child(button).child(
        el().key(format!("{key}-open-focus-{}", st.focus_open))
            .size(0.0, 0.0)
            .a11y_hidden()
            .transition(Transition::none())
            .focus_return(format!("{key}-open")),
    )
}

fn plain(ev: &KeyEvent) -> bool {
    let m = ev.mods;
    !(m.shift || m.ctrl || m.cmd || m.alt || m.logo)
}
