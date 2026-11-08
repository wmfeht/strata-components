//! Unit tests for the highlighter.
//!
//! Colours are checked against syntect's own highlighter (`HighlightState` with
//! `RangedHighlightIterator`) over a full parse, so they also check that colouring by scope
//! stack class gives syntect's colours. Most tests drive the worker on the test thread
//! through [`Pump`], so they are deterministic; a few run the real thread.

mod perf;

use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rune::{Color, Span};
use strata_kit::look::Look;
use syntect::highlighting::{
    HighlightState, Highlighter as SynHighlighter, RangedHighlightIterator, Theme,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference};

use super::detect::{detect, syntaxes};
use super::engine::Lines;
use super::palette::to_rune;
use super::worker::{Request, Worker};
use super::*;

/// How long a test waits on the real thread. Generous: debug builds parse slowly.
const WAIT: Duration = Duration::from_secs(30);

pub(super) fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/code/");
    std::fs::read_to_string(format!("{path}{name}")).expect("fixture")
}

/// The first `n` lines of a fixture.
fn head(name: &str, n: usize) -> String {
    fixture(name).split_inclusive('\n').take(n).collect()
}

pub(super) fn light() -> Arc<Theme> {
    Arc::new(theme_from_look(&Look::fallback(false)))
}

fn dark() -> Arc<Theme> {
    Arc::new(theme_from_look(&Look::fallback(true)))
}

type Runs = Vec<(Range<usize>, Color)>;

/// syntect's own colours for every line of `text`, after a full parse, as runs that join
/// where they touch, with default-coloured text left out. Lines longer than `long_line`
/// are skipped as the highlighter skips them: plain, with the state carried over.
pub(super) fn reference(
    text: &str,
    syntax: &SyntaxReference,
    theme: &Theme,
    long_line: usize,
) -> Vec<Runs> {
    let set = syntaxes();
    let hl = SynHighlighter::new(theme);
    let default = hl.get_default().foreground;
    let mut ps = ParseState::new(syntax);
    let mut hs = HighlightState::new(&hl, ScopeStack::new());
    let lines = Lines::new(text.into());
    (0..lines.len())
        .map(|i| {
            let content = lines.content(i);
            if content.len() > long_line {
                return Vec::new();
            }
            let line = format!("{content}\n");
            let ops = ps.parse_line(&line, set).expect("parse");
            let mut runs: Runs = Vec::new();
            for (style, _, r) in RangedHighlightIterator::new(&mut hs, &ops, &line, &hl) {
                let r = r.start.min(content.len())..r.end.min(content.len());
                if r.is_empty() || style.foreground == default {
                    continue;
                }
                let c = to_rune(style.foreground);
                match runs.last_mut() {
                    Some(last) if last.0.end == r.start && last.1 == c => last.0.end = r.end,
                    _ => runs.push((r, c)),
                }
            }
            runs
        })
        .collect()
}

fn runs_of(spans: &[Span]) -> Runs {
    spans
        .iter()
        .map(|s| {
            (
                s.start as usize..s.end as usize,
                s.color.expect("colour-only span"),
            )
        })
        .collect()
}

fn syntax_for(file_name: &str, text: &str) -> &'static SyntaxReference {
    detect(syntaxes(), None, Some(file_name), text)
}

/// A highlighter worker driven on the test thread: requests queue until [`Pump::run`].
struct Pump {
    worker: Worker,
    rx: Receiver<Request>,
    tx: Sender<Request>,
    blocks_asked: usize,
}

impl Pump {
    fn new(theme: Arc<Theme>) -> Pump {
        let (tx, rx) = mpsc::channel();
        Pump {
            worker: Worker::new(theme, Options::default(), Arc::default()),
            rx,
            tx,
            blocks_asked: 0,
        }
    }

    fn open(&self, src: Source) -> CodeDoc {
        CodeDoc::open(Some(self.tx.clone()), src, Options::default())
    }

    fn blocks(&self) -> CodeBlocks {
        CodeBlocks::new(Some(self.tx.clone()))
    }

    fn set_theme(&self, theme: Arc<Theme>) {
        self.tx.send(Request::SetTheme(theme)).expect("send");
    }

    /// Hands the queued requests to the worker, and sends their replies.
    fn requests(&mut self) {
        while let Ok(req) = self.rx.try_recv() {
            if matches!(req, Request::Block { .. }) {
                self.blocks_asked += 1;
            }
            self.worker.handle(req);
        }
        self.worker.flush();
    }

