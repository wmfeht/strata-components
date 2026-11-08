use super::*;
use crate::look::Look;
use crate::shape::LookParams;
use crate::slots;
use crate::theme::syntax::Role;
use crate::theme::{Rgba8, bundled_theme};
use crate::tokens::{self, Value};

/// A bundled theme's look at Strata's default settings: Tokyo Night when `dark`, Omarchy
/// Light otherwise.
fn theme_look(dark: bool) -> Look {
    themed(dark, &LookParams::default())
}

fn themed(dark: bool, params: &LookParams) -> Look {
    let id = if dark { "tokyo-night" } else { "omarchy-light" };
    let look = bundled_theme(id)
        .expect("bundled")
        .look()
        .with_params(params);
    assert_eq!(look.kit.dark, dark, "{id}");
    look
}

fn color(hex: &str) -> rune::Color {
    Rgba8::parse(hex).expect("a colour").color()
}

fn get(look: &Look, name: &str) -> Value {
    tokens::get(look, name).expect("a token")
}

fn at(line: usize, column: usize) -> Location {
    Location { line, column }
}

/// The file's problems as a user reads them.
fn messages(o: &Overrides) -> Vec<String> {
    o.errors().iter().map(ToString::to_string).collect()
}

/// Every registry token whose value differs between two looks.
fn changed(a: &Look, b: &Look) -> Vec<&'static str> {
    tokens::all()
        .filter(|t| t.get(a) != t.get(b))
        .map(|t| t.name())
        .collect()
}

#[test]
fn an_override_reaches_its_token_and_the_reader_its_component_calls() {
    let o = Overrides::parse(
        r##"
syntax.keyword = "#00ff00"

[sidebar.row]
height = 40

[sidebar.selected]
fill = "rgba(255, 0, 0, 0.5)"
"##,
    );
    assert_eq!(messages(&o), Vec::<String>::new());
    let look = o.applied(theme_look(true));
    assert_eq!(slots::sidebar::row_height(&look), 40.0);
    assert_eq!(
        slots::sidebar::selected_fill(&look),
        color("rgba(255, 0, 0, 0.5)")
    );
    // The syntect theme (K1b) reads its colours through the roles.
    assert_eq!(Role::Keyword.color(&look), color("#00ff00"));
    assert_eq!(o.len(), 3);
}

#[test]
fn a_core_override_moves_the_slot_tokens_that_inherit_it() {
    let look = Overrides::parse("row.list = 33").applied(theme_look(false));
    assert_eq!(slots::list::row_height(&look), 33.0);
    assert_eq!(get(&look, "list.row.height"), Value::Length(33.0));
}

/// The WP's test: a scoped override.
#[test]
fn a_scoped_override_applies_only_to_its_appearance_and_beats_an_unscoped_one() {
    // The scoped rows come first in the file, and still win.
    let o = Overrides::parse(
        r##"
dark.color.bg_base = "#111111"
color.bg_base = "#222222"
row.list = 30

[light.list.row]
height = 44
"##,
    );
    assert_eq!(messages(&o), Vec::<String>::new());
    let dark = o.applied(theme_look(true));
    let light = o.applied(theme_look(false));
    assert_eq!(dark.kit.c.bg_base, color("#111111"));
    assert_eq!(light.kit.c.bg_base, color("#222222"));
    assert_eq!(slots::list::row_height(&dark), 30.0);
    assert_eq!(slots::list::row_height(&light), 44.0);
}

/// The WP's test: an unknown token.
#[test]
fn an_unknown_token_is_reported_once_with_its_line_and_column() {
    let o = Overrides::parse(
        "row.list = 30\n  list.row.length = 30\n\n[list.roww]\nheight = 1\nwidth = 2\n",
    );
    assert_eq!(
        o.errors(),
        &[
            OverrideError {
                at: at(2, 3),
                scope: Scope::All,
                problem: Problem::Unknown("list.row.length".into()),
            },
            OverrideError {
                at: at(4, 1),
                scope: Scope::All,
                problem: Problem::Unknown("list.roww".into()),
            },
        ]
    );
    assert_eq!(
        messages(&o)[0],
        "line 2, column 3: unknown token `list.row.length`"
    );
    // The good row still applies, and rebuilding the look reports nothing more.
    let look = o.applied(o.applied(theme_look(true)));
    assert_eq!(look.row.list, 30.0);
    assert_eq!(o.errors().len(), 2);
}

#[test]
fn a_scoped_problem_names_its_scope() {
    let o = Overrides::parse("[dark]\ncolor.bg_basee = \"#000\"\n");
    assert_eq!(
        messages(&o),
        ["line 2, column 1: unknown token `color.bg_basee` (in [dark])"]
    );
}

#[test]
fn a_value_on_a_group_of_tokens_names_one_of_them() {
    let o = Overrides::parse("list.row = 30");
    assert_eq!(o.errors()[0].problem, Problem::Group("list.row".into()));
    assert_eq!(
        messages(&o),
        [
            "line 1, column 1: `list.row` names a group of tokens: set one of them, such as `list.row.height`"
        ]
    );
}

