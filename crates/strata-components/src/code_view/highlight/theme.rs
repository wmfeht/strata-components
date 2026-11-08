//! The syntect theme built from the Look's syntax colours, used until K1 supplies real ones.

use std::str::FromStr;

use rune::Color;
use rune_kit::syntax::Syntax;
use strata_kit::look::Look;
use syntect::highlighting::{ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings};

use super::palette::to_syntect;

/// What a selector colours: one of rune-kit's token kinds, or Strata's preprocessor colour.
#[derive(Clone, Copy)]
enum Ink {
    Kind(Syntax),
    Preprocessor,
}

/// TextMate scope selectors for each colour, broadest first: syntect scores a match on a
/// deeper scope higher, so a selector later in the list only wins a tie.
const SELECTORS: &[(&str, Ink)] = &[
    (
        "comment, punctuation.definition.comment",
        Ink::Kind(Syntax::Comment),
    ),
    (
        "string, constant.character, string.regexp, markup.raw",
        Ink::Kind(Syntax::String),
    ),
    (
        "constant.numeric, constant.language, constant.character.escape",
        Ink::Kind(Syntax::Number),
    ),
    (
        "keyword, storage, keyword.operator.word, entity.name.tag, markup.heading, \
         entity.name.section",
        Ink::Kind(Syntax::Keyword),
    ),
    (
        "keyword.operator, punctuation",
        Ink::Kind(Syntax::Punctuation),
    ),
    (
        "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
         entity.name.trait, entity.name.union, entity.name.impl, support.type, support.class, \
         storage.type.primitive",
        Ink::Kind(Syntax::Type),
    ),
    (
        "entity.name.function, support.function, variable.function, meta.function-call.identifier, \
         markup.underline.link",
        Ink::Kind(Syntax::Function),
    ),
    (
        "entity.other.attribute-name, support.type.property-name, meta.mapping.key, \
         entity.name.tag.yaml, variable.other.member, variable.other.readwrite.shell, \
         variable.other.normal.shell, punctuation.definition.variable.shell",
        Ink::Kind(Syntax::Property),
    ),
    ("variable.parameter", Ink::Kind(Syntax::Parameter)),
    ("markup.inserted", Ink::Kind(Syntax::Inserted)),
    ("markup.deleted", Ink::Kind(Syntax::Deleted)),
    (
        "meta.preprocessor keyword, meta.preprocessor punctuation.definition, \
         keyword.control.import.include, keyword.directive, meta.annotation, meta.attribute, \
         punctuation.definition.annotation",
        Ink::Preprocessor,
    ),
];

/// A syntect theme from `look.syntax` and `look.syntax_extra`, with the look's text colour as
/// its default foreground. Tokens left in the default foreground get no span, so they draw
/// in the host's own text colour.
///
/// This is the interim default. K1 builds a theme per Strata syntax palette and may move this
/// scope mapping into strata-kit, beside the palettes it reads.
pub fn theme_from_look(look: &Look) -> Theme {
    let color = |ink: Ink| -> Color {
        match ink {
            Ink::Kind(k) => look.syntax.color(k),
            Ink::Preprocessor => look.syntax_extra.preprocessor,
        }
    };
    let scopes = SELECTORS
        .iter()
        .filter_map(|&(sel, ink)| {
            Some(ThemeItem {
                scope: ScopeSelectors::from_str(sel).ok()?,
                style: StyleModifier {
                    foreground: Some(to_syntect(color(ink))),
                    background: None,
                    font_style: None,
                },
            })
        })
        .collect();
    Theme {
        name: Some("Strata look".into()),
        author: None,
        settings: ThemeSettings {
            foreground: Some(to_syntect(look.kit.c.text_base)),
            ..ThemeSettings::default()
        },
        scopes,
    }
}