    /// One unit of work, and its replies.
    fn step(&mut self) -> bool {
        let did = self.worker.engine.step();
        self.worker.flush();
        did
    }

    /// Everything queued, to the end of every pass.
    fn run(&mut self) {
        loop {
            self.requests();
            if !self.step() && self.rx.try_recv().is_err() {
                break;
            }
        }
        self.requests();
    }

    fn parsed(&self, doc: &CodeDoc) -> u64 {
        self.worker.engine.doc(doc.id).map_or(0, |d| d.parsed)
    }
}

/// Every line's colours, after asking for all of them.
fn all_lines(pump: &mut Pump, doc: &mut CodeDoc) -> Vec<Runs> {
    doc.show(0..doc.line_count());
    pump.run();
    doc.poll();
    (0..doc.line_count())
        .map(|i| {
            runs_of(
                &doc.line_spans(i)
                    .unwrap_or_else(|| panic!("line {i} has no colours")),
            )
        })
        .collect()
}

fn assert_lines_eq(got: &[Runs], want: &[Runs], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: line count");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{what}: line {i}");
    }
}

// Detection.

fn detected(name: Option<&str>, text: &str) -> String {
    detect(syntaxes(), None, name, text).name.clone()
}

#[test]
fn detects_by_file_name() {
    assert_eq!(detected(Some("Makefile"), ""), "Makefile");
    assert_eq!(detected(Some("PKGBUILD"), ""), "Bourne Again Shell (bash)");
    assert_eq!(detected(Some("Cargo.lock"), ""), "TOML");
    assert_eq!(
        detected(Some("/home/me/.bashrc"), ""),
        "Bourne Again Shell (bash)"
    );
}

#[test]
fn detects_by_extension_in_any_case() {
    assert_eq!(detected(Some("main.rs"), ""), "Rust");
    assert_eq!(detected(Some("SETUP.PY"), ""), "Python");
    assert_eq!(detected(Some("src/lib/io.c"), ""), "C");
}

#[test]
fn detects_shebangs_from_the_first_line() {
    assert_eq!(
        detected(None, "#!/usr/bin/env bash\necho hi\n"),
        "Bourne Again Shell (bash)"
    );
    assert_eq!(
        detected(Some("run"), "#!/usr/bin/python3\nprint(1)\n"),
        "Python"
    );
    assert_eq!(
        detected(None, "\u{feff}#!/bin/sh\n"),
        "Bourne Again Shell (bash)"
    );
}

#[test]
fn detects_vim_and_emacs_modelines() {
    assert_eq!(
        detected(None, "// vim: set ft=rust:\nfn main() {}\n"),
        "Rust"
    );
    assert_eq!(detected(None, "x = 1\n\n# vi: filetype=python\n"), "Python");
    assert_eq!(
        detected(None, "echo\n# ex: ts=4 ft=sh.doc\n"),
        "Bourne Again Shell (bash)"
    );
    let tail = format!("{}# vim: ft=python\n", "line\n".repeat(30));
    assert_eq!(detected(None, &tail), "Python");
    assert_eq!(detected(None, "/* -*- c++ -*- */\nint x;\n"), "C++");
    assert_eq!(
        detected(None, "# -*- mode: ruby; coding: utf-8 -*-\n"),
        "Ruby"
    );
    // Vim wants the marker after white space, and reads only the first and last 5 lines.
    assert_eq!(detected(None, "novim: ft=rust\n"), "Plain Text");
    let middle = format!("{}// vim: ft=rust\n{}", "a\n".repeat(10), "b\n".repeat(10));
    assert_eq!(detected(None, &middle), "Plain Text");
}

#[test]
fn the_name_beats_the_first_line_and_an_explicit_grammar_beats_both() {
    assert_eq!(detected(Some("tool.py"), "#!/usr/bin/env bash\n"), "Python");
    let s = detect(
        syntaxes(),
        Some("rust"),
        Some("tool.py"),
        "#!/usr/bin/env bash\n",
    );
    assert_eq!(s.name, "Rust");
    let s = detect(syntaxes(), Some("Bourne Again Shell (bash)"), None, "");
    assert_eq!(s.name, "Bourne Again Shell (bash)");
}

#[test]
fn unknown_text_falls_back_to_plain_text() {
    assert_eq!(
        detected(Some("notes.unknownext"), "just words\n"),
        "Plain Text"
    );
    assert_eq!(detected(None, ""), "Plain Text");
    let s = detect(syntaxes(), Some("no-such-grammar"), None, "");
    assert_eq!(s.name, "Plain Text");
}

