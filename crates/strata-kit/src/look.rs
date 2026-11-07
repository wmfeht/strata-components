//! `Look`: the `rune_kit::Kit` plus Strata's extension tokens, the one surface components read
//! their look from.
//!
//! Work package: W2, `docs/work/phase-0-groundwork/W2-look-contract.md`.
//!
//! # Building a look
//!
//! [`Look::fallback`] is the look before any Strata theme exists. The real one is built in
//! this order, each step changing the previous step's `Look` in place:
//!
//! 1. the theme (K1, `theme/`): `kit`, `syntax` and `glow`;
//! 2. the user's shape, density and type settings (K2, `shape.rs`, `density.rs`,
//!    `typography.rs`): `kit.radius` and `roundness`, `kit.ty` and `ty`, `space`, `frame`,
//!    `content`, `rows`, `icon_size`, `tiles` and `font`;
//! 3. the user's overrides (K4, `overrides/`), by name through [`crate::tokens`].
//!
//! The fallback values are Strata's own defaults at its default 13 px text size, so a
//! component built against them looks right when K1–K4 land. Only the goldens move.
//!
//! # Rules for components
//!
//! - Take the look as `&Look` in a builder, as rune-kit's builders take `&Kit`, and hand
//!   `&look.kit` to the rune-kit parts you use. Where a handler or a memo needs it later, take
//!   an accessor, `fn(&S) -> &Look`.
//! - Colours come from `kit.c` (or a slot token). Sizes and gaps come from `space`, `frame`,
//!   `rows` and `icon_size`. Radii come from `kit.radius` or [`Look::round`]. Never write a
//!   literal colour, and never a literal size where a token says it.
//! - Icons come from [`Look::icon`] by [`IconName`], or [`Look::icon_named`] for a catalog
//!   name (an entry's icon key). Tint them with Kit's icon roles; a multi-colour catalog's
//!   bodies name their own colours, and Rune draws those instead, so a component never needs
//!   to know which catalog is in use.
//! - Skip your own motion (flights, dissolves, open and close travel) when
//!   `look.animations` is false. The host also passes `!look.animations` to
//!   `Cx::reduced_motion` on every commit, so Rune's own motion follows.
//! - A component-specific token (a list row's height, say) is a slot: see [`crate::slots`].

use rune::{Color, Icon, TextStyle};
use rune_kit::Kit;
use rune_kit::syntax::SyntaxTheme;

use crate::icons::{self, IconName, IconSet};
use crate::slots::Slots;

#[cfg(test)]
mod tests;

/// The kit components draw with until a Strata theme is loaded: [`Look::fallback`]'s kit.
///
/// `lib.rs` re-exports this as `strata_kit::fallback`. Prefer [`Look::fallback`], which
/// carries Strata's extension tokens too.
pub fn fallback(dark: bool) -> Kit {
    Look::fallback(dark).kit
}

/// Everything a component reads its look from: rune-kit's tokens, plus the ones Strata adds.
///
/// Fields are public so the look's builders (K1, K2, K4) change them in place and components
/// read them. The struct is `non_exhaustive`: a new token is a new field, which breaks no
/// reader. Build one with [`Look::fallback`], never with a struct literal.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Look {
    /// rune-kit's tokens: colours, elevation, glyphs, the type ramp, radii and motion. K2
    /// scales `kit.ty` by the text size and `kit.radius` by the roundness, so rune-kit's own
    /// components follow both.
    pub kit: Kit,
    /// The type styles Kit's ramp lacks. Scaled by the text size with `kit.ty`.
    pub ty: Ramp,
    /// Gaps and padding.
    pub space: Space,
    /// The window's bars: header, tab strip, sidebar rows, column and preview headers.
    pub frame: Frame,
    /// Spacing inside the file views.
    pub content: Content,
    /// Row height per view.
    pub rows: Rows,
    /// Icon sizes.
    pub icon_size: IconSizes,
    /// The tile sizes IconsGrid steps through (C3): Ctrl+Shift+wheel and the size shortcuts.
    pub tiles: Ladder,
    /// The thumbnail size classes (C25): one rendered image per class, not per tile size.
    pub thumbnails: ThumbnailSteps,
    /// Corner roundness as a factor: 1.0 is 100 %. K2 has already applied it to
    /// `kit.radius`; [`Look::round`] applies it to a radius Kit doesn't have.
    pub roundness: f32,
    /// The element glow (Strata's "element glow" setting and its theme colour).
    pub glow: Glow,
    /// Syntax colours for source previews and code blocks (C10a, C11), per token kind.
    pub syntax: SyntaxTheme,
    /// The icon catalog behind [`IconName`].
    pub icons: IconSet,
    /// The interface and code fonts. `None` keeps the app's own (`AppSpec::families`). The
    /// host passes a change to `Tx::set_font_families`; components never read this.
    pub font: Option<Families>,
    /// False when the user or the desktop turned animations off (P5's switch, as data).
    pub animations: bool,
    /// Component slots: tokens one component reads, which default to the tokens above.
    pub slots: Slots,
}

