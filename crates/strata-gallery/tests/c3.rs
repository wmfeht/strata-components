//! C3's page in process: what the accessibility tree doesn't show or a script can't hold at
//! the pinned Rune. Tooltips (a pointer resting for the page's 900 ms), hover, modified presses
//! and Ctrl+Shift+wheel can't be scripted or seen in the tree; held thumbnails across a zoom
//! need the store. Each test names the bullets it checks; the scripts in `tests/scripts/c3/`
//! and `strata-components`' `icons_grid` tests check the rest.

#[expect(
    dead_code,
    reason = "c3 uses only the gallery command and the scripts folder"
)]
#[path = "goldens/support.rs"]
mod support;

mod c3 {
    use rune::{
        ElementInfo, Env, GesturePhase, Key, KeyEvent, Mods, PhysicalInput, Runtime, WheelEvent,
    };
    use strata_components::contract::{EntryId, EntrySource};
    use strata_components::thumbnails::State;
    use strata_gallery::scenes::c3::{HELP_DELAY, grid};
    use strata_gallery::{Gallery, scenes};

    const FRAME: f64 = 1.0 / 60.0;

    const CTRL: Mods = Mods {
        shift: false,
        cmd: true,
        alt: false,
        ctrl: true,
        logo: false,
    };

    const SHIFT: Mods = Mods {
        shift: true,
        cmd: false,
        alt: false,
        ctrl: false,
        logo: false,
    };