// Checkpoints.

/// Replies from the engine for one document, applied as the UI side does.
fn engine_lines(
    engine_out: Vec<(u64, super::engine::Reply)>,
    palette: &mut Vec<Option<Color>>,
) -> Vec<(usize, Runs)> {
    let mut lines = Vec::new();
    for (_, reply) in engine_out {
        match reply {
            super::engine::Reply::Palette { from, colors } => {
                palette.truncate(from);
                palette.extend(colors);
            }
            super::engine::Reply::Lines {
                first, lines: l, ..
            } => {
                for (i, tokens) in l.into_iter().enumerate() {
                    let mut runs = Vec::new();
                    spans::colored(&tokens, palette, 0, &mut runs);
                    lines.push((first + i, runs));
                }
            }
            _ => {}
        }
    }
    lines
}

#[test]
fn a_viewport_resumed_from_a_checkpoint_matches_a_full_parse() {
    let theme = light();
    for (name, n) in [
        ("rust-50k.rs", 1500),
        ("c-50k.c", 1500),
        ("js-50k.js", 1500),
        ("python-50k.py", 1500),
        ("markdown-50k.md", 1500),
        ("shell-5k.sh", 5000),
    ] {
        let text = head(name, n);
        let want = reference(&text, syntax_for(name, &text), &theme, LONG_LINE);
        let mut engine = super::engine::Engine::new(theme.clone(), Options::default());
        let open = |text: &str| super::engine::OpenArgs {
            text: text.into(),
            file_name: Some(name.into()),
            syntax: None,
            mode: Mode::Preview,
        };
        engine.open(1, 1, open(&text));
        // A jump to the end before the pass gets there: the lines land when it does.
        let lines = want.len();
        engine.show(1, 2, lines - 80..lines);
        while engine.step() {}
        let mut palette = Vec::new();
        let landed = engine_lines(engine.take_out(), &mut palette);
        assert!(
            landed.iter().any(|(i, _)| *i == lines - 1),
            "{name}: the end landed"
        );
        for (i, runs) in &landed {
            assert_eq!(runs, &want[*i], "{name}: line {i}, deferred to the pass");
        }
        // Each window resumes from the checkpoint at or above it.
        for start in [0, 63, 64, 65, 700, lines - 80] {
            engine.show(1, 3, start..start + 80);
            while engine.step() {}
            let landed = engine_lines(engine.take_out(), &mut palette);
            let covered: Vec<usize> = landed.iter().map(|(i, _)| *i).collect();
            assert!(
                (start..start + 80).all(|i| covered.contains(&i)),
                "{name}: window {start}"
            );
            for (i, runs) in &landed {
                assert_eq!(runs, &want[*i], "{name}: line {i}, window from {start}");
            }
        }
    }
}

#[test]
fn a_line_over_the_cap_draws_plain_and_carries_the_state_over() {
    let theme = light();
    let long = format!("let s = \"{}\";\n", "x".repeat(LONG_LINE + 1024));
    let text = format!("fn a() {{ let n = 1; }}\n{long}fn b() {{ let m = \"two\"; }}\n// done\n");
    let mut pump = Pump::new(theme.clone());
    let mut doc = pump.open(Source::new(text.as_str()).file_name("long.rs"));
    let got = all_lines(&mut pump, &mut doc);
    let want = reference(&text, syntax_for("long.rs", &text), &theme, LONG_LINE);
    assert_lines_eq(&got, &want, "long line");
    assert!(got[1].is_empty(), "the long line is plain");
    assert!(!got[2].is_empty(), "the line after it is coloured");
}

#[test]
fn crlf_lines_colour_like_lf_lines() {
    let lf = "fn main() {\n    let s = \"hi\"; // greet\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    let mut pump = Pump::new(light());
    let mut a = pump.open(Source::new(lf).file_name("a.rs"));
    let mut b = pump.open(Source::new(crlf.as_str()).file_name("a.rs"));
    let (la, lb) = (all_lines(&mut pump, &mut a), all_lines(&mut pump, &mut b));
    assert_eq!(la, lb);
    assert!(la.iter().any(|l| !l.is_empty()));
}

#[test]
fn lines_draw_plain_until_their_colours_land() {
    let mut pump = Pump::new(light());
    let mut doc = pump.open(Source::new("fn main() {}\n").file_name("a.rs"));
    doc.show(0..1);
    assert!(doc.line_spans(0).is_none());
    pump.run();
    assert!(doc.line_spans(0).is_none(), "nothing lands before poll");
    assert!(doc.poll());
    assert_eq!(doc.syntax(), Some("Rust"));
    assert!(doc.line_spans(0).is_some_and(|s| !s.is_empty()));
}

