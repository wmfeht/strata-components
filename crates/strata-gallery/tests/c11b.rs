//! C11b's page: what the sheet grid does in process, beside its goldens and scripts, where a
//! script can't look: a drag held while the preview closes, and a sheet arriving from the
//! page's fake reader. Each test that checks a feature-map Behavior bullet names it.

mod c11b {
    use rune::{Env, Key, KeyEvent, Mods, Runtime};
    use strata_components::preview_drawer::HostedPreview;
    use strata_gallery::{Gallery, scenes};

    const DT: f64 = 1.0 / 60.0;

    fn scene(name: &str) -> Runtime<Gallery> {
        let f = scenes::all_scenes()
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, f)| f)
            .unwrap_or_else(|| panic!("no scene {name}"));
        let mut g = Gallery::default();
        f(&mut g);
        let env = Env::new(g.dark, 1.0);
        let mut rt = strata_gallery::runtime(g, env);
        rt.frame(0.0);
        settle(&mut rt, 30);
        rt
    }

    fn settle(rt: &mut Runtime<Gallery>, frames: usize) {
        for _ in 0..frames {
            rt.frame(DT);
        }
    }

    fn mid(rt: &Runtime<Gallery>, end: &str) -> (f32, f32) {
        let r = rt
            .elements()
            .into_iter()
            .find(|e| e.path.ends_with(end))
            .unwrap_or_else(|| panic!("no element ending {end}"))
            .rect;
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    fn tap(rt: &mut Runtime<Gallery>, at: (f32, f32)) {
        rt.pointer_move(at.0, at.1);
        rt.pointer_down(at.0, at.1);
        rt.frame(DT);
        rt.pointer_up(at.0, at.1);
        settle(rt, 5);
    }

    /// preview/preview-panel/documents: "Closing the preview during a table cell drag ends the
    /// drag and its autoscroll."
    #[test]
    fn closing_the_preview_during_a_cell_drag_ends_the_drag_and_its_autoscroll() {
        let mut rt = scene("c11b-csv-light");
        let from = mid(&rt, "sheet-r0-c1");
        rt.pointer_move(from.0, from.1);
        rt.pointer_down(from.0, from.1);
        rt.frame(DT);
        // Down to the drawer's bottom edge, and held there.
        let bottom = rt
            .elements()
            .into_iter()
            .find(|e| e.path.ends_with("sheet-s0-scroll"))
            .expect("the rows' viewport")
            .rect;
        rt.pointer_move(from.0, bottom[1] + bottom[3] - 2.0);
        settle(&mut rt, 20);
        let moving = rt.state.c11b.grid.cursor().unwrap().row;
        assert!(
            moving > 10,
            "the range follows the autoscroll: row {moving}"
        );
        rt.key(KeyEvent {
            key: Key::Function(9),
            mods: Mods::default(),
        });
        settle(&mut rt, 3);
        assert_eq!(rt.state.c11b.last, "Closed");
        assert!(!rt.state.c11b.grid.dragging(), "the drag ended");
        let at = rt.state.c11b.grid.cursor();
        settle(&mut rt, 30);
        assert_eq!(
            rt.state.c11b.grid.cursor(),
            at,
            "nothing scrolls the range on"
        );
        rt.pointer_up(from.0, bottom[1] + bottom[3] - 2.0);
    }

    /// A workbook's other sheets come from Strata's reader when their tab is picked: the
    /// drawer shows it working meanwhile (no bullet: Strata previews the first sheet only, D4).
    #[test]
    fn a_picked_sheet_arrives_from_the_reader_while_the_drawer_shows_it_loading() {
        let mut rt = scene("c11b-book-light");
        let at = mid(&rt, "sheet-sheet1");
        tap(&mut rt, at);
        assert_eq!(rt.state.c11b.last, "Load 1");
        assert_eq!(rt.state.c11b.grid.busy(), Some("Loading sheet…"));
        assert!(rt.state.c11b.grid.sheet(1).is_none());
        settle(&mut rt, 30);
        let g = &rt.state.c11b.grid;
        assert_eq!(g.busy(), None);
        assert_eq!(g.sheet(1).map(|s| s.name.as_ref()), Some("Q1 sales"));
        assert!(rt.elements().iter().any(|e| e.path.ends_with("sheet-h4")));
    }

    /// Escape is the grid's only while a range is selected: without one, the page hears it.
    #[test]
    fn escape_without_a_range_passes_on_past_the_grid() {
        let mut rt = scene("c11b-csv-light");
        let at = mid(&rt, "sheet-r1-c0");
        tap(&mut rt, at);
        assert!(rt.state.c11b.grid.cursor().is_some());
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Mods::default(),
        });
        settle(&mut rt, 2);
        assert_eq!(
            rt.state.page, None,
            "the gallery's Escape went back to the index"
        );
    }
}