/// The WP's test: a bad value. settings/themes: "A rule GTK cannot parse, such as
/// `label { color: nope; }`, is skipped with an "invalid rule in user stylesheet" warning."
#[test]
fn a_bad_value_reports_where_it_is_and_keeps_the_themes_value() {
    let cases: &[(&str, &str, Problem)] = &[
        (
            "row.columns",
            "\"28px\"",
            Problem::Kind {
                name: "row.columns",
                expected: tokens::Kind::Length,
                found: "string",
            },
        ),
        (
            "collection.cut.opacity",
            "1.5",
            Problem::Range {
                name: "collection.cut.opacity",
                min: 0.0,
                max: 1.0,
            },
        ),
        (
            "color.text_base",
            "\"nope\"",
            Problem::BadColor {
                name: "color.text_base",
                value: "nope".into(),
            },
        ),
        (
            "glow.on",
            "1",
            Problem::Kind {
                name: "glow.on",
                expected: tokens::Kind::Flag,
                found: "integer",
            },
        ),
        (
            "color.bg_base",
            "0xff0000",
            Problem::Kind {
                name: "color.bg_base",
                expected: tokens::Kind::Color,
                found: "integer",
            },
        ),
        (
            "row.list",
            "-1",
            Problem::Range {
                name: "row.list",
                min: 0.0,
                max: f32::INFINITY,
            },
        ),
        (
            "row.list",
            "nan",
            Problem::Range {
                name: "row.list",
                min: 0.0,
                max: f32::INFINITY,
            },
        ),
        (
            "radius.md",
            "1e40",
            Problem::Range {
                name: "radius.md",
                min: 0.0,
                max: f32::INFINITY,
            },
        ),
    ];
    let base = theme_look(true);
    for (name, value, problem) in cases {
        let src = format!("space.md = 9\n{name} = {value}\n");
        let o = Overrides::parse(&src);
        let column = name.chars().count() + 4;
        assert_eq!(
            o.errors(),
            &[OverrideError {
                at: at(2, column),
                scope: Scope::All,
                problem: problem.clone(),
            }],
            "{src}"
        );
        let look = o.applied(base.clone());
        assert_eq!(get(&look, name), get(&base, name), "{src}");
        assert_eq!(look.space.md, 9.0, "{src}");
    }
    assert_eq!(
        messages(&Overrides::parse("row.columns = \"28px\"")),
        [
            "line 1, column 15: `row.columns` takes a length, not a string: write the number without quotes or units"
        ]
    );
}

/// The WP's test: partial apply. settings/themes: "Valid rules in the same file still
/// apply."
#[test]
fn rows_that_parse_still_apply_around_syntax_errors() {
    // Each file sets row.list = 30, row.columns = 28 and list.row.height = 40 around one
    // break, on the lines given.
    let cases: &[(&str, &str, &[usize])] = &[
        (
            "an unquoted value",
            "row.list = 30\ncolor.bg_base = #fff\nrow.columns = 28\n[list.row]\nheight = 40\n",
            &[2],
        ),
        (
            "a line of garbage",
            "row.list = 30\nthis is not toml\nrow.columns = 28\n[list.row]\nheight = 40\n",
            &[2],
        ),
        (
            "an unterminated string",
            "row.list = 30\ncolor.bg_base = \"#fff\nrow.columns = 28\n[list.row]\nheight = 40\n",
            &[2],
        ),
        (
            "a key set twice",
            "row.list = 30\nrow.list = 31\nrow.columns = 28\n[list.row]\nheight = 40\n",
            &[2],
        ),
        (
            "a table defined twice",
            "row.list = 30\nrow.columns = 28\n[list.row]\nheight = 40\n[list.row]\nheight = 41\n",
            &[5],
        ),
        (
            "a broken table header",
            "row.list = 30\nrow.columns = 28\n[sidebar.row\nheight = 99\n[list.row]\nheight = 40\n",
            &[3],
        ),
    ];
    for (what, src, lines) in cases {
        let o = Overrides::parse(src);
        let got: Vec<usize> = o.errors().iter().map(|e| e.at.line).collect();
        assert_eq!(&got, lines, "{what}: {:?}", messages(&o));
        assert!(
            o.errors()
                .iter()
                .all(|e| matches!(e.problem, Problem::Syntax(_))),
            "{what}: {:?}",
            messages(&o)
        );
        let look = o.applied(theme_look(false));
        assert_eq!(look.row.list, 30.0, "{what}");
        assert_eq!(look.row.columns, 28.0, "{what}");
        assert_eq!(slots::list::row_height(&look), 40.0, "{what}");
        assert_eq!(o.len(), 3, "{what}");
    }
}