// Edits (S4's workaround 1).

/// Applies `new` to an editable document that has every line cached, runs the thread to the
/// end of the background pass, and checks every line against a full parse of `new`.
fn edit_and_check(
    pump: &mut Pump,
    doc: &mut CodeDoc,
    name: &str,
    new: &str,
    visible: Range<usize>,
    what: &str,
) {
    let old = doc.text().clone();
    doc.edit(new.into(), LineEdit::diff(&old, new), visible);
    pump.run();
    doc.poll();
    assert_eq!(doc.provisional_from(), None, "{what}: the pass ended");
    let got = all_lines(pump, doc);
    let want = reference(new, syntax_for(name, new), &light(), LONG_LINE);
    assert_lines_eq(&got, &want, what);
}

fn replace_line(text: &str, line: usize, f: impl Fn(&str) -> String) -> String {
    text.split_inclusive('\n')
        .enumerate()
        .map(|(i, l)| if i == line { f(l) } else { l.to_owned() })
        .collect()
}

fn line_index(text: &str, from: usize, pred: impl Fn(&str) -> bool) -> usize {
    text.lines()
        .enumerate()
        .skip(from)
        .find(|(_, l)| pred(l))
        .map(|(i, _)| i)
        .expect("a matching line")
}

#[test]
fn edits_are_exact_once_the_background_pass_ends() {
    let name = "shell-5k.sh";
    let text = head(name, 700);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name(name)
            .mode(Mode::Editable),
    );
    all_lines(&mut pump, &mut doc);

    let at = line_index(&text, 100, |l| l.contains("echo"));
    let renamed = replace_line(&text, at, |l| l.replacen("echo", "printf", 1));
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &renamed,
        at - 10..at + 70,
        "rename",
    );

    // An unmatched quote cascades to the end of the text.
    let at = line_index(&renamed, 200, |l| l.trim_start().starts_with("local"));
    let quoted = replace_line(&renamed, at, |l| format!("\"{l}"));
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &quoted,
        at - 10..at + 70,
        "unmatched quote",
    );
    let closed = replace_line(&quoted, at, |l| l[1..].to_owned());
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &closed,
        at - 10..at + 70,
        "quote closed",
    );

    // A heredoc's terminator changes: the body runs on until a line says the new one.
    let at = line_index(&closed, 0, |l| l.contains("<<EOF") || l.contains("<<'EOF'"));
    let renamed_tag = replace_line(&closed, at, |l| l.replace("EOF", "EOT"));
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &renamed_tag,
        at..at + 80,
        "heredoc terminator",
    );

    // Whole lines in and out.
    let at = 300;
    let mut lines: Vec<&str> = renamed_tag.split_inclusive('\n').collect();
    lines.splice(
        at..at,
        ["if [[ -n \"$x\" ]]; then\n", "  echo \"$x\"\n", "fi\n"],
    );
    let inserted: String = lines.concat();
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &inserted,
        at..at + 80,
        "insert lines",
    );
    let mut lines: Vec<&str> = inserted.split_inclusive('\n').collect();
    lines.drain(400..402);
    let deleted: String = lines.concat();
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &deleted,
        390..470,
        "delete lines",
    );
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &format!("{deleted}echo end"),
        640..710,
        "append a line",
    );
}

#[test]
fn a_markdown_fence_opener_edit_is_exact_once_the_pass_ends() {
    let name = "markdown-50k.md";
    let text = head(name, 400);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name(name)
            .mode(Mode::Editable),
    );
    all_lines(&mut pump, &mut doc);
    let at = line_index(&text, 0, |l| l.starts_with("```") && l.len() > 3);
    let opener = replace_line(&text, at, |l| format!("`{l}"));
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &opener,
        at..at + 80,
        "fence opener",
    );
    let dropped = replace_line(&opener, at, |_| "\n".into());
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &dropped,
        at..at + 80,
        "fence opener removed",
    );
}