impl Look {
    /// Strata's look before a theme is loaded: rune-kit's neutral kit, with Strata's default
    /// shape, density and text size.
    pub fn fallback(dark: bool) -> Look {
        let kit = Kit::neutral(dark);
        Look {
            ty: Ramp::fallback(&kit),
            space: Space {
                xxs: 2.0,
                xs: 4.0,
                sm: 6.0,
                md: 8.0,
                lg: 12.0,
                xl: 16.0,
                xxl: 24.0,
            },
            frame: Frame {
                inset: 0.0,
                bar: 40.0,
                control: 24.0,
            },
            content: Content { pad: 4.0 },
            rows: Rows {
                list: 26.0,
                columns: 26.0,
                sidebar: 28.0,
                palette: 32.0,
            },
            icon_size: IconSizes {
                sm: 12.0,
                md: 16.0,
                lg: 20.0,
            },
            tiles: Ladder::strata_tiles(),
            thumbnails: ThumbnailSteps {
                sizes: vec![128.0, 256.0],
            },
            roundness: 1.0,
            glow: Glow {
                on: true,
                color: kit.c.text_accent,
                radius: 14.0,
                intensity: 1.0,
            },
            syntax: SyntaxTheme::from_kit(&kit),
            icons: icons::lucide::SET,
            font: None,
            animations: true,
            slots: Slots::default(),
            kit,
        }
    }

    /// The icon for `name` in the current catalog.
    pub fn icon(&self, name: IconName) -> &'static Icon {
        (self.icons.icon)(name)
    }

    /// The icon a catalog calls `key`, such as an entry's icon key from Strata's file-name
    /// mapping, or `None` when the catalog has none by that name. Draw
    /// [`IconName::File`] then.
    pub fn icon_named(&self, key: &str) -> Option<&'static Icon> {
        (self.icons.named)(key)
    }

    /// `base`, a radius in points that Kit has no token for, at the current roundness.
    pub fn round(&self, base: f32) -> f32 {
        base * self.roundness
    }
}

/// The light fallback, so a state holding a `Look` can derive `Default`.
impl Default for Look {
    fn default() -> Look {
        Look::fallback(false)
    }
}

/// The type styles Strata adds to Kit's ramp (`kit.ty`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Ramp {
    /// Dialog and panel titles.
    pub title: TextStyle,
    /// Section headings over rows (PINNED, DEVICES), and group headers.
    pub heading: TextStyle,
}

impl Ramp {
    fn fallback(kit: &Kit) -> Ramp {
        Ramp {
            title: TextStyle {
                size: 15.0,
                weight: 600.0,
                ..kit.ty.ui
            }
            .lh(22.0),
            heading: kit.ty.small_medium.lh(16.0),
        }
    }
}

/// A spacing ramp in points, smallest first.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Space {
    pub xxs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

/// The window's bars, which Frame density grows together so they stay aligned.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Frame {
    /// Frame density: the extra padding above and below every bar's content, and before
    /// each bar action, in points. 0 is Strata's Compact.
    pub inset: f32,
    /// A bar's minimum height before the inset: the header, the tab strip and the preview
    /// header.
    pub bar: f32,
    /// A bar action's square size: header buttons, sidebar icons and device actions.
    pub control: f32,
}

/// Spacing inside the file views, which Content density sets.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Content {
    /// Content density: the space above and below a row's content (and a tile's), in
    /// points. 4 is Strata's Compact.
    pub pad: f32,
}