#[test]
fn a_broken_table_header_skips_its_rows_and_says_so() {
    let base = theme_look(false);
    let o = Overrides::parse("[sidebar.row\nheight = 99\n");
    assert!(o.is_empty());
    assert!(
        messages(&o)[0].ends_with("; the rows under it don't apply"),
        "{:?}",
        messages(&o)
    );
    let look = o.applied(base.clone());
    assert_eq!(
        slots::sidebar::row_height(&look),
        slots::sidebar::row_height(&base)
    );
}

#[test]
fn a_string_left_open_at_the_end_applies_the_rows_before_it() {
    let o = Overrides::parse("row.list = 30\nrow.columns = \"\"\"\nrow.palette = 50\n");
    assert_eq!(o.errors().len(), 1, "{:?}", messages(&o));
    let base = theme_look(true);
    let look = o.applied(base.clone());
    assert_eq!(look.row.list, 30.0);
    assert_eq!(look.row.palette, base.row.palette);
}

/// Done when: "The token names in error messages match the registry."
#[test]
fn token_names_in_messages_match_the_registry() {
    for token in tokens::all() {
        let name = token.name();
        // An array is the wrong kind for every token.
        let o = Overrides::parse(&format!("{name} = [1]"));
        let m = messages(&o);
        assert_eq!(m.len(), 1, "{name}: {m:?}");
        assert!(m[0].contains(&format!("`{name}` takes ")), "{m:?}");
    }
}

/// settings/themes: "Without the file, Strata's appearance is unchanged." and "Deleting the
/// file, or making it unreadable, restores Strata's default appearance." The host hands in
/// empty bytes for a missing or unreadable file.
#[test]
fn an_empty_or_missing_file_leaves_the_look_as_the_theme_made_it() {
    for src in ["", "# only a comment\n\n", "[dark]\n[light]\n"] {
        let o = Overrides::from_bytes(src.as_bytes());
        assert!(o.is_empty() && o.errors().is_empty(), "{src:?}");
        assert_eq!(o, Overrides::default(), "{src:?}");
        for dark in [false, true] {
            let base = theme_look(dark);
            assert_eq!(changed(&o.applied(base.clone()), &base), Vec::<&str>::new());
        }
    }
}

/// settings/themes: "With `~/.config/strata/style.css` present at startup, its rules … override
/// Strata's own rules and the active theme." and "Corner roundness does not scale radii set in
/// the user stylesheet."
#[test]
fn an_override_is_final_over_the_theme_and_the_settings() {
    let o = Overrides::parse("radius.md = 4\nrow.list = 50\nglow.on = false\n");
    for roundness in [0, 60, 200] {
        let params = LookParams {
            roundness,
            text_size: 48,
            content_density: 16,
            element_glow: true,
            ..LookParams::default()
        };
        let base = themed(true, &params);
        let look = o.applied(base.clone());
        assert_eq!(look.kit.radius.md, 4.0, "{roundness}");
        assert_eq!(look.row.list, 50.0);
        // The Element glow setting is on; the override wins (ruling Q6).
        assert!(base.glow.on && !look.glow.on);
        // Only what the file names moves.
        let mut moved = changed(&look, &base);
        moved.sort_unstable();
        let mut expected = vec!["glow.on", "list.row.height", "radius.md", "row.list"];
        if base.kit.radius.md != 4.0 {
            // The collection rows' radius follows `radius.md` (slots inherit).
            expected.insert(0, "collection.row.radius");
        } else {
            expected.retain(|n| *n != "radius.md");
        }
        assert_eq!(moved, expected, "{roundness}");
    }
}

#[test]
fn light_and_dark_are_scopes_and_no_token_starts_with_them() {
    for token in tokens::all() {
        let first = token.name().split('.').next().unwrap_or_default();
        assert!(!matches!(first, "light" | "dark"), "{}", token.name());
    }
    let o = Overrides::parse("dark = 3\n[light.dark]\nrow.list = 30\n");
    let problems: Vec<_> = o.errors().iter().map(|e| e.problem.clone()).collect();
    assert_eq!(
        problems,
        [Problem::Scope("dark".into()), Problem::Scope("dark".into())]
    );
    assert_eq!(o.errors()[1].scope, Scope::Light);
    assert!(o.is_empty());
}

#[test]
fn a_file_that_is_not_utf8_applies_nothing() {
    let o = Overrides::from_bytes(b"row.list = 30\nrow.columns = \xff\n");
    assert!(o.is_empty());
    assert_eq!(
        o.errors(),
        &[OverrideError {
            at: at(2, 15),
            scope: Scope::All,
            problem: Problem::NotUtf8,
        }]
    );
}

#[test]
fn a_rewrite_that_changes_nothing_parses_equal() {
    let a = "row.list = 30\n[dark]\ncolor.bg_base = \"#111\"\n";
    assert_eq!(Overrides::parse(a), Overrides::parse(a));
    assert_ne!(
        Overrides::parse(a),
        Overrides::parse(&a.replace("30", "31"))
    );
}
