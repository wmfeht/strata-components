//! Syntax colours: Strata's source palette, and the rune-kit [`SyntaxTheme`] it becomes.
//!
//! Strata colours code with a GtkSourceView scheme built from five theme colours plus the
//! dim text (`src/ui/theme/source_style.rs` at lgse/strata 35ee677ab). A theme that leaves a
//! syntax colour out gets one blended from its accent and text, as Strata's
//! `resolved_source_palette` does.
//!
//! # Kinds
//!
//! GtkSourceView's `def.lang` maps functions and built-ins onto identifiers, which Strata's
//! scheme draws in the text colour, and styles nothing else. So rune-kit's kinds map as:
//!
//! | rune-kit kind | Colour |
//! | --- | --- |
//! | `Keyword` | keyword (`def:statement`) |
//! | `String` | string |
//! | `Number` | constant (`def:constant`, numbers and special characters) |
//! | `Type` | type |
//! | `Comment` | dim text |
//! | `Function`, `Property`, `Parameter`, `Punctuation` | text |
//! | `Inserted`, `Deleted` | the kit's success and danger colours |
//!
//! Deliberate changes from Strata (owner rulings Q9):
//!
//! - Strata's scheme draws keywords and types bold, comments italic and the shebang bold.
//!   rune-kit highlights change colour only, never weight or style, so those are dropped.
//! - Strata's scheme leaves diff lines unstyled; rune-kit's diff kinds keep green and red.

use rune::Color;
use rune_kit::Kit;
use rune_kit::syntax::{Syntax, SyntaxTheme};

use super::color::{Rgba8, blend};
use super::colors::Colors;
use crate::look::SyntaxExtra;

#[cfg(test)]
mod tests;

/// The five colours code previews draw from, every one resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Palette {
    /// Keywords and operators (`def:statement`).
    pub statement: Rgba8,
    pub string: Rgba8,
    /// Numbers, booleans, special constants and escapes.
    pub constant: Rgba8,
    pub type_: Rgba8,
    pub preprocessor: Rgba8,
}

/// The theme's syntax colours, each left-out one blended from the accent toward the text.
pub fn resolve(colors: &Colors) -> Palette {
    let (accent, text) = (colors.accent, colors.text);
    let s = &colors.syntax;
    Palette {
        statement: s.keyword.unwrap_or(accent),
        string: s.string.unwrap_or_else(|| blend(accent, text, 0.48)),
        constant: s.constant.unwrap_or_else(|| blend(accent, text, 0.18)),
        type_: s.type_.unwrap_or_else(|| blend(accent, text, 0.24)),
        preprocessor: s.preprocessor.unwrap_or_else(|| blend(accent, text, 0.32)),
    }
}

/// The rune-kit syntax theme and Strata's extra kinds for `colors`, on the theme's `kit`.
pub(crate) fn themes(kit: &Kit, colors: &Colors) -> (SyntaxTheme, SyntaxExtra) {
    let p = resolve(colors);
    let text: Color = colors.text.color();
    let theme = SyntaxTheme::from_kit(kit)
        .with(Syntax::Keyword, p.statement.color())
        .with(Syntax::String, p.string.color())
        .with(Syntax::Number, p.constant.color())
        .with(Syntax::Type, p.type_.color())
        .with(Syntax::Comment, colors.dim_text.color())
        .with(Syntax::Function, text)
        .with(Syntax::Property, text)
        .with(Syntax::Parameter, text)
        .with(Syntax::Punctuation, text);
    let extra = SyntaxExtra {
        preprocessor: p.preprocessor.color(),
    };
    (theme, extra)
}
