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
//!
//! # Scopes
//!
//! [`SCOPES`] maps TextMate scope selectors, which syntect's grammars emit, onto a [`Role`],
//! following GtkSourceView's `def:` groups, and [`Role::color`] reads each role's colour from
//! a finished [`Look`], so a user's `syntax.*` override reaches it. They are plain data: the
//! syntect `Theme` built from them comes in a follow-up (K1b), after C10a's highlighter lands.
//! [`editor`] gives the colours around the code.

use rune::Color;
use rune_kit::Kit;
use rune_kit::syntax::{Syntax, SyntaxTheme};

use super::color::{Rgba8, blend};
use super::colors::Colors;
use crate::look::{Look, SyntaxExtra};

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
    resolve_over(colors, None)
}

/// [`resolve`], with left-out colours taken from `fallback` when there is one (an Omarchy
/// theme's terminal palette), else blended.
pub(crate) fn resolve_over(colors: &Colors, fallback: Option<Palette>) -> Palette {
    let (accent, text) = (colors.accent, colors.text);
    let f = fallback.unwrap_or(Palette {
        statement: accent,
        string: blend(accent, text, 0.48),
        constant: blend(accent, text, 0.18),
        type_: blend(accent, text, 0.24),
        preprocessor: blend(accent, text, 0.32),
    });
    let s = &colors.syntax;
    Palette {
        statement: s.keyword.unwrap_or(f.statement),
        string: s.string.unwrap_or(f.string),
        constant: s.constant.unwrap_or(f.constant),
        type_: s.type_.unwrap_or(f.type_),
        preprocessor: s.preprocessor.unwrap_or(f.preprocessor),
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

/// What colours a scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Unstyled code: identifiers, functions, built-ins (GtkSourceView's `def:identifier`).
    Text,
    Comment,
    String,
    /// Numbers, booleans, special constants and escapes.
    Constant,
    /// Keywords, storage modifiers and operators (`def:statement`).
    Keyword,
    Type,
    /// Directives, imports and attributes.
    Preprocessor,
    /// Markup headings, in the accent, as Strata's Markdown headings are.
    Heading,
    /// Invalid code, in the danger colour.
    Error,
}

impl Role {
    pub const ALL: [Role; 9] = [
        Role::Text,
        Role::Comment,
        Role::String,
        Role::Constant,
        Role::Keyword,
        Role::Type,
        Role::Preprocessor,
        Role::Heading,
        Role::Error,
    ];

    /// The role's colour in `look`, after the user's overrides.
    pub fn color(self, look: &Look) -> Color {
        let s = &look.syntax;
        match self {
            Role::Text => look.kit.c.text_base,
            Role::Comment => s.color(Syntax::Comment),
            Role::String => s.color(Syntax::String),
            Role::Constant => s.color(Syntax::Number),
            Role::Keyword => s.color(Syntax::Keyword),
            Role::Type => s.color(Syntax::Type),
            Role::Preprocessor => look.syntax_extra.preprocessor,
            Role::Heading => look.kit.c.text_accent,
            Role::Error => look.kit.c.danger.fg,
        }
    }
}

/// TextMate scope selectors and their roles. A scope not listed draws as [`Role::Text`];
/// the most specific match wins, as syntect matches them.
pub const SCOPES: &[(&str, Role)] = &[
    ("comment", Role::Comment),
    ("punctuation.definition.comment", Role::Comment),
    ("string", Role::String),
    ("markup.underline.link", Role::String),
    ("string.other.link", Role::String),
    ("constant", Role::Constant),
    ("constant.numeric", Role::Constant),
    ("constant.language", Role::Constant),
    ("constant.character", Role::Constant),
    ("constant.character.escape", Role::Constant),
    ("constant.other", Role::Constant),
    ("support.constant", Role::Constant),
    ("keyword", Role::Keyword),
    ("keyword.operator", Role::Keyword),
    ("storage", Role::Keyword),
    ("storage.modifier", Role::Keyword),
    ("storage.type", Role::Type),
    ("entity.name.type", Role::Type),
    ("entity.name.class", Role::Type),
    ("entity.other.inherited-class", Role::Type),
    ("support.type", Role::Type),
    ("support.class", Role::Type),
    ("meta.preprocessor", Role::Preprocessor),
    ("keyword.control.import", Role::Preprocessor),
    ("keyword.control.directive", Role::Preprocessor),
    ("punctuation.definition.directive", Role::Preprocessor),
    ("meta.attribute", Role::Preprocessor),
    ("meta.annotation", Role::Preprocessor),
    ("markup.heading", Role::Heading),
    ("entity.name.section", Role::Heading),
    ("invalid", Role::Error),
];

/// The colours around code, as Strata's source scheme sets them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Editor {
    pub foreground: Color,
    /// Code sits on the surface.
    pub background: Color,
    pub caret: Color,
    pub selection: Color,
    /// Selected text, on the selection.
    pub selection_foreground: Color,
    pub gutter: Color,
    pub gutter_background: Color,
    pub line_highlight: Color,
}

/// The colours around code in `look`: text on the surface, an accent caret and selection
/// with background-coloured text, dim line numbers and a background-coloured current line.
pub fn editor(look: &Look) -> Editor {
    let c = &look.kit.c;
    Editor {
        foreground: c.text_base,
        background: c.layer_01,
        caret: c.text_accent,
        selection: c.bg_accent,
        selection_foreground: c.text_inverse,
        gutter: c.text_muted,
        gutter_background: c.bg_base,
        line_highlight: c.bg_base,
    }
}
