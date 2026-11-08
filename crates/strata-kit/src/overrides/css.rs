//! Converting a user stylesheet (`~/.config/strata/style.css`) into override rows.
//!
//! [`migrate_css`] is a pure function: Strata reads the stylesheet on first launch, writes
//! [`Migration::toml`] to `overrides.toml`, lists [`Migration::unconverted`] in a one-time
//! notice, and leaves the stylesheet in place.
//!
//! # What converts
//!
//! A declaration converts only where a token stands for exactly what it styled: a selector
//! that a registry token takes its default from, checked against Strata's `src/style.css`
//! (lgse/strata `upstream/main` 35ee677ab, line numbers there), and a plain value.
//!
//! | Selector | Property | Token | Strata's rule |
//! | --- | --- | --- | --- |
//! | `.file-list row` | `margin`, or `margin-left` with `margin-right`, equal sides | `collection.row.inset` | 5076: `margin-left: 5px; margin-right: 5px` |
//! | `.file-list row` | `border-radius`, equal corners | `collection.row.radius` | 5061: `border-radius: 6px` (`radius.md`) |
//! | `.sidebar-row:hover` | `background`, `background-color` | `sidebar.hover.fill` | 2373: `alpha(@strata_text, 0.06)` |
//! | `.sidebar-row.active` | `background`, `background-color` | `sidebar.selected.fill` | 2378: `alpha(@strata_accent, 0.18)` |
//! | `.sidebar-row.active image` | `color` | `sidebar.selected.icon` | 2386: `@strata_accent` |
//!
//! Lengths convert from `px` (or a bare `0`), colours from the forms a theme file takes
//! ([`Rgba8::parse`]). Within a selector list, each selector converts on its own.
//!
//! Some near matches are left out on purpose, because their token isn't the same thing:
//!
//! - `min-height` on `.file-row`, `.list-row` or `.sidebar-row`: a minimum, where `row.*` and
//!   the row slots are fixed heights that K2 grows with the text size;
//! - the `.file-row > *` and `.list-row > *` margins: Content density's pad, which sizes the
//!   rows when K2 builds the look, so an override after it wouldn't regrow them;
//! - `window { background }`: `color.bg_base` is the file columns' fill too;
//! - `.search-result` and `.browser-tab`: no slot holds their fills or padding yet.
//!
//! Everything else is listed as unconverted with its location and a [`Reason`]: other
//! selectors (including `.sidebar-shell` from Strata's own documentation), other properties,
//! relative units, `@strata_*` colours and `alpha()`, `shade()`, `mix()`, `!important`,
//! `@define-color` (a theme colour belongs in a custom theme) and every other at-rule.

use std::fmt;

use crate::theme::{Location, Rgba8};
use crate::tokens::{self, Token};

#[cfg(test)]
mod tests;

/// What [`migrate_css`] made of a stylesheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Migration {
    /// The override file: one row per token, in the registry's order, each with the line it
    /// came from. Empty when nothing converted.
    pub toml: String,
    /// How many declarations converted. A later one for the same token replaces the earlier
    /// row, as the later rule wins in CSS.
    pub converted: usize,
    /// Everything that didn't convert, in the order it appears.
    pub unconverted: Vec<Unconverted>,
}

/// A rule or declaration [`migrate_css`] left alone.
#[derive(Clone, Debug, PartialEq)]
pub struct Unconverted {
    /// Where it starts in the stylesheet.
    pub at: Location,
    /// The selector, or the at-rule's prelude, as written (white space collapsed).
    pub selector: String,
    /// The declaration, when only it didn't convert: `color: red`.
    pub declaration: Option<String>,
    pub reason: Reason,
}

