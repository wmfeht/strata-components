//! A tile: the thumbnail frame (C1e's icon slot), the name (its label slot) and the card that
//! stacks them with the details line (its content slot). The collection draws no chrome of
//! its own around a tile (`Collection::bare_items`): the frame and the name light up for the
//! tile's state, as Strata's `.file-icons` rules light `.icons-card-icon-frame` and
//! `.icons-card-label`, never the gutters between tiles.
//!
//! A line mounts all its tiles in one frame, so a tile is kept to few elements: the card (its
//! margin and padding in one), the frame, the thumbnail box with the icon and the image in it,
//! the gap, the name and the details line. The name and the details sit where their reserved
//! space puts them through margins, not wrappers.

use std::hash::{Hash, Hasher};
use std::rc::Rc;

use rune::{Align, Color, El, SurfaceFit, col, el, icon, row, text};
use strata_kit::icons::IconName;
use strata_kit::look::Look;
use strata_kit::slots::icons as tokens;

use super::Shared;
use super::caption::Cut;
use super::metrics::TileMetrics;
use crate::collection::element::{ContentSlot, ICON, LABEL, RowCx, RowState, Slot};
use crate::thumbnails::{Step, THUMB};

/// The key of a tile's thumbnail frame, under its cell (`row-{id}/frame`).
pub const FRAME: &str = "frame";

/// The key of a tile's details line, under its cell (`row-{id}/details`).
pub const DETAILS: &str = "details";

/// The key of the blank space between a tile's frame and its name, under its cell
/// (`row-{id}/gap`): blank space a band starts from, which scripts press.
pub const GAP: &str = "gap";

/// What one build worked out for every tile.
#[derive(Clone, Copy, Debug)]
pub(super) struct Built {
    pub m: TileMetrics,
    /// A cell's width.
    pub cell_w: f32,
    /// The width a name wraps at.
    pub caption_w: f32,
    /// The thumbnail step the grid draws at.
    pub step: Step,
}

impl Built {
    /// `name` as this build's captions show it.
    pub fn cut<S>(&self, shared: &Shared<S>, s: &S, name: &str) -> Cut {
        let parts = (shared.parts)(s);
        parts.grid.captions.get(
            name,
            self.m.name_font,
            self.caption_w,
            self.m.name_lines.max(1),
        )
    }

    /// A hash of everything this build draws every tile with: its sizes, the caption width
    /// (a name's cut follows from it and the name, which the row's own version holds) and the
    /// thumbnail step.
    pub fn key(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        self.m.key().hash(&mut h);
        self.caption_w.to_bits().hash(&mut h);
        self.step.hash(&mut h);
        h.finish()
    }
}

/// The frame's look for a tile's state: fill, outline and hover.
fn frame_look(
    look: &Look,
    st: RowState,
    hover: bool,
) -> (Option<Color>, Option<(f32, Color)>, Option<Color>) {
    let k = &look.kit;
    if st.drop_target {
        return (
            Some(tokens::frame_drop_fill(look)),
            Some((1.0, tokens::frame_focus_ring(look))),
            None,
        );
    }
    let ring = if st.cursor_shown {
        Some((2.0, tokens::frame_focus_ring(look)))
    } else if st.marks.enter_target {
        Some((1.0, k.c.text_accent))
    } else if st.selected {
        Some((1.0, tokens::frame_selected_ring(look)))
    } else {
        None
    };
    if st.selected {
        let fill = tokens::frame_selected_fill(look);
        // Strata deepens a selected frame under the pointer: 22 % to 30 % of the accent.
        let hovered = k.alpha(fill, (fill.a * 30.0 / 22.0).min(1.0));
        return (Some(fill), ring, hover.then_some(hovered));
    }
    (None, ring, hover.then(|| tokens::frame_hover_fill(look)))
}

/// The entry's catalog icon, else the Folder or File icon, in the frame's icon colour.
fn placeholder(rcx: &RowCx<'_>, points: f32) -> El {
    let look = rcx.look;
    let fallback = if rcx.entry.kind.is_folder_like() {
        IconName::Folder
    } else {
        IconName::File
    };
    let glyph = look
        .icon_named(rcx.entry.icon)
        .unwrap_or_else(|| look.icon(fallback));
    icon(glyph, points).color(tokens::frame_icon(look))
}