#[test]
fn quick_edits_before_and_after_the_frontier_settle_exact() {
    let name = "shell-5k.sh";
    let text = head(name, 700);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name(name)
            .mode(Mode::Editable),
    );
    all_lines(&mut pump, &mut doc);
    // An unmatched quote at 100: run only its foreground, so the pass stops near 210.
    let one = replace_line(&text, 100, |l| format!("\"{l}"));
    doc.edit(one.as_str().into(), LineEdit::diff(&text, &one), 90..170);
    pump.requests();
    pump.step();
    // Then edits before the frontier and after it, while the pass is still running.
    let two = replace_line(&one, 50, |l| format!("# {l}"));
    doc.edit(two.as_str().into(), LineEdit::diff(&one, &two), 40..120);
    pump.requests();
    pump.step();
    let three = replace_line(&two, 500, |l| format!("x=1; {l}"));
    edit_and_check(
        &mut pump,
        &mut doc,
        name,
        &three,
        490..570,
        "three quick edits",
    );
}

#[test]
fn an_edit_re_parses_to_the_viewport_bottom_then_finishes_in_the_background() {
    let name = "shell-5k.sh";
    let text = head(name, 1000);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name(name)
            .mode(Mode::Editable),
    );
    all_lines(&mut pump, &mut doc);
    let before = pump.parsed(&doc);
    let new = replace_line(&text, 100, |l| format!("\"{l}"));
    doc.edit(new.as_str().into(), LineEdit::diff(&text, &new), 90..170);
    pump.requests();
    pump.step();
    doc.poll();
    let foreground = pump.parsed(&doc) - before;
    assert!(
        foreground <= 170 + OVERSCAN as u64 - 100,
        "foreground parsed {foreground} lines"
    );
    assert_eq!(doc.provisional_from(), Some(100));
    assert!(
        doc.is_provisional(600),
        "a line past the frontier is provisional"
    );
    assert!(!doc.is_provisional(150), "a line in view is exact");
    pump.run();
    doc.poll();
    assert_eq!(doc.provisional_from(), None);
    assert!(!doc.is_provisional(600));
    assert!(
        pump.parsed(&doc) - before > 800,
        "the cascade ran to the end in the background"
    );
}

#[test]
fn an_edit_below_an_unfinished_cascade_is_coloured_at_once_then_made_exact() {
    let name = "shell-5k.sh";
    let text = head(name, 1500);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name(name)
            .mode(Mode::Editable),
    );
    all_lines(&mut pump, &mut doc);
    let one = replace_line(&text, 100, |l| format!("\"{l}"));
    doc.edit(one.as_str().into(), LineEdit::diff(&text, &one), 90..170);
    pump.requests();
    pump.step();
    let before = pump.parsed(&doc);
    let two = replace_line(&one, 1200, |l| format!("x=1; {l}"));
    doc.edit(two.as_str().into(), LineEdit::diff(&one, &two), 1190..1270);
    pump.requests();
    pump.step();
    doc.poll();
    let foreground = pump.parsed(&doc) - before;
    assert!(
        foreground <= 200,
        "the foreground parsed {foreground} lines, not the cascade"
    );
    assert!(doc.line_spans(1200).is_some() && doc.is_provisional(1200));
    pump.run();
    doc.poll();
    assert!(!doc.is_provisional(1200));
    let got = all_lines(&mut pump, &mut doc);
    assert_lines_eq(
        &got,
        &reference(&two, syntax_for(name, &two), &light(), LONG_LINE),
        "two edits",
    );
}

#[test]
fn a_rename_outside_a_capture_context_converges_within_a_few_lines() {
    let text: String = (0..300)
        .map(|i| format!("run_step {i} --flag value_{i}\n"))
        .collect();
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name("s.sh")
            .mode(Mode::Editable),
    );
    pump.run();
    let before = pump.parsed(&doc);
    let new = replace_line(&text, 150, |l| l.replace("run_step", "go_step"));
    doc.edit(new.as_str().into(), LineEdit::diff(&text, &new), 140..220);
    pump.run();
    let reparsed = pump.parsed(&doc) - before;
    assert!(reparsed <= 8, "the edit re-parsed {reparsed} lines");
    doc.poll();
    let got = all_lines(&mut pump, &mut doc);
    assert_lines_eq(
        &got,
        &reference(&new, syntax_for("s.sh", &new), &light(), LONG_LINE),
        "rename",
    );
}

#[test]
fn a_preview_starts_over_on_an_edit_and_ends_exact() {
    let name = "rust-50k.rs";
    let text = head(name, 300);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(Source::new(text.as_str()).file_name(name));
    all_lines(&mut pump, &mut doc);
    let new = replace_line(&text, 10, |l| format!("/* {l}"));
    doc.edit(new.as_str().into(), LineEdit::diff(&text, &new), 0..80);
    assert!(doc.line_spans(200).is_none(), "the cache started over");
    pump.run();
    doc.poll();
    let got = all_lines(&mut pump, &mut doc);
    assert_lines_eq(
        &got,
        &reference(&new, syntax_for(name, &new), &light(), LONG_LINE),
        "preview edit",
    );
}

