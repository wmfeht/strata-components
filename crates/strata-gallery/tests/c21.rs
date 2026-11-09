//! C21's page in process: what scripts can't reach. The scripts in `tests/scripts/c21/`
//! check the rest. Each test names the bullets it covers.

mod c21 {
    use rune::{ElementInfo, Env, Key, KeyEvent, Mods, Runtime};
    use strata_gallery::{Gallery, scenes};

    const FRAME: f64 = 1.0 / 60.0;

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

    fn element(rt: &Runtime<Gallery>, key: &str) -> Option<ElementInfo> {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }

    fn focused(rt: &Runtime<Gallery>) -> Option<String> {
        rt.elements()
            .into_iter()
            .find(|e| e.focused && !e.exiting)
            .and_then(|e| e.key)
    }

    fn press_with(rt: &mut Runtime<Gallery>, key: Key, mods: Mods) {
        rt.key(KeyEvent {
            key: key.clone(),
            mods,
        });
        rt.frame(FRAME);
        rt.key_up(KeyEvent { key, mods });
        run(rt, 0.1);
    }

    fn press(rt: &mut Runtime<Gallery>, key: Key) {
        press_with(rt, key, Mods::default());
    }

    fn status(rt: &Runtime<Gallery>) -> String {
        rt.state.c21.status.clone()
    }

    fn tab_into_the_list(rt: &mut Runtime<Gallery>) {
        for _ in 0..3 {
            press(rt, Key::Tab);
        }
    }

    // browser/context-menu: "Menu or Shift+F10 with Hyper or Meta held opens the menu as
    // without them." Rune reports Super, Hyper and Meta alike as `logo`. The script
    // `menu-key-super` sends the keys with `"logo": true`; a script's `{"hold": ..}` reaches
    // pointer events only, not keys.
    #[test]
    fn menu_and_shift_f10_with_super_held_open_the_menu() {
        let logo = Mods {
            logo: true,
            ..Mods::default()
        };
        for (key, mods) in [
            (Key::ContextMenu, logo),
            (
                Key::Function(10),
                Mods {
                    shift: true,
                    ..logo
                },
            ),
        ] {
            let mut rt = mount("c21-start-light");
            tab_into_the_list(&mut rt);
            press_with(&mut rt, key.clone(), mods);
            assert_eq!(status(&rt), "Menu 1 on Notes.md", "{key:?}");
            assert!(element(&rt, "c21-menu-menu").is_some());
        }
    }

    // browser/context-menu: "A keyboard-opened item menu anchors to the entry, not the
    // pointer". The cursor's entry scrolled out of view is revealed first, and the menu opens
    // at it (where, the unit tests and the keyboard-anchor goldens show).
    #[test]
    fn the_menu_key_reveals_a_cursor_scrolled_out_of_view() {
        let mut rt = mount("c21-start-light");
        tab_into_the_list(&mut rt);
        press(&mut rt, Key::End);
        rt.update(|_, tx| {
            tx.scroll_to_instant(
                strata_components::collection::element::viewport_path(
                    strata_gallery::scenes::c21::SCOPE,
                ),
                0.0,
                0.0,
            )
        });
        run(&mut rt, 0.2);
        let row = element(&rt, "row-40");
        assert!(
            row.is_none_or(|r| r.visible[3] <= 0.0),
            "the cursor's row is out of view"
        );
        press(&mut rt, Key::ContextMenu);
        run(&mut rt, 0.5);
        assert_eq!(status(&rt), "Menu 1 on scan-36.pdf");
        let row = element(&rt, "row-40").expect("revealed");
        assert!(row.visible[3] > 0.0, "the cursor's row is back in view");
        let vp = rt.state.c21.coll.viewport();
        let at = rt.state.c21.menu.at().expect("open");
        assert!(
            at.y >= vp.y && at.y <= vp.y + vp.h,
            "the menu opens where the row is"
        );
    }

    // browser/context-menu: "A menu opens with keyboard focus on its first visible action".
    // After a right-click the first action has the keyboard but shows no highlight until a key
    // asks for it, as NSMenu (rune-kit's model): the first Down shows it there, the next moves.
    // Enter at once runs it.
    #[test]
    fn a_right_click_menu_has_the_keyboard_on_its_first_action() {
        let mut rt = mount("c21-nothing-light");
        let [x, y, w, h] = element(&rt, "row-5").unwrap().rect;
        rt.secondary_down(x + w / 2.0, y + h / 2.0);
        rt.frame(FRAME);
        rt.secondary_up(x + w / 2.0, y + h / 2.0);
        run(&mut rt, 0.2);
        assert_eq!(focused(&rt).as_deref(), Some("c21-menu-item3"));
        press(&mut rt, Key::Down);
        assert_eq!(
            focused(&rt).as_deref(),
            Some("c21-menu-item3"),
            "shown, not moved"
        );
        press(&mut rt, Key::Down);
        assert_eq!(focused(&rt).as_deref(), Some("c21-menu-item4"));
    }

    // The stand-in for C5's editor: its menu flag is set while the field menu is open, and
    // the menu's close drops it, as `menu_opened` and `menu_closed` would.
    #[test]
    fn the_rename_stand_ins_menu_flag_follows_the_field_menu() {
        let mut rt = mount("c21-start-light");
        let [x, y, w, h] = element(&rt, "c21-rename").unwrap().rect;
        rt.secondary_down(x + w / 2.0, y + h / 2.0);
        rt.frame(FRAME);
        rt.secondary_up(x + w / 2.0, y + h / 2.0);
        run(&mut rt, 0.2);
        assert!(rt.state.c21.rename_menu);
        assert!(element(&rt, "c21-field-menu-menu").is_some());
        press(&mut rt, Key::Escape);
        assert!(!rt.state.c21.rename_menu);
        assert_eq!(focused(&rt).as_deref(), Some("c21-rename"));
    }
}