/// The icon slot: the frame, holding the slot box ([`ICON`], so C23's flights find it) with
/// the entry's placeholder icon and, when it has a thumbnail key and thumbnails show, its
/// image over the icon. As C25's `picture` draws them: the image is fitted (letterboxed) in
/// the box, and the icon fades out through the handle's opacity while an image is held, so a
/// thumbnail landing, changing size or going needs no build.
///
/// Versioned by the store's handles for the key and the step, never the key's state.
pub(super) fn icon_slot<S: 'static>(shared: &Rc<Shared<S>>, b: Built) -> Slot<S> {
    let (version_shared, build_shared) = (shared.clone(), shared.clone());
    let key = b.key();
    Slot::new(
        move |s: &S, rcx: &RowCx<'_>| match (rcx.entry.thumbnail, (version_shared.parts)(s).thumbs)
        {
            (Some(k), Some((store, _))) => key ^ store.version(k).rotate_left(29),
            _ => key,
        },
        move |s: &S, rcx: &RowCx<'_>| {
            let m = &b.m;
            let look = rcx.look;
            let glyph = placeholder(rcx, m.slot);
            let mut slot = row().key(ICON).w(m.slot).h(m.slot).center();
            slot = match (rcx.entry.thumbnail, (build_shared.parts)(s).thumbs) {
                (Some(key), Some((store, _))) if store.enabled() => {
                    let thumb = store.thumb(key);
                    slot.child(glyph.live_opacity(thumb.icon_alpha())).child(
                        el().key(THUMB)
                            .absolute(0.0, 0.0)
                            .w(m.slot)
                            .h(m.slot)
                            .pixels_fit(thumb.pixels(), SurfaceFit::Fit),
                    )
                }
                _ => slot.child(glyph),
            };
            let (fill, ring, hover) = frame_look(look, rcx.state, rcx.hover_live);
            let mut frame = row()
                .key(FRAME)
                .w(m.frame)
                .h(m.frame)
                .center()
                .radius(tokens::frame_radius(look))
                .a11y_hidden()
                .child(slot);
            if let Some(fill) = fill {
                frame = frame.fill(fill);
            }
            if let Some((w, c)) = ring {
                frame = frame.outline(w, -w, c);
            }
            if let Some(c) = hover {
                frame = frame.hover(c);
            }
            frame
        },
    )
}

/// The label slot: the name, cut to its lines, in a padded box that lights up for the tile's
/// state, with the space its lines leave below it as margin, so the details line sits where
/// the name's reserved lines end. Rename's editor takes its place while the tile renames
/// (C1e's rename slot).
pub(super) fn label_slot<S: 'static>(shared: &Rc<Shared<S>>, b: Built) -> Slot<S> {
    let build_shared = shared.clone();
    let key = b.key();
    Slot::new(
        move |_: &S, _: &RowCx<'_>| key,
        move |s: &S, rcx: &RowCx<'_>| {
            let m = &b.m;
            let look = rcx.look;
            let st = rcx.state;
            let cut = b.cut(&build_shared, s, rcx.entry.name);
            let lit = st.selected || st.drop_target;
            let color = if lit {
                tokens::caption_selected_text(look)
            } else {
                look.kit.c.text_base
            };
            let below = (m.caption_height() - (cut.size.1 + m.caption_pad)).max(0.0);
            let mut name = text(cut.text.to_string())
                .key(LABEL)
                .a11y_hidden()
                .font(m.name_font)
                .color(color)
                .align(Align::Center)
                .wrap_anywhere()
                .max_lines(m.name_lines.max(1))
                .px(m.caption_pad)
                .py(m.caption_pad / 2.0)
                .mb(below)
                .radius(tokens::caption_radius(look))
                .min_w(0.0)
                .shrink();
            if lit {
                name = name.fill(tokens::caption_selected_fill(look));
            } else if rcx.hover_live {
                name = name.hover(tokens::caption_hover_fill(look));
            }
            name
        },
    )
}

/// The details line's text for a tile, from the host.
pub(super) fn details_of<S>(shared: &Shared<S>, s: &S, rcx: &RowCx<'_>) -> Option<String> {
    let (_, f) = shared.details.as_ref()?;
    f(s, rcx).map(|d| d.to_string())
}

/// The content slot: the card, its frame, name and details stacked and centred, inside the
/// tile's margin and the card's padding. A tile whose name is cut carries the whole name as a
/// tooltip over its frame, its name and its details, none while it renames.
pub(super) fn content_slot<S: 'static>(shared: &Rc<Shared<S>>, b: Built) -> ContentSlot<S> {
    let (version_shared, build_shared) = (shared.clone(), shared.clone());
    let key = b.key();
    ContentSlot::new(
        move |s: &S, rcx: &RowCx<'_>| match &version_shared.details {
            Some((version, _)) => key ^ version(s, rcx).rotate_left(17),
            None => key,
        },
        move |s: &S, rcx: &RowCx<'_>, frame: El, label: El| {
            let m = &b.m;
            let look = rcx.look;
            let help = (b.cut(&build_shared, s, rcx.entry.name).cut && !rcx.state.renaming)
                .then(|| rcx.entry.name.to_owned());
            let (frame, label) = match &help {
                Some(name) => (frame.help(name.clone()), label.help(name.clone())),
                None => (frame, label),
            };
            let mut card = col()
                .w_full()
                .h_full()
                .px(m.margin + m.card_x)
                .py(m.margin + m.card_y)
                .items_center()
                .child(frame);
            if m.name_lines > 0 {
                // The rename editor (C5's) keeps the name's reserved lines, so the details line
                // stays put.
                let label = if rcx.state.renaming {
                    col()
                        .w_full()
                        .h(m.caption_height())
                        .items_center()
                        .child(label)
                } else {
                    label
                };
                card = card
                    .child(el().key(GAP).w_full().h(m.caption_gap))
                    .child(label);
            }
            if m.details_line > 0.0
                && let Some(details) = details_of(&build_shared, s, rcx).filter(|d| !d.is_empty())
            {
                let mut line = text(details)
                    .key(DETAILS)
                    .a11y_hidden()
                    .font(m.details_font)
                    .color(look.kit.c.text_muted)
                    .align(Align::Center)
                    .truncate()
                    .h(m.details_line)
                    .px(m.caption_pad)
                    .min_w(0.0)
                    .shrink();
                if let Some(name) = help {
                    line = line.help(name);
                }
                card = card.child(line);
            }
            card
        },
    )
}