/// Why a rule or declaration didn't convert.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reason {
    /// No token stands for anything this selector styles.
    Selector,
    /// The selector has tokens, but none for this property.
    Property,
    /// The value isn't a plain colour or `px` length.
    Value,
    /// The sides or corners differ, or only one side is set, and one token sets them all.
    Sides,
    /// A margin's top or bottom: only its sides have a token, and those converted.
    Vertical,
    /// `@define-color` redefines a theme colour.
    DefineColor,
    /// Another at-rule (`@media`, `@import`, …).
    AtRule,
    /// Not valid CSS: an unclosed block, a declaration without a colon, a stray brace.
    Syntax,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Reason::Selector => "no token stands for this selector",
            Reason::Property => "no token stands for this property here",
            Reason::Value => "the value isn't a plain colour or px length",
            Reason::Sides => "its sides differ, and one token sets them all",
            Reason::Vertical => "the top and bottom margins have no token; the sides converted",
            Reason::DefineColor => "it redefines a theme colour: make a custom theme instead",
            Reason::AtRule => "at-rules don't convert",
            Reason::Syntax => "it isn't valid CSS",
        })
    }
}

impl fmt::Display for Unconverted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: `{}`",
            self.at.line, self.at.column, self.selector
        )?;
        if let Some(d) = &self.declaration {
            write!(f, " {{ {d} }}")?;
        }
        write!(f, ": {}", self.reason)
    }
}

/// The properties a selector's tokens take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prop {
    /// `background` or `background-color`: a colour.
    Background,
    /// `color`: a colour.
    Color,
    /// `border-radius`: one length for every corner.
    Radius,
    /// `margin`'s sides, or `margin-left` with `margin-right`: one length for both.
    Sides,
}

impl Prop {
    fn of(property: &str) -> Option<Prop> {
        Some(match property {
            "background" | "background-color" => Prop::Background,
            "color" => Prop::Color,
            "border-radius" => Prop::Radius,
            "margin" | "margin-left" | "margin-right" => Prop::Sides,
            _ => return None,
        })
    }
}

/// The selectors that convert: each with a property and the token it sets. See the table in
/// the module docs.
const RULES: &[(&str, Prop, &str)] = &[
    (".file-list row", Prop::Sides, "collection.row.inset"),
    (".file-list row", Prop::Radius, "collection.row.radius"),
    (".sidebar-row:hover", Prop::Background, "sidebar.hover.fill"),
    (
        ".sidebar-row.active",
        Prop::Background,
        "sidebar.selected.fill",
    ),
    (
        ".sidebar-row.active image",
        Prop::Color,
        "sidebar.selected.icon",
    ),
];

/// Converts what of `css` can be converted into override rows, and lists the rest.
pub fn migrate_css(css: &str) -> Migration {
    let text = strip_comments(css);
    let mut out = Out {
        css,
        rows: Vec::new(),
        converted: 0,
        unconverted: Vec::new(),
    };
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let Some(stop) = find(b, i, b"{;}") else {
            out.skip(i, collapse(&text[i..]), None, Reason::Syntax);
            break;
        };
        let prelude = collapse(&text[i..stop]);
        match b[stop] {
            b'{' => {
                let Some((close, nested)) = block_end(b, stop + 1) else {
                    out.skip(i, prelude, None, Reason::Syntax);
                    break;
                };
                if prelude.starts_with('@') {
                    out.skip(i, prelude, None, Reason::AtRule);
                } else if nested {
                    out.skip(i, prelude, None, Reason::Syntax);
                } else {
                    out.rule(&text, i..stop, stop + 1..close);
                }
                i = close + 1;
            }
            b';' => {
                let reason = if prelude.starts_with("@define-color") {
                    Reason::DefineColor
                } else if prelude.starts_with('@') {
                    Reason::AtRule
                } else {
                    Reason::Syntax
                };
                out.skip(i, prelude, None, reason);
                i = stop + 1;
            }
            _ => {
                out.skip(i, "}".into(), None, Reason::Syntax);
                i = stop + 1;
            }
        }
    }
    out.finish()
}

/// A converted row: the token, its value as TOML writes it, and the CSS line it came from.
struct Converted {
    token: &'static Token,
    value: String,
    line: usize,
}

struct Out<'a> {
    css: &'a str,
    rows: Vec<Converted>,
    converted: usize,
    unconverted: Vec<Unconverted>,
}

