use rune_kit::syntax::Syntax;

use crate::theme::color::Rgba8;
use crate::theme::colors::SourceColors;
use crate::theme::{bundled_theme, parse_custom};

const NO_SYNTAX: &str = r##"name = "Plain"
background = "#0c1a2b"
surface = "#122438"
text = "#c9deed"
accent = "#4fd6ff"
muted = "#1e3a52"
highlight = "#244d68"
border = "#315b75"
dim_text = "#6f8da3"
"##;

fn kinds(look: &crate::look::Look) -> [rune::Color; 5] {
    [
        look.syntax.color(Syntax::Keyword),
        look.syntax.color(Syntax::String),
        look.syntax.color(Syntax::Number),
        look.syntax.color(Syntax::Type),
        look.syntax_extra.preprocessor,
    ]
}

/// settings/themes: "In a custom file, each `syntax_*` key overrides one role; an omitted key
/// keeps its color derived from accent and text."
#[test]
fn an_omitted_syntax_colour_follows_the_accent_and_a_set_one_holds() {
    let plain = parse_custom("plain", NO_SYNTAX).expect("loads");
    assert_eq!(
        plain.look().syntax.color(Syntax::Keyword),
        plain.colors.accent.color(),
        "keywords take the accent itself"
    );

    let mut other_accent = plain.clone();
    other_accent.colors.accent = Rgba8::rgb(0xff, 0x7b, 0x72);
    let (a, b) = (kinds(&plain.look()), kinds(&other_accent.look()));
    assert!(
        a.iter().zip(&b).all(|(x, y)| x != y),
        "every derived colour follows the accent"
    );

    let src = format!("{NO_SYNTAX}syntax_string = \"#7ee787\"\n");
    let one = parse_custom("one", &src).expect("loads");
    let (base, set) = (kinds(&plain.look()), kinds(&one.look()));
    assert_eq!(set[1], Rgba8::rgb(0x7e, 0xe7, 0x87).color());
    for i in [0, 2, 3, 4] {
        assert_eq!(base[i], set[i], "only the string colour moves");
    }
}

/// settings/themes: "Comments use the dim text color"; functions and the rest draw as text,
/// as GtkSourceView's identifier style does.
#[test]
fn comments_take_dim_text_and_unstyled_kinds_take_text() {
    let t = bundled_theme("gruvbox-dark-hard").expect("bundled");
    let look = t.look();
    assert_eq!(
        look.syntax.color(Syntax::Comment),
        t.colors.dim_text.color()
    );
    for kind in [
        Syntax::Function,
        Syntax::Property,
        Syntax::Parameter,
        Syntax::Punctuation,
    ] {
        assert_eq!(look.syntax.color(kind), t.colors.text.color(), "{kind:?}");
    }
    assert_eq!(look.syntax.color(Syntax::Deleted), t.colors.danger.color());
}

#[test]
fn a_theme_with_every_syntax_colour_uses_them_as_written() {
    let t = bundled_theme("catppuccin-mocha").expect("bundled");
    let SourceColors {
        keyword,
        string,
        constant,
        type_,
        preprocessor,
    } = t.colors.syntax;
    let written = [keyword, string, constant, type_, preprocessor].map(|c| c.expect("set").color());
    assert_eq!(kinds(&t.look()), written);
}
