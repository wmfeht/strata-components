//! The override file: TOML parsed line by line where it breaks, then each row read against
//! the registry.
//!
//! toml's own recovery (`DeTable::parse_recoverable`, toml 1.2.1) isn't enough to keep every
//! good row: after a line of garbage it drops every row below it, a bad value comes back as an
//! empty string, an unclosed `[header` leaves its rows at the top level, and an unterminated
//! multi-line string runs to the end of the file. So [`salvage`] blanks each line toml
//! reports, and a broken header's rows with it, and parses again until nothing is reported.
//! Blanking writes spaces over the bytes, so every span still points into the original text.

use std::ops::Range;

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use super::{OverrideError, Overrides, Problem, Row, Scope, value};
use crate::theme::Location;
use crate::tokens;

pub(super) fn parse(src: &str) -> Overrides {
    let (text, cutoff, mut errors) = salvage(src);
    let (doc, _) = DeTable::parse_recoverable(&text);
    let mut out = Overrides::default();
    let mut walk = Walk {
        src,
        cutoff,
        out: &mut out,
    };
    walk.table(doc.get_ref(), "", Scope::All);
    errors.append(&mut out.errors);
    errors.sort_by_key(|e| (e.at.line, e.at.column));
    out.errors = errors;
    out
}

/// `src` with every line toml can't parse blanked, the byte past which nothing applies (the
/// end, unless an error couldn't be blanked away), and the syntax errors.
fn salvage(src: &str) -> (String, usize, Vec<OverrideError>) {
    let mut text = src.as_bytes().to_vec();
    let mut errors = Vec::new();
    let mut reported = Vec::new();
    // Each round blanks at least one line or stops, so this bounds the loop.
    for _ in 0..=src.lines().count() {
        let found: Vec<(usize, String)> = {
            let s = std::str::from_utf8(&text).expect("blanking keeps UTF-8");
            let (_, errs) = DeTable::parse_recoverable(s);
            errs.iter()
                .map(|e| {
                    let at = e.span().map_or(s.len(), |r| r.start);
                    (at, e.message().trim().to_owned())
                })
                .collect()
        };
        if found.is_empty() {
            return (into_string(text), usize::MAX, errors);
        }
        let mut progressed = false;
        // What this round blanked: an error inside it (a row under a broken header) is a
        // consequence of the one that blanked it.
        let mut hit: Vec<Range<usize>> = Vec::new();
        for &(at, ref message) in &found {
            if hit.iter().any(|r| r.contains(&at) || r.start == at) {
                continue;
            }
            let line = line_of(&text, at);
            let skipped = blank(&mut text, line.clone());
            hit.push(line.start..skipped.end);
            progressed |= skipped.blanked;
            if reported.contains(&at) {
                continue;
            }
            reported.push(at);
            let message = if skipped.rows > 0 {
                format!("{message}; the rows under it don't apply")
            } else {
                message.clone()
            };
            errors.push(OverrideError {
                at: locate(src, at),
                scope: Scope::All,
                problem: Problem::Syntax(message),
            });
        }
        if !progressed {
            // An error blanking can't remove, such as a multi-line string still open at the
            // end of the file: nothing from the first such error on applies.
            let cutoff = found.iter().map(|&(at, _)| at).min().unwrap_or(0);
            return (into_string(text), cutoff, errors);
        }
    }
    (into_string(text), 0, errors)
}

fn into_string(text: Vec<u8>) -> String {
    String::from_utf8(text).expect("blanking keeps UTF-8")
}

/// What [`blank`] did.
struct Blanked {
    /// Some byte that wasn't white space became a space.
    blanked: bool,
    /// Rows under a broken table header blanked with it.
    rows: usize,
    /// The end of what was blanked: the line's end, or the last row's under a header.
    end: usize,
}