#[test]
fn line_edit_diff_finds_the_changed_lines() {
    assert_eq!(
        LineEdit::diff("a\nb\nc", "a\nB\nc"),
        LineEdit {
            first: 1,
            removed: 1,
            inserted: 1
        }
    );
    assert_eq!(
        LineEdit::diff("a\nc", "a\nb\nc"),
        LineEdit {
            first: 1,
            removed: 0,
            inserted: 1
        }
    );
    assert_eq!(
        LineEdit::diff("a\nb\nc", "a\nc"),
        LineEdit {
            first: 1,
            removed: 1,
            inserted: 0
        }
    );
    assert_eq!(
        LineEdit::diff("a\na", "a\na\na"),
        LineEdit {
            first: 2,
            removed: 0,
            inserted: 1
        }
    );
}

// Themes.

#[test]
fn a_theme_switch_recolours_cached_lines_without_re_parsing() {
    let name = "rust-50k.rs";
    let text = head(name, 300);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(Source::new(text.as_str()).file_name(name));
    let before = all_lines(&mut pump, &mut doc);
    let parsed = pump.parsed(&doc);
    pump.set_theme(dark());
    pump.run();
    assert!(doc.poll(), "the palette landed");
    assert_eq!(pump.parsed(&doc), parsed, "no line was parsed again");
    let after: Vec<Runs> = (0..doc.line_count())
        .map(|i| runs_of(&doc.line_spans(i).expect("still cached")))
        .collect();
    assert_lines_eq(
        &after,
        &reference(&text, syntax_for(name, &text), &dark(), LONG_LINE),
        "dark",
    );
    assert_ne!(before, after);
}

// The worker.

#[test]
fn the_newest_viewport_wins() {
    let name = "rust-50k.rs";
    let text = head(name, 1200);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(Source::new(text.as_str()).file_name(name));
    pump.run();
    doc.show(0..10);
    doc.show(500..510);
    doc.show(1000..1010);
    pump.run();
    doc.poll();
    assert!(doc.line_spans(1000).is_some());
    assert!(
        doc.line_spans(0).is_none(),
        "a superseded viewport is skipped"
    );
    assert!(
        doc.line_spans(500).is_none(),
        "a superseded viewport is skipped"
    );
}

#[test]
fn an_edit_cuts_in_on_another_texts_background_pass() {
    let mut pump = Pump::new(light());
    let big = head("markdown-50k.md", 3000);
    let _preview = pump.open(Source::new(big.as_str()).file_name("big.md"));
    let script = "echo one\necho two\n";
    let mut doc = pump.open(Source::new(script).file_name("s.sh").mode(Mode::Editable));
    pump.requests();
    pump.step();
    pump.step();
    let new = "echo one\nprintf two\n";
    doc.edit(new.into(), LineEdit::diff(script, new), 0..2);
    pump.requests();
    pump.step();
    doc.poll();
    assert!(
        doc.line_spans(1).is_some(),
        "the edited line is coloured after one step"
    );
    assert!(
        pump.worker.engine.has_work(),
        "while the preview's pass still runs"
    );
}

#[test]
fn closing_a_text_stops_its_pass() {
    let mut pump = Pump::new(light());
    let big = head("rust-50k.rs", 3000);
    let doc = pump.open(Source::new(big.as_str()).file_name("big.rs"));
    pump.requests();
    pump.step();
    assert!(pump.worker.engine.has_work());
    drop(doc);
    pump.requests();
    assert!(!pump.worker.engine.has_work());
}

// The thread.

#[test]
fn the_thread_wakes_the_window_and_an_idle_text_stays_idle() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let h = Highlighter::new(light());
    let w = wakes.clone();
    h.set_waker(Arc::new(move || {
        w.fetch_add(1, Ordering::SeqCst);
    }));
    let mut doc = h.open(Source::new("fn main() { let x = 1; }\n").file_name("a.rs"));
    doc.show(0..1);
    assert!(doc.settle(WAIT));
    assert!(wakes.load(Ordering::SeqCst) > 0);
    assert!(doc.line_spans(0).is_some_and(|s| !s.is_empty()));
    let after = wakes.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(50));
    assert!(!doc.poll(), "nothing more lands");
    assert_eq!(wakes.load(Ordering::SeqCst), after, "no more wakes");
}

