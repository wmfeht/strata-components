//! `cargo run -p strata-gallery` opens the gallery. `-- --snap out.png --scene empty-dark`
//! renders a scene headless; `-- --script tests/scripts/<name>.json` drives one.

use rune::AppSpec;
use strata_gallery::{Gallery, SCENES, view};

fn main() {
    rune::run(AppSpec::<Gallery> {
        scenes: SCENES,
        ..AppSpec::new("Strata components", (960.0, 640.0), Gallery::default, view)
    });
}