/// Writes spaces over `line`, and when it is a table header, over the rows under it up to the
/// next header: they would otherwise land in the table above.
fn blank(text: &mut [u8], line: Range<usize>) -> Blanked {
    let is_header = |l: &[u8]| l.trim_ascii_start().starts_with(b"[");
    let header = is_header(&text[line.clone()]);
    let mut blanked = spaces(text, line.clone());
    let mut rows = 0;
    let mut end = line.end;
    if header {
        let mut at = line.end + 1;
        while at < text.len() {
            let next = line_of(text, at);
            let l = text[next.clone()].trim_ascii();
            if l.starts_with(b"[") {
                break;
            }
            let row = !l.is_empty() && !l.starts_with(b"#");
            if spaces(text, next.clone()) {
                blanked = true;
                rows += usize::from(row);
            }
            end = next.end;
            at = next.end + 1;
        }
    }
    Blanked { blanked, rows, end }
}

/// Writes spaces over every byte of `range` that isn't one, keeping line breaks. True when
/// anything changed.
fn spaces(text: &mut [u8], range: Range<usize>) -> bool {
    let mut changed = false;
    for b in &mut text[range] {
        if !b.is_ascii_whitespace() {
            *b = b' ';
            changed = true;
        }
    }
    changed
}

/// The byte range of the line holding byte `at`, without its line break.
fn line_of(text: &[u8], at: usize) -> Range<usize> {
    let at = at.min(text.len());
    let start = text[..at]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let end = text[at..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(text.len(), |i| at + i);
    start..end
}

/// The location of byte `at` in `src`, moved back onto a character boundary.
pub(super) fn locate(src: &str, at: usize) -> Location {
    let mut at = at.min(src.len());
    while !src.is_char_boundary(at) {
        at -= 1;
    }
    Location::of(src, at)
}

/// Reads a parsed table's rows into `out`.
struct Walk<'a> {
    src: &'a str,
    cutoff: usize,
    out: &'a mut Overrides,
}

impl Walk<'_> {
    fn error(&mut self, at: usize, scope: Scope, problem: Problem) {
        self.out.errors.push(OverrideError {
            at: locate(self.src, at),
            scope,
            problem,
        });
    }

    /// A problem with a key: reported where its line's text starts, so a dotted key
    /// (`list.row.length = 3`) or a table header (`[list.roww]`) reads from its first segment.
    fn key_error(&mut self, at: usize, scope: Scope, problem: Problem) {
        let line = line_of(self.src.as_bytes(), at);
        let indent = self.src.as_bytes()[line.clone()]
            .iter()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
        self.error((line.start + indent).min(at), scope, problem);
    }

    /// The rows of `table`, whose keys continue `prefix` (empty at the top of the file or of
    /// a scope).
    fn table(&mut self, table: &DeTable<'_>, prefix: &str, scope: Scope) {
        for (key, value) in table.iter() {
            // A value reaching an error blanking couldn't remove runs into it.
            if value.span().end >= self.cutoff {
                continue;
            }
            let k: &str = key.get_ref();
            let name = if prefix.is_empty() {
                k.to_owned()
            } else {
                format!("{prefix}.{k}")
            };
            let at = key.span().start;
            if prefix.is_empty() && matches!(k, "light" | "dark") {
                match (scope, value.get_ref()) {
                    (Scope::All, DeValue::Table(t)) => {
                        let inner = if k == "dark" {
                            Scope::Dark
                        } else {
                            Scope::Light
                        };
                        self.table(t, "", inner);
                    }
                    _ => self.key_error(at, scope, Problem::Scope(k.to_owned())),
                }
                continue;
            }
            self.row(&name, at, value, scope);
        }
    }

    fn row(&mut self, name: &str, at: usize, value: &Spanned<DeValue<'_>>, scope: Scope) {
        if let Some(token) = tokens::find(name) {
            match value::read(token, value.get_ref()) {
                Ok(v) => self.out.rows.push(Row {
                    scope,
                    token,
                    value: v,
                    at: locate(self.src, at),
                }),
                Err(problem) => self.error(value.span().start, scope, problem),
            }
            return;
        }
        let group = is_group(name);
        match value.get_ref() {
            DeValue::Table(t) if group => self.table(t, name, scope),
            _ if group => self.key_error(at, scope, Problem::Group(name.to_owned())),
            _ => self.key_error(at, scope, Problem::Unknown(name.to_owned())),
        }
    }
}

/// True when `name` starts some token's name at a dot: `list.row` for `list.row.height`.
fn is_group(name: &str) -> bool {
    tokens::all().any(|t| {
        t.name()
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('.'))
    })
}