    fn mount(name: &str) -> Runtime<Gallery> {
        let scene = scenes::all_scenes()
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, f)| f)
            .unwrap_or_else(|| panic!("no scene {name}"));
        let mut g = Gallery::default();
        scene(&mut g);
        let env = Env::new(g.dark, 1.0);
        let mut rt = strata_gallery::runtime(g, env);
        rt.frame(0.0);
        run(&mut rt, 1.0);
        rt
    }

    fn run(rt: &mut Runtime<Gallery>, seconds: f64) {
        for _ in 0..(seconds / FRAME).round() as usize {
            rt.frame(FRAME);
        }
    }

    /// The element keyed as `path`'s last part, under an element keyed as each part before it.
    fn element(rt: &Runtime<Gallery>, path: &str) -> Option<ElementInfo> {
        let parts: Vec<&str> = path.split('/').collect();
        let (last, above) = parts.split_last()?;
        rt.elements().into_iter().find(|e| {
            !e.exiting
                && e.key.as_deref() == Some(*last)
                && above.iter().all(|k| e.path.split('/').any(|seg| seg == *k))
        })
    }

    fn center(rt: &Runtime<Gallery>, path: &str) -> (f32, f32) {
        let [x, y, w, h] = element(rt, path)
            .unwrap_or_else(|| panic!("no element {path}"))
            .visible;
        (x + w / 2.0, y + h / 2.0)
    }

    /// Presses and releases at the middle of `path`, with `mods` held.
    fn click(rt: &mut Runtime<Gallery>, path: &str, mods: Mods) {
        let (x, y) = center(rt, path);
        rt.physical_input(PhysicalInput::Modifiers(mods));
        rt.pointer_down(x, y);
        rt.frame(FRAME);
        rt.pointer_up(x, y);
        rt.frame(FRAME);
        rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
        run(rt, 0.6);
    }

    fn key(rt: &mut Runtime<Gallery>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        run(rt, 0.1);
    }

    /// Moves the pointer onto `path` and rests it there past the page's tooltip delay.
    fn rest_on(rt: &mut Runtime<Gallery>, path: &str) -> Option<String> {
        let (x, y) = center(rt, path);
        rt.pointer_move(x, y);
        run(rt, HELP_DELAY + 0.2);
        rt.help_tooltip().map(|(t, _)| t.to_owned())
    }

    fn selected(rt: &Runtime<Gallery>) -> Vec<u64> {
        let d = &rt.state.c3;
        d.coll
            .selection
            .selected_ids(&d.listing)
            .into_iter()
            .map(EntryId::get)
            .collect()
    }

    const LONG: &str = "2026-10-07-release-candidate-build-final-v3-with-a-very-long-tail.tar.gz";

    /// icons: "Resting the pointer about a second (900 ms) on a card whose name is
    /// middle-ellipsized shows the full name in a tooltip", "A name that fits in its two caption
    /// lines shows no tooltip", "The tooltip appears only over the card's thumbnail or caption,
    /// not its padding" and "Over the caption, only the name's text box and the details text
    /// count".
    #[test]
    fn a_cut_name_rests_into_a_tooltip_over_its_frame_name_and_details_only() {
        let mut rt = mount("c3-default-light");
        // Entry 11 is the long name.
        assert_eq!(rest_on(&mut rt, "row-11/label").as_deref(), Some(LONG));
        rt.pointer_move(1.0, 1.0);
        run(&mut rt, 0.2);
        assert_eq!(rest_on(&mut rt, "row-11/frame").as_deref(), Some(LONG));
        rt.pointer_move(1.0, 1.0);
        run(&mut rt, 0.2);
        assert_eq!(rest_on(&mut rt, "row-11/details").as_deref(), Some(LONG));
        rt.pointer_move(1.0, 1.0);
        run(&mut rt, 0.2);
        assert_eq!(rest_on(&mut rt, "row-11/gap"), None, "the card's padding");
        rt.pointer_move(1.0, 1.0);
        run(&mut rt, 0.2);
        assert_eq!(rest_on(&mut rt, "row-7/label"), None, "beach.jpg fits");
    }

    /// icons: "Resting the pointer about a second (900 ms)…": the page's delay, not Rune's
    /// 0.5 s, holds it back.
    #[test]
    fn the_tooltip_waits_for_the_pages_delay() {
        let mut rt = mount("c3-default-light");
        let (x, y) = center(&rt, "row-11/label");
        rt.pointer_move(x, y);
        run(&mut rt, 0.7);
        assert_eq!(rt.help_tooltip(), None, "not yet at 0.7 s");
        run(&mut rt, 0.4);
        assert!(rt.help_tooltip().is_some(), "shown past 0.9 s");
    }

    /// icons: "After a rename commits, the tooltip shows the new name; while the rename field
    /// is open, none appears."
    #[test]
    fn a_tile_being_renamed_has_no_tooltip_and_a_renamed_one_shows_its_new_name() {
        let mut rt = mount("c3-default-light");
        rt.update(|g, _| g.c3.renaming = Some(EntryId::new(11)));
        run(&mut rt, 0.1);
        assert_eq!(rest_on(&mut rt, "row-11/frame"), None);
        rt.pointer_move(1.0, 1.0);
        let renamed = "renamed-candidate-build-with-another-quite-long-tail-for-the-cut.zip";
        rt.update(|g, tx| {
            g.c3.renaming = None;
            let mut entries = g.c3.listing.entries().to_vec();
            entries[10].name = renamed.to_owned();
            g.c3.listing.replace(entries);
            g.c3.refresh(tx);
        });
        run(&mut rt, 0.2);
        assert_eq!(rest_on(&mut rt, "row-11/label").as_deref(), Some(renamed));
    }

    /// icons: "Keyboard focus on a card shows no name tooltip."
    #[test]
    fn a_key_moving_onto_a_cut_tile_shows_no_tooltip() {
        let mut rt = mount("c3-default-light");
        click(&mut rt, "row-10/frame", Mods::default());
        rt.pointer_move(1.0, 1.0);
        key(&mut rt, Key::Right, Mods::default());
        run(&mut rt, HELP_DELAY + 0.3);
        assert_eq!(selected(&rt), vec![11]);
        assert_eq!(rt.help_tooltip(), None);
    }

    /// icons: "Hover feedback appears only over a tile's thumbnail and caption, not the gutters
    /// between tiles."
    #[test]
    fn hover_lights_the_frame_or_the_name_and_not_the_cards_blank_space() {
        let mut rt = mount("c3-default-light");
        let hover = |rt: &Runtime<Gallery>, path: &str| element(rt, path).map_or(0.0, |e| e.hover);
        let (x, y) = center(&rt, "row-13/gap");
        rt.pointer_move(x, y);
        run(&mut rt, 0.3);
        assert_eq!(hover(&rt, "row-13/frame"), 0.0);
        assert_eq!(hover(&rt, "row-13/label"), 0.0);
        let (x, y) = center(&rt, "row-13/frame");
        rt.pointer_move(x, y);
        run(&mut rt, 0.3);
        assert!(hover(&rt, "row-13/frame") > 0.0);
        let (x, y) = center(&rt, "row-13/label");
        rt.pointer_move(x, y);
        run(&mut rt, 0.3);
        assert!(hover(&rt, "row-13/label") > 0.0);
        assert_eq!(hover(&rt, "row-13/frame"), 0.0);
    }

    /// view-modes: "Click the first of five entries and Ctrl+click the third. Shift+Down in
    /// List or Shift+Right in Icons then selects only the third and fourth, as in Columns."
    #[test]
    fn a_ctrl_click_then_shift_right_selects_the_third_and_fourth() {
        let mut rt = mount("c3-default-light");
        click(&mut rt, "row-1/frame", Mods::default());
        click(&mut rt, "row-3/frame", CTRL);
        assert_eq!(selected(&rt), vec![1, 3]);
        key(&mut rt, Key::Right, SHIFT);
        assert_eq!(selected(&rt), vec![3, 4]);
    }

    /// marquee: "A Ctrl-marquee toggles each crossed entry relative to the selection at press"
    /// and "A Shift-marquee adds the crossed entries to the selection at press", each over a
    /// block of tiles two columns by two rows.
    #[test]
    fn ctrl_and_shift_bands_toggle_and_add_a_block_of_tiles() {
        let band = |rt: &mut Runtime<Gallery>, mods: Mods, from: &str, to: &str| {
            let (x0, y0) = center(rt, from);
            let (x1, y1) = center(rt, to);
            rt.physical_input(PhysicalInput::Modifiers(mods));
            rt.pointer_down(x0, y0);
            rt.frame(FRAME);
            for i in 1..=6 {
                let f = i as f32 / 6.0;
                rt.pointer_move(x0 + (x1 - x0) * f, y0 + (y1 - y0) * f);
                rt.frame(FRAME);
            }
            rt.pointer_up(x1, y1);
            rt.frame(FRAME);
            rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
            run(rt, 0.3);
        };
        let mut rt = mount("c3-default-light");
        click(&mut rt, "row-3/frame", Mods::default());
        // Columns 0 and 1 of rows 0 and 1, added to entry 3.
        band(&mut rt, SHIFT, "row-1/gap", "row-10/gap");
        assert_eq!(selected(&rt), vec![1, 2, 3, 9, 10]);
        // Columns 2 and 3 of the same rows, from an unselected tile (a press on a selected
        // tile's blank space drags the selection): 3 goes, 4, 11 and 12 come.
        band(&mut rt, CTRL, "row-4/gap", "row-11/gap");
        assert_eq!(selected(&rt), vec![1, 2, 4, 9, 10, 11, 12]);
    }

    /// thumbnail-size, by wheel (Strata's Ctrl+Shift+wheel, lgse/strata#1721): a notch over the
    /// grid steps the size and doesn't scroll; a plain wheel scrolls.
    #[test]
    fn ctrl_shift_wheel_steps_the_size_and_a_plain_wheel_scrolls() {
        let mut rt = mount("c3-default-light");
        let (x, y) = center(&rt, "row-100/frame");
        let ctrl_shift = Mods {
            shift: true,
            ..CTRL
        };
        let notch = |rt: &mut Runtime<Gallery>, dy: f32, mods: Mods| {
            rt.wheel_event(
                WheelEvent::new(0.0, dy)
                    .at(x, y)
                    .mods(mods)
                    .precise(false)
                    .phase(GesturePhase::Moved),
            );
            run(rt, 0.3);
        };
        notch(&mut rt, 40.0, ctrl_shift);
        notch(&mut rt, 40.0, ctrl_shift);
        assert_eq!(rt.state.c3.size, 80.0, "two notches away: two steps larger");
        notch(&mut rt, -40.0, ctrl_shift);
        assert_eq!(rt.state.c3.size, 72.0);
        assert_eq!(
            rt.state.c3.coll.scroll_offset(),
            0.0,
            "and nothing scrolled"
        );
        notch(&mut rt, -120.0, Mods::default());
        assert!(
            rt.state.c3.coll.scroll_offset() > 0.0,
            "a plain wheel scrolls"
        );
        assert_eq!(rt.state.c3.size, 72.0);
    }

    /// icons: "Moving the slider scales existing thumbnails in place without flashing generic
    /// icons." A size step keeps every landed image held, its icon hidden, until the new
    /// step's image replaces it.
    #[test]
    fn a_size_step_keeps_landed_thumbnails_drawn_while_the_new_step_loads() {
        let mut rt = mount("c3-default-light");
        let shown: Vec<_> = {
            let d = &rt.state.c3;
            d.coll
                .visible_range()
                .filter_map(|i| d.listing.entry(i).thumbnail)
                .collect()
        };
        assert!(!shown.is_empty());
        let held = |rt: &Runtime<Gallery>| {
            shown.iter().all(|&k| {
                let d = &rt.state.c3;
                matches!(d.store.state(k), State::Ready { .. })
                    && d.store.thumb(k).icon_alpha().get() == 0.0
            })
        };
        assert!(held(&rt), "all landed before the step");
        let before = rt.state.c3.grid.synced_step();
        rt.update(|g, tx| {
            grid().apply_size(g, strata_components::icons_grid::SizeCommand::Step(9), tx);
        });
        for _ in 0..20 {
            rt.frame(FRAME);
            assert!(held(&rt), "an icon never shows in a landed tile's place");
        }
        assert_ne!(
            rt.state.c3.grid.synced_step(),
            before,
            "the store was asked for the new step"
        );
    }

    /// view-modes (folder-views, the host's part): a size the host changes itself (another
    /// folder's, a slider) lays out at once, and the cursor tile stays in view.
    #[test]
    fn a_size_the_host_sets_keeps_the_cursor_tile_in_view() {
        let mut rt = mount("c3-default-light");
        click(&mut rt, "row-120/frame", Mods::default());
        rt.update(|g, tx| {
            g.c3.size = 160.0;
            grid().size_changed(g, tx);
        });
        run(&mut rt, 0.5);
        let cursor = element(&rt, "row-120").expect("still mounted");
        let page = element(&rt, "c3-grid").expect("the grid");
        let [_, y, _, h] = cursor.visible;
        assert!(h > 0.0 && y >= page.visible[1], "the cursor tile shows");
    }

    /// X2's drop zones: a folder tile's path follows the grid's count a row.
    #[test]
    fn a_tiles_path_finds_it_at_any_width() {
        let mut rt = mount("c3-default-light");
        // A key path names keyed elements only, as Rune resolves one (`#n` is unkeyed).
        let finds = |rt: &Runtime<Gallery>, path: &str| {
            rt.elements().iter().any(|e| {
                let keyed: Vec<&str> = e.path.split('/').filter(|s| !s.starts_with('#')).collect();
                keyed.join("/").ends_with(path) && e.key.as_deref() == Some("row-13")
            })
        };
        let path = grid()
            .tile_path(&rt.state, EntryId::new(13))
            .expect("listed");
        assert!(finds(&rt, &path), "{path}");
        rt.update(|g, _| g.c3.pane = Some(400.0));
        run(&mut rt, 0.5);
        let narrow = grid()
            .tile_path(&rt.state, EntryId::new(13))
            .expect("listed");
        assert_ne!(narrow, path, "a reflow moves it to another line");
        assert!(finds(&rt, &narrow), "{narrow}");
    }
}
