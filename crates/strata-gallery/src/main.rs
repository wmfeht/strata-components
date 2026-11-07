//! `cargo run -p strata-gallery` opens the gallery on its index; Escape goes back to it.
//! `-- --snap out.png --scene gallery-index-dark` renders a scene headless, and
//! `-- --scene <scene> --script tests/scripts/<id>/<name>.json` drives one.

use rune::{AppSpec, Key, KeyEvent, Tx};
use strata_gallery::{Gallery, SIZE, scenes, view};

fn main() {
    // AppSpec takes a 'static slice; the list is built once for the process.
    let scenes = Vec::leak(scenes::all_scenes());
    rune::run(AppSpec::<Gallery> {
        scenes,
        on_key: Some(on_key),
        ..AppSpec::new("Strata components", SIZE, Gallery::default, view)
    });
}

/// Escape on a page goes back to the index.
fn on_key(g: &mut Gallery, key: &KeyEvent, _tx: &mut Tx) -> bool {
    if key.key == Key::Escape && g.page.is_some() {
        g.page = None;
        return true;
    }
    false
}
