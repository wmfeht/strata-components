//! Generates the icon catalogs (K3, `docs/work/phase-1-leaves/K3-icon-catalogs.md`) from the
//! SVG sources in `icons/` (`icons/README.md`) into `$OUT_DIR/catalog.rs`.
//!
//! Each source becomes a `rune::Icon`: the elements inside its `<svg>`, with the root's paint
//! attributes moved to a group, as Rune's icon bodies have no `<svg>`. Three arrays hold every
//! icon, sorted by name, so a lookup by name is a binary search:
//!
//! - `LUCIDE`: Strata's Theme colors. Catppuccin colours become `currentColor`, with Strata's
//!   15 % fill on unfilled shapes, so they take the element's colour.
//! - `MOCHA` and `LATTE`: Strata's Brand colors, each Catppuccin colour resolved in that
//!   flavour. Rune draws a body that names its own colours in them.
//!
//! The Lucide and repository icons are the same in all three. Their tint placeholders become
//! `currentColor`. Anything unexpected in a source stops the build with the file's name, so a
//! refresh from Strata can't change what the catalogs mean without someone looking.
//!
//! std only, deterministic: sorted inputs, no paths, times or hash order in the output.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Strata's tint placeholders in its Lucide SVGs (`recolor_icon_source` in `src/assets.rs`),
/// plus the white of `strata-check-on-primary`, which a component tints instead.
const TINTS: [&str; 4] = ["#8bc9eb", "#22d3ee", "#2e3436", "#ffffff"];

/// Catppuccin Mocha and Latte, as Strata's `src/assets.rs` (`CATPPUCCIN_MOCHA`, `_LATTE`) has them.
const MOCHA: [(&str, &str); 16] = [
    ("rosewater", "#f5e0dc"),
    ("flamingo", "#f2cdcd"),
    ("pink", "#f5c2e7"),
    ("mauve", "#cba6f7"),
    ("red", "#f38ba8"),
    ("maroon", "#eba0ac"),
    ("peach", "#fab387"),
    ("yellow", "#f9e2af"),
    ("green", "#a6e3a1"),
    ("teal", "#94e2d5"),
    ("sky", "#89dceb"),
    ("sapphire", "#74c7ec"),
    ("blue", "#89b4fa"),
    ("lavender", "#b4befe"),
    ("text", "#cdd6f4"),
    ("overlay1", "#7f849c"),
];
const LATTE: [(&str, &str); 16] = [
    ("rosewater", "#dc8a78"),
    ("flamingo", "#dd7878"),
    ("pink", "#ea76cb"),
    ("mauve", "#8839ef"),
    ("red", "#d20f39"),
    ("maroon", "#e64553"),
    ("peach", "#fe640b"),
    ("yellow", "#df8e1d"),
    ("green", "#40a02b"),
    ("teal", "#179299"),
    ("sky", "#04a5e5"),
    ("sapphire", "#209fb5"),
    ("blue", "#1e66f5"),
    ("lavender", "#7287fd"),
    ("text", "#4c4f69"),
    ("overlay1", "#8c8fa1"),
];

/// Strata's fill for the folder and, under Theme colors, for a Catppuccin icon's unfilled
/// shapes: its own colour at 15 %.
const FAINT_FILL: &str = r#"fill="currentColor" fill-opacity="0.15""#;

/// Root attributes that paint, and move to the wrapping group.
const PAINT: [&str; 9] = [
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "fill-rule",
    "opacity",
    "fill-opacity",
    "stroke-opacity",
];
/// Root attributes the body doesn't need.
const DROPPED: [&str; 4] = ["xmlns", "width", "height", "viewBox"];

/// The source folders, and whether their colours are Catppuccin's.
const FOLDERS: [(&str, bool); 4] = [
    ("lucide", false),
    ("lucide-extra", false),
    ("own", false),
    ("catppuccin", true),
];

struct Source {
    name: String,
    view: String,
    /// One body for all three catalogs, or Theme, Mocha and Latte bodies.
    bodies: Bodies,
}

enum Bodies {
    Shared(String),
    Catppuccin {
        theme: String,
        mocha: String,
        latte: String,
    },
}