impl Out<'_> {
    fn skip(&mut self, at: usize, selector: String, declaration: Option<String>, reason: Reason) {
        self.unconverted.push(Unconverted {
            at: Location::of(self.css, at),
            selector,
            declaration,
            reason,
        });
    }

    fn set(&mut self, name: &'static str, value: String, at: usize) {
        let token = tokens::find(name).expect("RULES names registry tokens");
        let line = Location::of(self.css, at).line;
        self.converted += 1;
        self.rows.retain(|r| r.token.name() != name);
        self.rows.push(Converted { token, value, line });
    }

    /// One rule: `text[prelude]` is its selector list, `text[body]` its declarations.
    fn rule(&mut self, text: &str, prelude: std::ops::Range<usize>, body: std::ops::Range<usize>) {
        let decls = declarations(text, body);
        for (at, selector) in split(text, prelude, b',') {
            let rules: Vec<_> = RULES.iter().filter(|r| r.0 == selector).collect();
            if rules.is_empty() {
                self.skip(at, selector, None, Reason::Selector);
                continue;
            }
            // margin-left and margin-right convert together, once the rule has both.
            let mut sides: [Option<(f32, usize, String)>; 2] = [None, None];
            let mut sides_token = None;
            for (at, decl) in &decls {
                let Some((property, value)) = decl.split_once(':') else {
                    self.skip(*at, selector.clone(), Some(decl.clone()), Reason::Syntax);
                    continue;
                };
                let (property, value) = (property.trim().to_ascii_lowercase(), value.trim());
                let Some(&&(_, prop, token)) =
                    Prop::of(&property).and_then(|p| rules.iter().find(|r| r.1 == p))
                else {
                    self.skip(*at, selector.clone(), Some(decl.clone()), Reason::Property);
                    continue;
                };
                let unconverted = |reason| (*at, selector.clone(), Some(decl.clone()), reason);
                let problem = match prop {
                    Prop::Background | Prop::Color => match color(value) {
                        Some(c) => {
                            self.set(token, format!("{:?}", c.hex()), *at);
                            None
                        }
                        None => Some(unconverted(Reason::Value)),
                    },
                    Prop::Radius => match lengths(value).as_deref() {
                        Some([r, rest @ ..]) if rest.iter().all(|x| x == r) && rest.len() < 4 => {
                            self.set(token, number(*r), *at);
                            None
                        }
                        Some(_) => Some(unconverted(Reason::Sides)),
                        None => Some(unconverted(Reason::Value)),
                    },
                    Prop::Sides => {
                        sides_token = Some(token);
                        let l = lengths(value);
                        match (property.as_str(), l.as_deref()) {
                            (_, None) => Some(unconverted(Reason::Value)),
                            ("margin", Some(v)) => {
                                // CSS's one to four values: top, right, bottom, left.
                                let (top, right, bottom, left) = match *v {
                                    [a] => (a, a, a, a),
                                    [a, b] => (a, b, a, b),
                                    [a, b, c] => (a, b, c, b),
                                    [a, b, c, d] => (a, b, c, d),
                                    _ => (f32::NAN, 0.0, 0.0, 0.0),
                                };
                                if top.is_nan() {
                                    Some(unconverted(Reason::Value))
                                } else {
                                    sides = [
                                        Some((left, *at, decl.clone())),
                                        Some((right, *at, decl.clone())),
                                    ];
                                    (top != 0.0 || bottom != 0.0)
                                        .then(|| unconverted(Reason::Vertical))
                                }
                            }
                            (side, Some(&[x])) => {
                                let i = usize::from(side == "margin-right");
                                sides[i] = Some((x, *at, decl.clone()));
                                None
                            }
                            _ => Some(unconverted(Reason::Value)),
                        }
                    }
                };
                if let Some((at, selector, declaration, reason)) = problem {
                    self.skip(at, selector, declaration, reason);
                }
            }
            if let Some(token) = sides_token {
                match sides {
                    [Some((l, a, _)), Some((r, b, _))] if l == r => {
                        self.set(token, number(l), a.max(b));
                    }
                    [Some((_, at, decl)), _] | [None, Some((_, at, decl))] => {
                        self.skip(at, selector.clone(), Some(decl), Reason::Sides);
                    }
                    [None, None] => {}
                }
            }
        }
    }

    fn finish(mut self) -> Migration {
        self.unconverted.sort_by_key(|u| (u.at.line, u.at.column));
        let order = |t: &Token| tokens::all().position(|r| r.name() == t.name());
        self.rows.sort_by_key(|r| order(r.token));
        let mut toml = String::new();
        let mut table = "";
        for r in &self.rows {
            let name = r.token.name();
            let (prefix, key) = name.rsplit_once('.').unwrap_or(("", name));
            if toml.is_empty() {
                toml.push_str(
                    "# Converted from a user stylesheet (style.css): each row names the line it \
                     came from.\n",
                );
            }
            if prefix != table {
                toml.push_str(&format!("\n[{prefix}]\n"));
                table = prefix;
            }
            toml.push_str(&format!(
                "{key} = {} # style.css line {}\n",
                r.value, r.line
            ));
        }
        Migration {
            toml,
            converted: self.converted,
            unconverted: self.unconverted,
        }
    }
}

