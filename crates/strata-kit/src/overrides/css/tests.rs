use super::*;
use crate::overrides::Overrides;
use crate::shape::LookParams;
use crate::slots;
use crate::theme::bundled_theme;
use crate::tokens::Kind;

/// A user's stylesheet with rules that convert and rules that don't.
const STYLE: &str = "/* My Strata tweaks */
.file-list row {
  margin: 1px 8px;
  border-radius: 3px;
}

.sidebar-row:hover,
.sidebar-device:hover {
  background: #ff000020;
}

.sidebar-row.active { background-color: rgba(0, 128, 255, 0.3); color: @strata_text; }
.sidebar-row.active image { color: hsl(30, 100%, 50%); }

.sidebar-shell {
  border-radius: 14px;
  background: alpha(@strata_surface, 0.6);
}

.file-list row { border-radius: 1em; }

@define-color strata_accent #ff0000;

@media (min-width: 600px) { .file-list row { margin: 0; } }
";

fn listed(m: &Migration) -> Vec<String> {
    m.unconverted.iter().map(ToString::to_string).collect()
}

/// The WP's test: a CSS migration fixture with converted and unconverted rules.
#[test]
fn a_stylesheet_converts_what_a_token_stands_for_and_lists_the_rest() {
    let m = migrate_css(STYLE);
    assert_eq!(
        m.toml,
        "# Converted from a user stylesheet (style.css): each row names the line it came from.

[collection.row]
inset = 8 # style.css line 3
radius = 3 # style.css line 4

[sidebar.selected]
fill = \"#0080ff4d\" # style.css line 12
icon = \"#ff8000\" # style.css line 13

[sidebar.hover]
fill = \"#ff000020\" # style.css line 9
"
    );
    assert_eq!(m.converted, 5);
    assert_eq!(
        listed(&m),
        [
            "line 3, column 3: `.file-list row` { margin: 1px 8px }: the top and bottom margins have no token; the sides converted",
            "line 8, column 1: `.sidebar-device:hover`: no token stands for this selector",
            "line 12, column 65: `.sidebar-row.active` { color: @strata_text }: no token stands for this property here",
            "line 15, column 1: `.sidebar-shell`: no token stands for this selector",
            "line 20, column 18: `.file-list row` { border-radius: 1em }: the value isn't a plain colour or px length",
            "line 22, column 1: `@define-color strata_accent #ff0000`: it redefines a theme colour: make a custom theme instead",
            "line 24, column 1: `@media (min-width: 600px)`: at-rules don't convert",
        ]
    );

    // What it wrote is an override file that applies with no errors.
    let o = Overrides::parse(&m.toml);
    assert_eq!(o.errors(), &[]);
    let look = o.applied(
        bundled_theme("tokyo-night")
            .expect("bundled")
            .look()
            .with_params(&LookParams::default()),
    );
    assert_eq!(slots::collection::row_inset(&look), 8.0);
    assert_eq!(slots::collection::row_radius(&look), 3.0);
    let rgba = |s: &str| Rgba8::parse(s).expect("a colour").color();
    assert_eq!(slots::sidebar::hover_fill(&look), rgba("#ff000020"));
    assert_eq!(
        slots::sidebar::selected_fill(&look),
        rgba("rgba(0, 128, 255, 0.3)")
    );
    assert_eq!(slots::sidebar::selected_icon(&look), rgba("#ff8000"));
}

#[test]
fn a_later_rule_for_the_same_token_wins() {
    let m = migrate_css(
        ".file-list row { margin: 0 4px; }\n.file-list row { margin-left: 6px; margin-right: 6px; }\n",
    );
    assert_eq!(m.converted, 2);
    assert_eq!(m.unconverted, []);
    assert!(
        m.toml.contains("inset = 6 # style.css line 2\n"),
        "{}",
        m.toml
    );
    assert!(!m.toml.contains("inset = 4"), "{}", m.toml);
}

#[test]
fn margins_convert_only_when_both_sides_match() {
    for (css, converts) in [
        (
            ".file-list row { margin-left: 7px; margin-right: 7px; }",
            true,
        ),
        (".file-list row { margin: 0 3px 0 3px; }", true),
        (".file-list row { margin: 0; }", true),
        (".file-list row { margin-left: 7px; }", false),
        (".file-list row { margin: 0 4px 0 6px; }", false),
        (
            ".file-list row { margin-left: 7px; margin-right: 8px; }",
            false,
        ),
    ] {
        let m = migrate_css(css);
        assert_eq!(m.converted == 1, converts, "{css}: {:?}", listed(&m));
        let sides = m.unconverted.iter().any(|u| u.reason == Reason::Sides);
        assert_eq!(sides, !converts, "{css}: {:?}", listed(&m));
    }
}

#[test]
fn values_that_arent_plain_dont_convert() {
    for value in [
        "1em",
        "50%",
        "-2px",
        "6px / 3px",
        "6px !important",
        "calc(2px + 1px)",
    ] {
        let m = migrate_css(&format!(".file-list row {{ border-radius: {value}; }}"));
        assert_eq!(m.converted, 0, "{value}");
        assert_eq!(m.unconverted[0].reason, Reason::Value, "{value}");
    }
    let m = migrate_css(".file-list row { border-radius: 2px 4px; }");
    assert_eq!(m.unconverted[0].reason, Reason::Sides);
    for value in [
        "shade(@strata_bg, 0.9)",
        "mix(red, blue, 0.5)",
        "none",
        "red !important",
    ] {
        let m = migrate_css(&format!(".sidebar-row:hover {{ background: {value}; }}"));
        assert_eq!(m.converted, 0, "{value}");
        assert_eq!(m.unconverted[0].reason, Reason::Value, "{value}");
    }
}

#[test]
fn broken_css_is_listed_and_the_rest_converts() {
    let m = migrate_css(
        "}\n.sidebar-row:hover { background red; }\n.sidebar-row.active image { color: #123456; }\n.file-list row { margin: 0 2px;",
    );
    assert_eq!(m.converted, 1, "{:?}", listed(&m));
    let reasons: Vec<_> = m
        .unconverted
        .iter()
        .map(|u| (u.at.line, u.reason))
        .collect();
    assert_eq!(
        reasons,
        [
            (1, Reason::Syntax),
            (2, Reason::Syntax),
            (4, Reason::Syntax)
        ]
    );
}

#[test]
fn a_stylesheet_with_nothing_to_convert_writes_no_file() {
    let m = migrate_css("label { color: red; }\n/* .file-list row { margin: 0 9px; } */\n");
    assert_eq!(m.toml, "");
    assert_eq!(m.converted, 0);
    assert_eq!(m.unconverted.len(), 1);
}

#[test]
fn every_rule_sets_a_registry_token_of_its_kind() {
    for &(selector, prop, name) in RULES {
        let token = tokens::find(name).unwrap_or_else(|| panic!("{name} is a token"));
        let kind = match prop {
            Prop::Background | Prop::Color => Kind::Color,
            Prop::Radius | Prop::Sides => Kind::Length,
        };
        assert_eq!(token.kind(), kind, "{selector} → {name}");
    }
}
