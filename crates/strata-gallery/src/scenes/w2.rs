//! The gallery page for W2 Look contract: its scenes, demo state, view and key and poll hooks.
//! Scene names start with `w2-` and end in `-light` or `-dark`.
//!
//! Work package: `docs/work/phase-0-groundwork/W2-look-contract.md`.
//!
//! Two sheets, both read from `Gallery::look`: the tokens (colour swatches by registry name,
//! the type ramp, spacing and sizes, radii), and every semantic icon in each catalog. K3's
//! catalogs replace the placeholders behind the icon sheet with no change here.

use rune::{Color, Cx, El, KeyEvent, TextStyle, Tx, col, el, icon, row, text};
use strata_kit::icons::{self, IconName, IconSet};
use strata_kit::look::Look;
use strata_kit::tokens::{self, Value};

use crate::{Gallery, Scene};

/// W2's demo state, held in `Gallery::w2`.
#[derive(Default)]
pub struct Demo {
    pub sheet: Sheet,
}

/// What the page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sheet {
    #[default]
    Tokens,
    Icons,
}

fn open(g: &mut Gallery, dark: bool, sheet: Sheet) {
    g.open("w2", dark);
    g.w2.sheet = sheet;
}

/// Golden scenes: each gets an accessibility and a PNG golden. Each calls
/// `g.open("w2", dark)`, then fills in `g.w2`.
pub const SCENES: &[Scene] = &[
    ("w2-tokens-light", |g| open(g, false, Sheet::Tokens)),
    ("w2-tokens-dark", |g| open(g, true, Sheet::Tokens)),
    ("w2-icons-light", |g| open(g, false, Sheet::Icons)),
    ("w2-icons-dark", |g| open(g, true, Sheet::Icons)),
];

/// Scenes for review with `--scene` only, kept out of the goldens.
pub const REVIEW: &[Scene] = &[];

pub fn view(g: &Gallery, _cx: &mut Cx<Gallery>) -> El {
    let look = g.look();
    match g.w2.sheet {
        Sheet::Tokens => tokens_sheet(look),
        Sheet::Icons => icons_sheet(look),
    }
}

/// Keys no focused element took while this page is open: shortcuts, and keys with nothing
/// focused. True consumes the key; false lets the gallery's Escape go back to the index.
pub fn on_key(_g: &mut Gallery, _key: &KeyEvent, _tx: &mut Tx) -> bool {
    false
}

/// Runs at the start of every frame while this page is open, with the clock in seconds.
/// Deliver fake async results here; a worker wakes the loop with `Gallery::waker`. True when
/// state changed.
pub fn poll(_g: &mut Gallery, _t: f64, _tx: &mut Tx) -> bool {
    false
}

/// Rows per column of colour swatches.
const SWATCH_ROWS: usize = 20;

fn tokens_sheet(look: &Look) -> El {
    let s = &look.space;
    row()
        .key("w2-tokens")
        .w_full()
        .h_full()
        .padding(s.xl)
        .gap(s.xxl)
        .child(
            col()
                .gap(s.lg)
                .child(section(look, "color", swatches(look, "color.")))
                .child(section(look, "syntax", swatches(look, "syntax.")))
                .child(section(look, "radius", radii(look))),
        )
        .child(
            col()
                .gap(s.lg)
                .child(section(look, "type", type_ramp(look)))
                .child(
                    row()
                        .gap(s.xl)
                        .child(section(
                            look,
                            "space",
                            lengths(look, &["space.", "content."]),
                        ))
                        .child(section(
                            look,
                            "sizes",
                            lengths(look, &["frame.", "rows.", "icon_size."]),
                        )),
                )
                .child(section(look, "glow", glow(look))),
        )
}

/// A titled group of samples.
fn section(look: &Look, title: &str, body: El) -> El {
    let k = &look.kit;
    col()
        .key(format!("w2-{title}"))
        .gap(look.space.sm)
        .child(text(title).font(look.ty.heading).color(k.c.text_muted))
        .child(body)
}

