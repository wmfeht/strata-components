//! Every gallery scene's accessibility tree, compared with its golden under
//! `tests/golden/a11y/`. Accept an intended change with `RUNE_BLESS=1`, then review the diff.

use std::path::PathBuf;

use rune::{Env, Runtime};
use serde_json::Value;
use strata_gallery::{Gallery, SCENES, view};

fn mount(scene: fn(&mut Gallery)) -> Runtime<Gallery> {
    let mut gallery = Gallery::default();
    scene(&mut gallery);
    let mut rt = Runtime::new(gallery, view, (960.0, 640.0), Env::new(false, 1.0));
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(1.0 / 60.0);
    rt
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/a11y")
}

/// Bounds and scroll offsets are layout, not semantics; shortcuts are written per platform.
fn strip(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        for k in ["bounds", "scroll", "selection", "shortcut"] {
            o.remove(k);
        }
        if let Some(Value::Array(children)) = o.get_mut("children") {
            children.iter_mut().for_each(strip);
        }
    }
}

fn check_golden(name: &str, mut tree: Value) {
    strip(&mut tree);
    let actual = serde_json::to_string_pretty(&tree).expect("tree serializes") + "\n";
    let path = golden_dir().join(format!("{name}.json"));
    if std::env::var_os("RUNE_BLESS").is_some_and(|v| v != "0") {
        std::fs::create_dir_all(golden_dir()).expect("golden directory");
        std::fs::write(&path, &actual).expect("golden written");
        return;
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {name}; run with RUNE_BLESS=1"));
    if want.replace("\r\n", "\n") != actual {
        let fresh = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("a11y-{name}.json"));
        std::fs::write(&fresh, &actual).expect("fresh tree written");
        panic!(
            "accessibility golden differs: {name}\n  golden: {}\n  fresh:  {}\n  \
             accept with RUNE_BLESS=1 after review",
            path.display(),
            fresh.display(),
        );
    }
}

#[test]
fn every_scene_matches_its_accessibility_golden() {
    for (name, scene) in SCENES {
        check_golden(name, mount(*scene).accessibility_json());
    }
}
