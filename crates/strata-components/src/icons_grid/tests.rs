//! Tile metrics and the caption cut, measured in Rune's own fonts.

mod perf;
mod runtime;

use strata_kit::look::Look;

use super::caption::{CaptionCache, cut};
use super::metrics::TileMetrics;

const LONG: &str = "2026-10-07-release-candidate-build-final-v3-with-a-very-long-tail.tar.gz";

#[test]
fn a_tile_is_as_tall_with_any_name_or_details_and_its_slot_ignores_the_text_size() {
    let look = Look::fallback(false);
    let a = TileMetrics::new(&look, 64.0, 2, true);
    assert_eq!(a.slot, 64.0);
    // The name and details reserve their lines: nothing the host streams in moves the grid.
    let mut big = Look::fallback(false);
    big.kit.ty.ui_compact.size *= 2.0;
    big.kit.ty.ui_compact.line_height *= 2.0;
    let b = TileMetrics::new(&big, 64.0, 2, true);
    assert_eq!(b.slot, 64.0, "the thumbnail doesn't follow the text size");
    assert!(b.height > a.height, "the caption does");
}

#[test]
fn sizes_off_the_ladder_are_held_to_its_ends() {
    let look = Look::fallback(false);
    assert_eq!(TileMetrics::new(&look, 8.0, 2, true).slot, 32.0);
    assert_eq!(TileMetrics::new(&look, 999.0, 2, true).slot, 256.0);
    assert_eq!(TileMetrics::new(&look, f32::NAN, 2, true).slot, 64.0);
}

#[test]
fn rows_hold_more_tiles_as_the_width_grows_up_to_the_density_cap() {
    let mut look = Look::fallback(false);
    let m = TileMetrics::new(&look, 32.0, 2, true);
    assert_eq!(m.per_line(0.0), 1);
    assert_eq!(m.per_line(m.min_cell * 3.0 + 1.0), 3);
    assert_eq!(m.per_line(100_000.0), 20, "Compact caps at 20");
    look.content.pad = 8.0;
    let airy = TileMetrics::new(&look, 32.0, 2, true);
    assert_eq!(airy.per_line(100_000.0), 16, "Airy caps at 16");
}

#[test]
fn a_name_that_fits_is_whole_and_a_long_one_keeps_both_ends() {
    let font = Look::fallback(false).kit.ty.ui_compact;
    let short = cut("notes.md", font, 100.0, 2);
    assert!(!short.cut);
    assert_eq!(&*short.text, "notes.md");
    let long = cut(LONG, font, 100.0, 2);
    assert!(long.cut);
    assert!(long.text.contains('…'));
    assert!(long.text.starts_with("2026"));
    assert!(
        long.text.ends_with(".tar.gz"),
        "the end that tells names apart stays"
    );
    let line = strata_kit::typography::line_of(font);
    assert!(long.size.1 <= 2.0 * line + 0.5, "it fits its two lines");
    // One line holds less of it.
    let one = cut(LONG, font, 100.0, 1);
    assert!(one.text.chars().count() < long.text.chars().count());
}

#[test]
fn the_caption_cache_stays_bounded() {
    let cache = CaptionCache::default();
    let font = Look::fallback(false).kit.ty.ui_compact;
    for i in 0..10_000 {
        cache.get(&format!("file-{i}.txt"), font, 100.0, 2);
    }
    assert!(cache.len() <= 2 * 4096);
    let a = cache.get("file-9999.txt", font, 100.0, 2);
    assert_eq!(&*a.text, "file-9999.txt", "a recent cut is still there");
}

#[test]
fn a_middle_press_autoscrolls_from_no_tile_or_a_tiles_blank_space_only() {
    use crate::collection::element::{Ctx, Hit, PointerInput, PointerPhase, Region};
    use crate::collection::selection::Selection;
    use crate::contract::{Point, Rect, UniformRows};
    use crate::scroll::AutoscrollSettings;
    use rune::{Mods, PointerButton};

    let src = crate::contract::ListSource::new(Vec::new());
    let geometry = UniformRows::new(0, 20.0, 100.0);
    let mut sel = Selection::new();
    let cx = Ctx::new(&src, &geometry, Rect::default(), 0.0, &mut sel);
    let press = |index, region| PointerInput {
        phase: PointerPhase::Down,
        button: PointerButton::Middle,
        point: Point::default(),
        window: Point::default(),
        travel: Point::default(),
        hit: Hit { index, region },
        mods: Mods::default(),
        clicks: 1,
        cancelled: false,
        now: 0.0,
    };
    let s = AutoscrollSettings::default();
    let bg = |index, region| super::autoscroll_background(&press(index, region), &cx, &s);
    assert!(bg(None, Region::Gutter), "no tile");
    assert!(bg(Some(3), Region::Gutter), "a tile's blank space");
    assert!(!bg(Some(3), Region::Content), "its thumbnail");
    assert!(!bg(Some(3), Region::Name), "its name");
}
