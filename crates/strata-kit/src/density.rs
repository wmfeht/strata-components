//! Frame and Content density, and the bar and row sizes they set with the text size.
//!
//! Work package: K2, `docs/work/phase-1-leaves/K2-shape-density-type.md`.
//!
//! Values are Strata's, read at `lgse/strata` `upstream/main` 35ee677ab:
//! `src/ui/browser_modes/density.rs` (the scales and the Custom interpolation),
//! `src/ui/theme/generated_css.rs` (the frame inset and the text-scaled bars), and
//! `src/style.css` (`.file-row` and `.list-row` at 26 px and 4 px margins, 42 px and 8 px
//! when Airy; `.sidebar-row`; `.search-result`).
//!
//! # Two densities
//!
//! Frame density is one inset every bar takes above and below its content (the header, the
//! tab strip, the pane and preview headers, sidebar rows), so they grow together and stay
//! aligned. Content density is the space above and below a row's or a tile's content in the
//! file views. Each changes only its own tokens.
//!
//! # Row heights, and what rows must draw with
//!
//! A row is at least its density's minimum height (26 px at Compact, 42 px at Airy, linear
//! between and beyond), and at least its content plus the pad above and below it. The
//! content's height comes from two tokens, so **a row draws its label in
//! `kit.ty.ui_compact` and its icon at `icon_size.md`**: a taller style or icon than those
//! clips at large text sizes, because the row heights don't know about it. The sidebar row is
//! a bar action's square (`frame.control`) plus the frame inset. A search palette result is
//! a 32 px thumbnail box, or its name (`ui_compact`) and path (`small`) lines with a 2 px
//! gap, plus 10 px of padding above and below and a 1 px rule.
//!
//! Every size here is on whole points, so a virtual list of 100,000 rows accumulates no
//! fractions. Overrides (K4) of the type ramp come later and don't regrow rows.

use crate::look::{Content, Frame, Look, RowHeights};
use crate::typography;

#[cfg(test)]
mod tests;

/// A density slider's scale in px: its two snap points and its end. Every value from 0 to
/// `max` is allowed; one on a snap point is that preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Scale {
    pub compact: u8,
    pub airy: u8,
    pub max: u8,
}

/// A density's named snap points. Any other value is Custom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preset {
    Compact,
    Airy,
}

impl Scale {
    /// `px` clamped to the scale: a saved 200 loads as the end.
    pub fn clamp(&self, px: u8) -> u8 {
        px.min(self.max)
    }

    /// The preset `px` rests on, or `None` for a Custom value. Strata saves a slider resting
    /// on a snap point as that preset.
    pub fn preset(&self, px: u8) -> Option<Preset> {
        match self.clamp(px) {
            px if px == self.compact => Some(Preset::Compact),
            px if px == self.airy => Some(Preset::Airy),
            _ => None,
        }
    }
}

/// Frame density: the bars' extra inset, Compact 0 (the default), Airy 4, up to 12 px.
pub const FRAME: Scale = Scale {
    compact: 0,
    airy: 4,
    max: 12,
};

/// Content density: the space above and below a row's content, Compact 4 (the default),
/// Airy 8, up to 16 px.
pub const CONTENT: Scale = Scale {
    compact: 4,
    airy: 8,
    max: 16,
};

/// A file-view length at content pad `pad`, given its Compact and Airy values: Strata's
/// Custom spacing, linear through both presets so a slider never jumps at either, never
/// below 0, on whole points.
pub fn between(pad: f32, compact: f32, airy: f32) -> f32 {
    let (c, a) = (f32::from(CONTENT.compact), f32::from(CONTENT.airy));
    let step = (pad - c) / (a - c);
    (compact + (airy - compact) * step).max(0.0).round()
}

/// IconsGrid's spacing at content pad `pad`, for C3 to read or turn into slot tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconsSpacing {
    /// Around the whole grid.
    pub grid_pad: f32,
    /// Around each tile.
    pub child_margin: f32,
    /// Inside a tile's card, above and below.
    pub card_v: f32,
    /// Inside a tile's card, left and right.
    pub card_h: f32,
}

/// Strata's Icons spacing at content pad `pad`.
pub fn icons_spacing(pad: f32) -> IconsSpacing {
    IconsSpacing {
        grid_pad: between(pad, 0.0, 5.0),
        child_margin: between(pad, 1.0, 3.0),
        card_v: between(pad, 2.0, 5.0),
        card_h: between(pad, 4.0, 7.0),
    }
}

/// The bars at frame inset `inset` and text factor `factor`: a bar action's square scales
/// with the text, and a bar is that square plus 8, never under 36.
pub fn frame(inset: u8, factor: f32) -> Frame {
    let control = (24.0 * factor).round().max(1.0);
    Frame {
        inset: f32::from(FRAME.clamp(inset)),
        bar: (control + 8.0).max(36.0),
        control,
    }
}

/// The row heights for `look`'s ramp and icons (already at the text size), its frame, and
/// content pad `pad`.
pub fn rows(look: &Look, pad: f32) -> RowHeights {
    let label = typography::line_of(look.kit.ty.ui_compact);
    let path = typography::line_of(look.kit.ty.small);
    let content = label.max(look.icon_size.md);
    let file_row = between(pad, 26.0, 42.0).max(content + 2.0 * pad);
    let sidebar = look.frame.control.max(content) + 2.0 * look.frame.inset;
    let palette = 32.0_f32.max(label + 2.0 + path) + 21.0;
    RowHeights {
        list: file_row,
        // Columns rows follow the same rules in Strata (`.file-row` and `.list-row`).
        columns: file_row,
        sidebar,
        palette,
    }
}

/// The density step of [`Look::apply_params`]. Runs after the type step: rows are sized
/// from the scaled ramp and icons.
pub(crate) fn apply(look: &mut Look, frame_px: u8, content_px: u8, factor: f32) {
    look.frame = frame(frame_px, factor);
    let pad = f32::from(CONTENT.clamp(content_px));
    look.content = Content { pad };
    look.row = rows(look, pad);
}