#[test]
fn the_thread_starts_on_first_use_and_ends_with_the_last_handle() {
    let h = Highlighter::new(light());
    assert!(!h.join(), "no thread before first use");
    let h = Highlighter::new(light());
    let mut doc = h.open(Source::new("x\n"));
    assert!(doc.settle(WAIT));
    drop(doc);
    assert!(h.join(), "the thread ended");
}

// Adapters.

const BLOCK: &str = "fn main() {\n    let s = \"hi\"; // greet\n    println!(\"{s}\");\n}";

fn block_reference(lang: &str, code: &str, theme: &Theme) -> Runs {
    let syntax = super::detect::by_token(syntaxes(), lang).expect("grammar");
    let lines = Lines::new(code.into());
    let mut out = Vec::new();
    for (i, runs) in reference(code, syntax, theme, LONG_LINE)
        .into_iter()
        .enumerate()
    {
        let at = lines.start(i);
        for (r, c) in runs {
            out.push((r.start + at..r.end + at, c));
        }
    }
    out
}

fn highlight_runs(h: &[rune_kit::syntax::Highlight]) -> Runs {
    h.iter()
        .map(|(r, p)| match p {
            rune_kit::syntax::Paint::Color(c) => (r.clone(), *c),
            rune_kit::syntax::Paint::Kind(k) => panic!("a kind, not a colour: {k:?}"),
        })
        .collect()
}

#[test]
fn a_markdown_block_answers_plain_then_coloured_once_it_lands() {
    let mut pump = Pump::new(light());
    let mut blocks = pump.blocks();
    let first = blocks.markdown();
    assert!(
        first.highlight(Some("rust"), BLOCK).is_empty(),
        "a miss is plain"
    );
    assert!(
        blocks.highlight(Some("rust"), BLOCK).is_none(),
        "still pending"
    );
    pump.run();
    assert_eq!(pump.blocks_asked, 1, "one request for the same block");
    assert!(blocks.poll());
    let again = blocks.markdown();
    let got = highlight_runs(&again.highlight(Some("rust"), BLOCK));
    let want = block_reference("rust", BLOCK, &light());
    assert!(!want.is_empty());
    assert_eq!(got, want);
    assert!(!blocks.poll(), "nothing more lands");
}

#[test]
fn blocks_with_no_language_or_an_unknown_one_stay_plain() {
    let mut pump = Pump::new(light());
    let mut blocks = pump.blocks();
    assert_eq!(blocks.highlight(None, BLOCK), Some(Vec::new()));
    assert_eq!(blocks.highlight(Some("  "), BLOCK), Some(Vec::new()));
    pump.run();
    assert_eq!(pump.blocks_asked, 0, "no language asks nothing");
    assert!(blocks.highlight(Some("no-such-lang"), BLOCK).is_none());
    pump.run();
    blocks.poll();
    assert_eq!(
        blocks.highlight(Some("no-such-lang"), BLOCK),
        Some(Vec::new())
    );
}

#[test]
fn a_theme_switch_rekeys_and_recolours_blocks() {
    let mut pump = Pump::new(light());
    let mut blocks = pump.blocks();
    blocks.highlight(Some("rust"), BLOCK);
    pump.run();
    blocks.poll();
    let light_runs = highlight_runs(&blocks.highlight(Some("rust"), BLOCK).expect("landed"));
    let before = blocks.markdown();
    assert_eq!(
        highlight_runs(&before.highlight(Some("rust"), BLOCK)),
        light_runs
    );
    pump.set_theme(dark());
    pump.run();
    assert!(blocks.poll());
    let after = blocks.markdown();
    let dark_runs = highlight_runs(&after.highlight(Some("rust"), BLOCK));
    assert_ne!(light_runs, dark_runs);
    assert_eq!(pump.blocks_asked, 1, "no block was parsed again");
    // The old key's cached (light) answer stays apart from the new key's.
    assert_eq!(
        highlight_runs(&before.highlight(Some("rust"), BLOCK)),
        light_runs
    );
}

#[test]
fn wait_on_miss_answers_coloured_on_the_first_call() {
    let h = Highlighter::new(light());
    let mut blocks = h.blocks();
    blocks.wait_on_miss(Some(WAIT));
    let got = highlight_runs(
        &blocks
            .markdown()
            .highlight(Some("c"), "#include <stdio.h>\nint x = 1;"),
    );
    assert!(!got.is_empty());
}