/// A colour value, as a theme file takes one.
fn color(value: &str) -> Option<Rgba8> {
    Rgba8::parse(value)
}

/// Every length in `value`, each `<n>px` or a bare `0`, none negative; `None` if anything
/// else is in it.
fn lengths(value: &str) -> Option<Vec<f32>> {
    let v: Option<Vec<f32>> = value
        .split_ascii_whitespace()
        .map(|part| {
            let n = match part.strip_suffix("px") {
                Some(n) => n.parse::<f32>().ok()?,
                None if part == "0" => 0.0,
                None => return None,
            };
            (n.is_finite() && n >= 0.0).then_some(n)
        })
        .collect();
    v.filter(|v| !v.is_empty())
}

/// A length as the override file writes it: `8`, `5.5`.
fn number(x: f32) -> String {
    format!("{x}")
}

/// `css` with every comment written over in spaces, keeping line breaks, so offsets still
/// point into the original.
fn strip_comments(css: &str) -> String {
    let mut b = css.as_bytes().to_vec();
    let mut i = 0;
    let mut quote = None;
    while i < b.len() {
        match (quote, b[i]) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), b'\\') => i += 1,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'/') if b.get(i + 1) == Some(&b'*') => {
                let end = css[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2);
                for c in &mut b[i..end] {
                    if *c != b'\n' {
                        *c = b' ';
                    }
                }
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    String::from_utf8(b).expect("spaces over whole comments keep UTF-8")
}

/// The first of `stops` at or after `from`, outside quotes and parentheses.
fn find(b: &[u8], from: usize, stops: &[u8]) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut i = from;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) if c == b'\\' => i += 1,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ if depth == 0 && stops.contains(&c) => return Some(i),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// The `}` closing a block whose body starts at `from`, and whether a block nests inside it.
fn block_end(b: &[u8], from: usize) -> Option<(usize, bool)> {
    let mut depth = 1;
    let mut nested = false;
    let mut i = from;
    while let Some(at) = find(b, i, b"{}") {
        if b[at] == b'{' {
            depth += 1;
            nested = true;
        } else {
            depth -= 1;
            if depth == 0 {
                return Some((at, nested));
            }
        }
        i = at + 1;
    }
    None
}

/// `text[range]` split on `sep` outside quotes and parentheses: each part's start and its
/// text with white space collapsed, empty parts left out.
fn split(text: &str, range: std::ops::Range<usize>, sep: u8) -> Vec<(usize, String)> {
    let b = text.as_bytes();
    let mut parts = Vec::new();
    let mut start = range.start;
    loop {
        let end = find(&b[..range.end], start, &[sep]).unwrap_or(range.end);
        let part = &text[start..end];
        let lead = part.len() - part.trim_start().len();
        if !part.trim().is_empty() {
            parts.push((start + lead, collapse(part)));
        }
        if end >= range.end {
            return parts;
        }
        start = end + 1;
    }
}

fn declarations(text: &str, body: std::ops::Range<usize>) -> Vec<(usize, String)> {
    split(text, body, b';')
}

/// `s` trimmed, with each run of white space as one space.
fn collapse(s: &str) -> String {
    s.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}