/// A swatch per colour token whose name starts with `prefix`, labelled with the rest of
/// its name.
fn swatches(look: &Look, prefix: &str) -> El {
    let colours: Vec<(&'static str, Color)> = tokens::all()
        .filter_map(|t| match t.get(look) {
            Value::Color(c) => t.name().strip_prefix(prefix).map(|n| (n, c)),
            _ => None,
        })
        .collect();
    row()
        .gap(look.space.lg)
        .children(colours.chunks(SWATCH_ROWS).map(|column| {
            col()
                .gap(look.space.xxs)
                .children(column.iter().map(|&(name, c)| swatch(look, name, c)))
        }))
}

fn swatch(look: &Look, name: &str, colour: Color) -> El {
    let k = &look.kit;
    row()
        .gap(look.space.xs)
        .items_center()
        .child(
            el().size(look.icon_size.sm, look.icon_size.sm)
                .fill(colour)
                .radius(k.radius.xs)
                .border(1.0, k.c.border_base),
        )
        .child(text(name.to_owned()).font(k.ty.small).color(k.c.text_base))
}

/// A sample line in each style of Kit's ramp and Strata's.
fn type_ramp(look: &Look) -> El {
    let t = &look.kit.ty;
    let styles: [(&str, TextStyle); 14] = [
        ("title", look.ty.title),
        ("heading", look.ty.heading),
        ("body", t.body),
        ("body_medium", t.body_medium),
        ("ui", t.ui),
        ("ui_medium", t.ui_medium),
        ("ui_compact", t.ui_compact),
        ("text_small", t.text_small),
        ("text_small_medium", t.text_small_medium),
        ("text_xsmall", t.text_xsmall),
        ("text_xsmall_medium", t.text_xsmall_medium),
        ("small", t.small),
        ("small_medium", t.small_medium),
        ("mono", t.mono),
    ];
    col().children(styles.into_iter().map(|(name, style)| {
        text(format!("{name} {}/{}", style.size, style.line_height))
            .font(style)
            .color(look.kit.c.text_base)
    }))
}

/// A bar per length token under each prefix, as long as its value.
fn lengths(look: &Look, prefixes: &[&str]) -> El {
    let k = &look.kit;
    let bars = tokens::all().filter_map(|t| match t.get(look) {
        Value::Length(x) if prefixes.iter().any(|p| t.name().starts_with(p)) => Some((t.name(), x)),
        _ => None,
    });
    col().gap(look.space.xxs).children(bars.map(|(name, x)| {
        row()
            .gap(look.space.xs)
            .items_center()
            .child(
                text(format!("{name} {x}"))
                    .font(k.ty.small)
                    .color(k.c.text_base)
                    .w(110.0),
            )
            .child(
                el().size(x, look.space.xs)
                    .fill(k.c.icon_accent)
                    .radius(k.radius.xs),
            )
    }))
}

fn radii(look: &Look) -> El {
    let k = &look.kit;
    let r = k.radius;
    let side = look.frame.control;
    row().gap(look.space.lg).children(
        [
            ("xs", r.xs),
            ("sm", r.sm),
            ("md", r.md),
            ("lg", r.lg),
            ("xl", r.xl),
        ]
        .map(|(name, radius)| {
            col()
                .gap(look.space.xxs)
                .items_center()
                .child(
                    el().size(side, side)
                        .radius(radius)
                        .fill(k.c.layer_02)
                        .border(1.0, k.c.border_strong),
                )
                .child(
                    text(format!("{name} {radius}"))
                        .font(k.ty.small)
                        .color(k.c.text_base),
                )
        }),
    )
}

fn glow(look: &Look) -> El {
    let k = &look.kit;
    let g = &look.glow;
    row()
        .gap(look.space.md)
        .items_center()
        .child(swatch(look, "color", g.color))
        .child(
            text(format!(
                "on {}, radius {}, intensity {}",
                g.on, g.radius, g.intensity
            ))
            .font(k.ty.small)
            .color(k.c.text_base),
        )
}

/// Every semantic icon, in each catalog.
fn icons_sheet(look: &Look) -> El {
    col()
        .key("w2-icons")
        .w_full()
        .h_full()
        .padding(look.space.xl)
        .gap(look.space.lg)
        .children(icons::catalogs().into_iter().map(|set| catalog(look, set)))
}

fn catalog(look: &Look, set: IconSet) -> El {
    let k = &look.kit;
    let cells = IconName::ALL.iter().map(|&name| {
        col()
            .w(86.0)
            .gap(look.space.xxs)
            .items_center()
            .child(icon((set.icon)(name), look.icon_size.md).color(k.c.icon_base))
            .child(text(name.id()).font(k.ty.small).color(k.c.text_muted))
    });
    section(
        look,
        set.name,
        row()
            .w_full()
            .flex_wrap()
            .gap(look.space.xs)
            .children(cells),
    )
}