#[test]
fn text_spans_are_offset_into_the_whole_text() {
    let text = "fn a() {}\n\nlet s = \"x\"; // c\n";
    let mut pump = Pump::new(light());
    let mut doc = pump.open(Source::new(text).file_name("a.rs").mode(Mode::Editable));
    let per_line = all_lines(&mut pump, &mut doc);
    let lines = Lines::new(text.into());
    let mut want = Vec::new();
    for (i, runs) in per_line.into_iter().enumerate() {
        want.extend(
            runs.into_iter()
                .map(|(r, c)| (r.start + lines.start(i)..r.end + lines.start(i), c)),
        );
    }
    let got = runs_of(&doc.text_spans(0..doc.line_count()));
    assert_eq!(got, want);
    // Default-coloured text (the variable `s`) gets no span, and touching runs of one colour
    // are joined.
    let s_at = text.find("let s").expect("let") + 4;
    assert!(got.iter().all(|(r, _)| !r.contains(&s_at)));
    assert!(!got.is_empty());
    assert!(
        got.windows(2)
            .all(|w| w[0].0.end != w[1].0.start || w[0].1 != w[1].1)
    );
}

#[test]
fn text_spans_stop_below_runes_span_limit() {
    let line = format!("x = [{}]\n", "1,".repeat(1000));
    let text = line.repeat(40);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name("n.py")
            .mode(Mode::Editable),
    );
    doc.show(0..40);
    pump.run();
    doc.poll();
    assert_eq!(doc.text_spans(0..40).len(), MAX_SPANS);
}

#[test]
fn the_look_theme_colours_kinds_from_the_look() {
    let look = Look::fallback(false);
    let theme = theme_from_look(&look);
    let mut pump = Pump::new(Arc::new(theme));
    let mut blocks = pump.blocks();
    blocks.highlight(Some("c"), "#include <stdio.h>\n// note\n");
    pump.run();
    blocks.poll();
    let runs = highlight_runs(
        &blocks
            .highlight(Some("c"), "#include <stdio.h>\n// note\n")
            .expect("landed"),
    );
    let near = |a: Color, b: Color| {
        (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
    };
    let comment = look.syntax.color(rune_kit::syntax::Syntax::Comment);
    assert!(
        runs.iter()
            .any(|(_, c)| near(*c, look.syntax_extra.preprocessor)),
        "the preprocessor colour"
    );
    assert!(
        runs.iter().any(|(_, c)| near(*c, comment)),
        "the comment colour"
    );
}

#[test]
fn a_waker_set_after_the_thread_starts_is_used() {
    let h = Highlighter::new(light());
    let mut doc = h.open(Source::new("x = 1\n").file_name("a.py"));
    assert!(doc.settle(WAIT));
    let woke = Arc::new(Mutex::new(false));
    let w = woke.clone();
    h.set_waker(Arc::new(move || {
        if let Ok(mut b) = w.lock() {
            *b = true;
        }
    }));
    doc.show(0..1);
    assert!(doc.settle(WAIT));
    assert!(woke.lock().is_ok_and(|b| *b));
}

#[test]
fn random_edits_with_partial_passes_end_exact() {
    let name = "shell-5k.sh";
    let mut text = head(name, 400);
    let mut pump = Pump::new(light());
    let mut doc = pump.open(
        Source::new(text.as_str())
            .file_name(name)
            .mode(Mode::Editable),
    );
    all_lines(&mut pump, &mut doc);
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut rand = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    for round in 0..60 {
        let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
        let at = rand(lines.len());
        match rand(6) {
            0 => lines[at].insert(0, '"'),
            1 => lines[at] = lines[at].replacen('"', "", 1),
            2 => lines.insert(at, "cat <<EOF\n".into()),
            3 => lines.insert(at, "EOF\n".into()),
            4 if lines.len() > 10 => {
                lines.remove(at);
            }
            _ => lines[at] = lines[at].replace("EOF", "EOT"),
        }
        let new: String = lines.concat();
        let line = at.min(lines.len().saturating_sub(1));
        doc.edit(
            new.as_str().into(),
            LineEdit::diff(&text, &new),
            line.saturating_sub(20)..line + 60,
        );
        text = new;
        pump.requests();
        for _ in 0..rand(4) {
            pump.step();
        }
        doc.poll();
        if round % 15 == 14 {
            pump.run();
            doc.poll();
            assert_eq!(doc.provisional_from(), None);
            let got = all_lines(&mut pump, &mut doc);
            let want = reference(&text, syntax_for(name, &text), &light(), LONG_LINE);
            assert_lines_eq(&got, &want, &format!("after round {round}"));
        }
    }
}