/// Row heights per view, in points, at the current density and text size. K2 folds in
/// whichever density grows each view: Content density for the file views, Frame density for
/// the sidebar.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Rows {
    /// ListTable rows (C4).
    pub list: f32,
    /// ColumnsView rows (C2).
    pub columns: f32,
    /// PlacesSidebar rows (C8).
    pub sidebar: f32,
    /// Result rows in the search palette and folder peek (C17, C22).
    pub palette: f32,
}

/// Icon sizes in points.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct IconSizes {
    /// Inline with small text: badges, trailing marks.
    pub sm: f32,
    /// Rows, bar actions and buttons.
    pub md: f32,
    /// Emphasis: empty states, dialog status icons.
    pub lg: f32,
}

/// The element glow, as plain data. Where it is drawn, the component converts it to
/// rune-fx's glow parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Glow {
    /// Strata's "element glow" setting.
    pub on: bool,
    pub color: Color,
    /// The glow's reach, in points.
    pub radius: f32,
    /// 1.0 is full strength.
    pub intensity: f32,
}

/// The interface and code font families, as `AppSpec::families` takes them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Families {
    pub ui: String,
    pub mono: String,
}

impl Families {
    pub fn new(ui: impl Into<String>, mono: impl Into<String>) -> Families {
        Families {
            ui: ui.into(),
            mono: mono.into(),
        }
    }
}

/// Sizes to step through, smallest first, with the size a reset returns to.
#[derive(Clone, Debug, PartialEq)]
pub struct Ladder {
    steps: Vec<f32>,
    default: f32,
}

impl Ladder {
    /// A ladder over `steps`, or `None` unless they are finite and strictly increasing,
    /// with at least one step, and `default` is one of them.
    pub fn new(steps: Vec<f32>, default: f32) -> Option<Ladder> {
        let increasing = steps.windows(2).all(|w| w[0] < w[1]);
        let finite = steps.iter().all(|s| s.is_finite());
        (increasing && finite && steps.contains(&default)).then_some(Ladder { steps, default })
    }

    /// Strata's tile sizes: finer where one step is a larger share of the size.
    fn strata_tiles() -> Ladder {
        let steps = [
            32.0, 36.0, 40.0, 44.0, 48.0, 56.0, 64.0, 72.0, 80.0, 96.0, 112.0, 128.0, 144.0, 160.0,
            192.0, 224.0, 256.0,
        ];
        Ladder {
            steps: steps.to_vec(),
            default: 64.0,
        }
    }

    pub fn steps(&self) -> &[f32] {
        &self.steps
    }

    pub fn default(&self) -> f32 {
        self.default
    }

    pub fn min(&self) -> f32 {
        self.steps[0]
    }

    pub fn max(&self) -> f32 {
        self.steps[self.steps.len() - 1]
    }

    /// `from` moved `delta` steps, clamped to the ladder's ends. From a size between two
    /// steps, the first step lands on the nearer one in the direction moved. A `delta` of 0
    /// returns `from` unchanged.
    pub fn step(&self, from: f32, delta: i32) -> f32 {
        let last = self.steps.len() as i64 - 1;
        let index = match delta.signum() {
            0 => return from,
            1 => self.steps.partition_point(|&s| s <= from) as i64 - 1 + i64::from(delta),
            _ => self.steps.partition_point(|&s| s < from) as i64 + i64::from(delta),
        };
        self.steps[index.clamp(0, last) as usize]
    }
}

/// Thumbnail size classes in pixels, smallest first. A view asks for the class that covers
/// its tile, so thumbnails are rendered once per class, not once per tile size.
#[derive(Clone, Debug, PartialEq)]
pub struct ThumbnailSteps {
    sizes: Vec<f32>,
}

impl ThumbnailSteps {
    /// Classes over `sizes`, or `None` unless they are finite, positive and strictly
    /// increasing, with at least one.
    pub fn new(sizes: Vec<f32>) -> Option<ThumbnailSteps> {
        let increasing = sizes.windows(2).all(|w| w[0] < w[1]);
        let valid = sizes.iter().all(|s| s.is_finite() && *s > 0.0);
        (increasing && valid && !sizes.is_empty()).then_some(ThumbnailSteps { sizes })
    }

    pub fn sizes(&self) -> &[f32] {
        &self.sizes
    }

    /// The class for a tile `px` device pixels across: the smallest class at least that
    /// large, else the largest.
    pub fn for_size(&self, px: f32) -> usize {
        self.sizes
            .partition_point(|&s| s < px)
            .min(self.sizes.len() - 1)
    }
}