fn main() {
    let root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let icons = root.join("icons");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=icons");

    let mut sources = Vec::new();
    for (folder, catppuccin) in FOLDERS {
        for path in svgs(&icons.join(folder)) {
            sources.push(read(&path, catppuccin));
        }
    }
    sources.sort_by(|a, b| a.name.cmp(&b.name));
    for pair in sources.windows(2) {
        if pair[0].name == pair[1].name {
            panic!("icons: {} is in two folders", pair[0].name);
        }
    }

    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("catalog.rs");
    fs::write(&out, generate(&sources)).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
}

/// The `*.svg` files in `dir`, sorted by name.
fn svgs(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut paths: Vec<PathBuf> = entries
        .map(|e| {
            e.unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
                .path()
        })
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .collect();
    paths.sort();
    paths
}

fn read(path: &Path, catppuccin: bool) -> Source {
    let fail = |why: &str| -> ! { panic!("icons/{}: {why}", relative(path)) };
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| fail("the name isn't UTF-8"))
        .to_owned();
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        fail("names are lowercase ASCII letters, digits, `-` and `_`");
    }
    let svg = fs::read_to_string(path).unwrap_or_else(|e| fail(&e.to_string()));
    let (view, body) = parse(&strip_comments(&svg)).unwrap_or_else(|why| fail(&why));

    let bodies = if catppuccin {
        let theme = resolve(&body, None).unwrap_or_else(|why| fail(&why));
        let theme = theme.replace(r#"fill="none""#, FAINT_FILL);
        let mocha = resolve(&body, Some(&MOCHA)).unwrap_or_else(|why| fail(&why));
        let latte = resolve(&body, Some(&LATTE)).unwrap_or_else(|why| fail(&why));
        Bodies::Catppuccin {
            theme,
            mocha,
            latte,
        }
    } else {
        let mut body = body;
        for tint in TINTS {
            body = body.replace(tint, "currentColor");
        }
        if body.contains('#') || body.contains("var(") {
            fail("names a colour that isn't one of Strata's tint placeholders");
        }
        if name == "strata-folder" {
            body = body.replacen(r#"fill="none""#, FAINT_FILL, 1);
        }
        Bodies::Shared(body)
    };
    Source { name, view, bodies }
}

fn relative(path: &Path) -> String {
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    format!("{parent}/{file}")
}

fn strip_comments(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = rest[start..]
            .find("-->")
            .map_or("", |end| &rest[start + end + 3..]);
    }
    out.push_str(rest);
    out
}

