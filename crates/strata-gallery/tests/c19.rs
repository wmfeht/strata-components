//! C19's page in process: what the accessibility tree a script ends on doesn't show (scroll
//! offsets, the window losing focus, input-method text) and the large key map's cost. Each test
//! names the bullets it covers; the scripts in `tests/scripts/c19/` check the rest.

mod c19 {
    use std::time::Instant;

    use rune::{Env, Ime, Key, KeyEvent, Mods, Runtime};
    use strata_gallery::{Gallery, scenes};

    const FRAME: f64 = 1.0 / 60.0;
    const RESULTS: &str = "c19-reference-results";
    const SEARCH: &str = "c19-reference-search";

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
        run(&mut rt, 0.5);
        rt
    }

    fn run(rt: &mut Runtime<Gallery>, seconds: f64) {
        for _ in 0..(seconds / FRAME).round() as usize {
            rt.frame(FRAME);
        }
    }

    fn press(rt: &mut Runtime<Gallery>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        run(rt, 0.1);
    }

    fn plain(rt: &mut Runtime<Gallery>, key: Key) {
        press(rt, key, Mods::default());
    }

    fn ctrl(rt: &mut Runtime<Gallery>, c: char) {
        press(
            rt,
            Key::Command(c),
            Mods {
                cmd: true,
                ctrl: true,
                ..Mods::default()
            },
        );
    }

    fn focused(rt: &Runtime<Gallery>) -> Option<String> {
        rt.elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key)
    }

    /// The results' vertical scroll offset, as the accessibility tree reports it.
    fn offset(rt: &Runtime<Gallery>) -> f64 {
        fn find(n: &serde_json::Value) -> Option<f64> {
            if n["key"] == RESULTS {
                return n["scroll"][1].as_f64();
            }
            n["children"].as_array()?.iter().find_map(find)
        }
        find(&rt.accessibility_json()).unwrap_or_else(|| panic!("no {RESULTS} in the tree"))
    }

    fn mounted_rows(rt: &Runtime<Gallery>) -> usize {
        rt.elements()
            .into_iter()
            .filter(|e| {
                e.key
                    .as_deref()
                    .is_some_and(|k| k.starts_with("c19-reference-row-"))
            })
            .count()
    }

    /// `app/shortcut-reference`: "In the results, arrows, j/k, Page Up, Page Down, Home, and End
    /// scroll the list" and "With the search field focused, Up, Down, Page Up, and Page Down
    /// scroll the results without moving focus".
    #[test]
    fn the_results_keys_scroll_the_list_and_the_search_field_scrolls_it_in_place() {
        let mut rt = mount("c19-reference-light");
        assert_eq!(focused(&rt).as_deref(), Some(SEARCH));
        plain(&mut rt, Key::Down);
        plain(&mut rt, Key::Down);
        let after_two = offset(&rt);
        assert!(after_two > 0.0, "Down in the search field scrolls");
        assert_eq!(focused(&rt).as_deref(), Some(SEARCH), "and keeps focus");
        plain(&mut rt, Key::PageDown);
        let paged = offset(&rt);
        assert!(paged > after_two, "Page Down in the field scrolls a page");
        plain(&mut rt, Key::PageUp);
        assert!(offset(&rt) < paged);

        ctrl(&mut rt, 'l');
        assert_eq!(focused(&rt).as_deref(), Some(RESULTS));
        plain(&mut rt, Key::Home);
        assert_eq!(offset(&rt), 0.0);
        for k in ["j", "j", "j"] {
            plain(&mut rt, Key::Text(k.into()));
        }
        let down = offset(&rt);
        assert!(down > 0.0, "j scrolls down");
        plain(&mut rt, Key::Text("k".into()));
        assert!(offset(&rt) < down, "k scrolls up");
        plain(&mut rt, Key::Down);
        plain(&mut rt, Key::Up);
        plain(&mut rt, Key::End);
        let end = offset(&rt);
        plain(&mut rt, Key::PageDown);
        assert_eq!(offset(&rt), end, "End reaches the bottom");
        plain(&mut rt, Key::Home);
        assert_eq!(offset(&rt), 0.0);
        assert_eq!(focused(&rt).as_deref(), Some(RESULTS));
    }

    /// `app/shortcut-reference`: "Opening the reference focuses … and scrolls the list to the
    /// top" and "Reopening the reference keeps the previous search text and selected category".
    #[test]
    fn reopening_scrolls_the_results_to_the_top_and_keeps_the_query() {
        let mut rt = mount("c19-reference-light");
        plain(&mut rt, Key::Text("e".into()));
        plain(&mut rt, Key::PageDown);
        assert!(offset(&rt) > 0.0);
        plain(&mut rt, Key::Escape);
        run(&mut rt, 0.5);
        assert!(!rt.state.c19.reference.is_open());
        plain(&mut rt, Key::Function(1));
        run(&mut rt, 0.5);
        assert!(rt.state.c19.reference.is_open());
        assert_eq!(offset(&rt), 0.0);
        assert_eq!(focused(&rt).as_deref(), Some(SEARCH));
        assert_eq!(rt.state.c19.reference.query(), "e");
    }

    /// `app/shortcut-reference`: "Focusing another application window leaves the reference
    /// open with its search text, category, and scroll position" (scripts have no window-focus
    /// event: Rune draft U-X1-1).
    #[test]
    fn the_window_losing_focus_leaves_the_reference_as_it_was() {
        let mut rt = mount("c19-reference-light");
        plain(&mut rt, Key::Text("i".into()));
        plain(&mut rt, Key::PageDown);
        let at = offset(&rt);
        rt.state
            .c19
            .reference
            .set_category(Some("Files and selection".into()));
        run(&mut rt, 0.1);
        rt.window_focus(false);
        run(&mut rt, 0.5);
        rt.window_focus(true);
        run(&mut rt, 0.2);
        let r = &rt.state.c19.reference;
        assert!(r.is_open());
        assert_eq!(r.query(), "i");
        assert_eq!(r.category(), Some("Files and selection"));
        assert!(at > 0.0);
        assert_eq!(offset(&rt), at, "the scroll position stays");
    }

    /// `app/shortcut-reference`: "With fcitx5 … Chinese can be typed there" (the input method's
    /// own switch key is the input method's; scripts have no input-method event, U-C6-7).
    #[test]
    fn an_input_method_types_into_the_search() {
        let mut rt = mount("c19-reference-light");
        rt.ime(Ime::Preedit("zhong".into(), None));
        run(&mut rt, 0.1);
        rt.ime(Ime::Commit("中文".into()));
        run(&mut rt, 0.1);
        assert_eq!(rt.state.c19.reference.query(), "中文");
        assert!(rt.state.c19.reference.is_open());
    }

    /// The plan's performance check: with 5,000 rows, a keystroke builds only the rows on
    /// screen. Timings print with `--nocapture`; they aren't asserted.
    #[test]
    fn a_keystroke_in_a_5000_row_reference_builds_only_the_rows_on_screen() {
        let mut rt = mount("c19-reference-large-light");
        let before = mounted_rows(&rt);
        assert!(before > 0 && before < 120, "{before} rows mounted");
        let mut times = Vec::new();
        for c in ["1", "2", "3"] {
            let t = Instant::now();
            rt.key(KeyEvent {
                key: Key::Text(c.into()),
                mods: Mods::default(),
            });
            rt.frame(FRAME);
            times.push(t.elapsed());
            run(&mut rt, 0.1);
            assert!(
                mounted_rows(&rt) < 120,
                "{} rows mounted",
                mounted_rows(&rt)
            );
        }
        let t = Instant::now();
        for _ in 0..3 {
            rt.key(KeyEvent {
                key: Key::Backspace,
                mods: Mods::default(),
            });
            rt.frame(FRAME);
        }
        let clear = t.elapsed() / 3;
        println!("c19 perf: 5,000 rows, keystroke + frame {times:?}, backspace + frame {clear:?}");
        assert_eq!(rt.state.c19.reference.query(), "");
    }
}
