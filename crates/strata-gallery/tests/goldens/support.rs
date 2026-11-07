//! What the golden checks share: repository paths, the bless decision, the stripped
//! accessibility tree, a headless runtime per scene, and the gallery binary run headless.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use rune::{Env, Runtime};
use serde_json::Value;
use strata_gallery::scenes::{self, Page};
use strata_gallery::{Gallery, SIZE, view};

/// The repository root.
pub fn repo() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// `tests/golden/<kind>/<id>/`.
pub fn golden_dir(kind: &str, id: &str) -> PathBuf {
    repo().join("tests/golden").join(kind).join(id)
}

/// `tests/scripts/<id>/`.
pub fn scripts_dir(id: &str) -> PathBuf {
    repo().join("tests/scripts").join(id)
}

/// Where fresh output goes for review when a golden differs.
pub fn fresh_dir(kind: &str, id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("goldens")
        .join(kind)
        .join(id)
}

pub fn page(id: &str) -> &'static Page {
    scenes::find(id).unwrap_or_else(|| panic!("no gallery page {id:?}"))
}

/// Whether to write the golden for item `name` of page `id` instead of comparing it.
///
/// `rune_bless` is `RUNE_BLESS`: blessing is on when it is set and not `0`, as in Rune.
/// `filter` is `STRATA_BLESS`: empty or unset blesses everything; otherwise it must equal the
/// page id (`c1`), the scene name (`c1-empty-dark`) or `<id>/<script stem>` (`c5/stem`).
pub fn blessing(rune_bless: Option<&str>, filter: Option<&str>, id: &str, name: &str) -> bool {
    if !bless_on(rune_bless) {
        return false;
    }
    match filter.map(str::trim) {
        None | Some("") => true,
        Some(f) => f == id || f == name,
    }
}

fn bless_on(rune_bless: Option<&str>) -> bool {
    rune_bless.is_some_and(|v| v != "0")
}

/// What to do with one golden item in this run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Compare,
    Bless,
    /// Blessing is on and the filter names something else: neither compared nor written.
    Skip,
}

pub fn mode(id: &str, name: &str) -> Mode {
    let rune_bless = std::env::var("RUNE_BLESS").ok();
    let filter = std::env::var("STRATA_BLESS").ok();
    if !bless_on(rune_bless.as_deref()) {
        Mode::Compare
    } else if blessing(rune_bless.as_deref(), filter.as_deref(), id, name) {
        Mode::Bless
    } else {
        Mode::Skip
    }
}

/// Bounds and scroll offsets are layout, not semantics; shortcuts are written per platform.
pub fn strip(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        for k in ["bounds", "scroll", "selection", "shortcut"] {
            o.remove(k);
        }
        if let Some(Value::Array(children)) = o.get_mut("children") {
            children.iter_mut().for_each(strip);
        }
    }
}

pub fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).expect("JSON serializes") + "\n"
}

/// The gallery in `scene`, laid out and settled on the virtual clock.
pub fn mount(scene: fn(&mut Gallery)) -> Runtime<Gallery> {
    let mut gallery = Gallery::default();
    scene(&mut gallery);
    let env = Env::new(gallery.dark, 1.0);
    let mut rt = Runtime::new(gallery, view, SIZE, env);
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(1.0 / 60.0);
    rt
}

/// Whether `scene` leaves the gallery dark.
pub fn is_dark(scene: fn(&mut Gallery)) -> bool {
    let mut g = Gallery::default();
    scene(&mut g);
    g.dark
}

/// The gallery binary, headless: no desktop display, and Rune's GPU line on stderr. It renders
/// on lavapipe when `STRATA_VK_ICD` names an ICD file or a lavapipe ICD is installed, so local
/// renders match CI's; otherwise on the default adapter.
pub fn gallery() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_strata-gallery"));
    cmd.env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("RUNE_GPU_LOG", "1");
    if let Some(icd) = vulkan_icd() {
        cmd.env("VK_ICD_FILENAMES", icd);
    }
    cmd
}

fn vulkan_icd() -> Option<PathBuf> {
    if let Some(icd) = std::env::var_os("STRATA_VK_ICD").filter(|v| !v.is_empty()) {
        return Some(icd.into());
    }
    let dir = Path::new("/usr/share/vulkan/icd.d");
    let mut lavapipe: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("lvp_icd") && n.ends_with(".json"))
        })
        .collect();
    lavapipe.sort();
    lavapipe.into_iter().next()
}

/// Runs `cmd`; its stderr on success, or a description of the failure.
pub fn run(mut cmd: Command) -> Result<String, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("could not start the gallery: {e}"))?;
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if out.status.success() {
        Ok(stderr)
    } else {
        Err(format!(
            "the gallery exited with {}:\n{}",
            out.status,
            tail(&stderr, 20)
        ))
    }
}

/// The adapter Rune reported, for failure messages.
pub fn gpu_line(stderr: &str) -> &str {
    stderr
        .lines()
        .find(|l| l.starts_with("rune: GPU"))
        .unwrap_or("rune: GPU (not reported)")
}

fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Compares `actual` with the text golden `tests/golden/<kind>/<id>/<name>.json`, or writes
/// it when blessing. A difference is pushed onto `failures`, with the fresh copy's path.
/// `bless_name` is what a bless filter names the item by: the scene, or `<id>/<script>`.
pub fn text_golden(
    kind: &str,
    id: &str,
    name: &str,
    bless_name: &str,
    actual: &str,
    failures: &mut Vec<String>,
) {
    let path = golden_dir(kind, id).join(format!("{name}.json"));
    match mode(id, bless_name) {
        Mode::Skip => {}
        Mode::Bless => write(&path, actual.as_bytes()),
        Mode::Compare => {
            let Ok(want) = std::fs::read_to_string(&path) else {
                failures.push(format!(
                    "{kind} {name}: missing golden; run `mise run bless {id}`"
                ));
                return;
            };
            if want.replace("\r\n", "\n") != actual {
                let fresh = fresh_dir(kind, id).join(format!("{name}.json"));
                write(&fresh, actual.as_bytes());
                let line = want
                    .lines()
                    .zip(actual.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(want.lines().count().min(actual.lines().count()));
                failures.push(format!(
                    "{kind} {name}: differs from its golden at line {}\n  golden: {}\n  fresh:  {}",
                    line + 1,
                    path.display(),
                    fresh.display(),
                ));
            }
        }
    }
}

pub fn write(path: &Path, bytes: &[u8]) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("output directory");
    }
    std::fs::write(path, bytes).expect("output written");
}

/// Goldens in `tests/golden/<kind>/<id>/` with no item behind them fail when comparing, and
/// are deleted when blessing the whole page.
pub fn orphans(
    kind: &str,
    id: &str,
    ext: &str,
    expected: &BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    let dir = golden_dir(kind, id);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("golden directory entry").path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        let known = path.extension().is_some_and(|e| e == ext) && expected.contains(&stem);
        if known {
            continue;
        }
        match mode(id, id) {
            Mode::Compare => failures.push(format!(
                "{kind}: {} has no scene or script behind it; remove it or run `mise run bless {id}`",
                path.display()
            )),
            Mode::Bless => std::fs::remove_file(&path).expect("stale golden removed"),
            Mode::Skip => {}
        }
    }
}

/// Panics with every failure, so one run reports them all.
pub fn report(id: &str, kind: &str, failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} {kind} golden failure(s) on page {id}:\n\n{}\n\nAccept intended changes with \
         `mise run bless {id}`, then review every golden diff.",
        failures.len(),
        failures.join("\n\n"),
    );
}