/// The viewBox side, as written, and the body: the root's children, wrapped in a group with
/// its paint attributes.
fn parse(svg: &str) -> Result<(String, String), String> {
    let open = svg.find("<svg").ok_or("no <svg>")?;
    let head_end = open + svg[open..].find('>').ok_or("an unclosed <svg>")?;
    let close = svg.rfind("</svg>").ok_or("no </svg>")?;
    let head = svg[open + 4..head_end].trim_end_matches('/');
    if close < head_end {
        return Err("an empty <svg>".into());
    }

    let mut view = None;
    let mut paint = Vec::new();
    for (key, value) in attributes(head)? {
        if key == "viewBox" {
            view = Some(square_view(value)?);
        } else if PAINT.contains(&key) {
            paint.push(format!(r#"{key}="{value}""#));
        } else if !DROPPED.contains(&key) {
            return Err(format!("an unknown root attribute `{key}`"));
        }
    }
    let view = view.ok_or("no viewBox")?;

    let inner = collapse(&svg[head_end + 1..close]);
    if inner.is_empty() {
        return Err("nothing to draw".into());
    }
    let body = if paint.is_empty() {
        inner
    } else {
        format!("<g {}>{inner}</g>", paint.join(" "))
    };
    Ok((view, body))
}

/// `name="value"` pairs in a tag's head.
fn attributes(head: &str) -> Result<Vec<(&str, &str)>, String> {
    let mut out = Vec::new();
    let mut rest = head.trim_start();
    while !rest.is_empty() {
        let eq = rest
            .find('=')
            .ok_or_else(|| format!("a stray `{rest}` in <svg>"))?;
        let key = rest[..eq].trim();
        let after = rest[eq + 1..].trim_start();
        let value = after
            .strip_prefix('"')
            .ok_or_else(|| format!("`{key}` isn't double-quoted"))?;
        let end = value
            .find('"')
            .ok_or_else(|| format!("`{key}` isn't closed"))?;
        out.push((key, &value[..end]));
        rest = value[end + 1..].trim_start();
    }
    Ok(out)
}

/// The side of a `0 0 s s` viewBox, as written: Rune's icons are square and start at 0 0.
fn square_view(view: &str) -> Result<String, String> {
    let parts: Vec<&str> = view.split([' ', ',']).filter(|s| !s.is_empty()).collect();
    let numeric = |s: &str| {
        let digits = s.replace('.', "");
        !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && s.matches('.').count() <= 1
    };
    match parts.as_slice() {
        ["0", "0", w, h] if w == h && numeric(w) && !w.starts_with('.') && !w.ends_with('.') => {
            Ok((*w).to_owned())
        }
        _ => Err(format!("viewBox `{view}` isn't `0 0 s s`")),
    }
}

/// One space for each run of whitespace, and none between tags.
fn collapse(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.replace("> <", "><").trim().to_owned()
}

/// Catppuccin's `var(--vscode-ctp-<colour>)`s in `palette`, or as `currentColor` without one.
fn resolve(body: &str, palette: Option<&[(&str, &str); 16]>) -> Result<String, String> {
    const VAR: &str = "var(--vscode-ctp-";
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start]);
        let tail = rest[start..]
            .strip_prefix(VAR)
            .ok_or("a CSS variable that isn't Catppuccin's")?;
        let end = tail.find(')').ok_or("an unclosed var(")?;
        let colour = &tail[..end];
        let value = match palette {
            None if MOCHA.iter().any(|(c, _)| *c == colour) => "currentColor",
            None => return Err(format!("`{colour}` isn't a Catppuccin colour")),
            Some(p) => p
                .iter()
                .find(|(c, _)| *c == colour)
                .map(|(_, hex)| *hex)
                .ok_or_else(|| format!("`{colour}` isn't a Catppuccin colour"))?,
        };
        out.push_str(value);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn ident(name: &str) -> String {
    name.to_ascii_uppercase().replace('-', "_")
}

fn generate(sources: &[Source]) -> String {
    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(
        w,
        "// Generated by strata-kit's build.rs from icons/. Do not edit.\n"
    );
    let _ = writeln!(
        w,
        "/// Each icon's index in [`LUCIDE`], [`MOCHA`] and [`LATTE`], by `Icon::name`."
    );
    let _ = writeln!(
        w,
        "#[expect(dead_code, reason = \"the semantic map uses only some of them\")]"
    );
    let _ = writeln!(w, "pub mod index {{");
    let mut idents = Vec::new();
    for (i, s) in sources.iter().enumerate() {
        let id = ident(&s.name);
        if idents.contains(&id) {
            panic!(
                "icons: {} and another name share the identifier {id}",
                s.name
            );
        }
        let _ = writeln!(w, "    pub const {id}: usize = {i};");
        idents.push(id);
    }
    let _ = writeln!(w, "}}\n");

    for (s, id) in sources.iter().zip(&idents) {
        if let Bodies::Shared(body) = &s.bodies {
            let _ = writeln!(w, "const B_{id}: &str = {body:?};");
        }
    }

    let n = sources.len();
    for (array, doc) in [
        (
            "LUCIDE",
            "Theme colors: every icon takes the element's colour.",
        ),
        ("MOCHA", "Brand colors, Catppuccin Mocha."),
        ("LATTE", "Brand colors, Catppuccin Latte."),
    ] {
        let _ = writeln!(w, "\n/// {doc} Sorted by name.");
        let _ = writeln!(w, "pub static {array}: [Icon; {n}] = [");
        for (s, id) in sources.iter().zip(&idents) {
            let body = match (&s.bodies, array) {
                (Bodies::Shared(_), _) => format!("B_{id}"),
                (Bodies::Catppuccin { theme, .. }, "LUCIDE") => format!("{theme:?}"),
                (Bodies::Catppuccin { mocha, .. }, "MOCHA") => format!("{mocha:?}"),
                (Bodies::Catppuccin { latte, .. }, _) => format!("{latte:?}"),
            };
            let _ = writeln!(
                w,
                "    Icon {{ name: {:?}, view: {}_f32, body: {body} }},",
                s.name, s.view
            );
        }
        let _ = writeln!(w, "];");
    }
    out
}
