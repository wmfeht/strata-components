//! C9's page: what the drawer does in process, beside its goldens and scripts.

mod c9 {
    use rune::{Env, Key, KeyEvent, Mods, Runtime};
    use serde_json::Value;
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
        settle(&mut rt, 10);
        rt
    }

    fn settle(rt: &mut Runtime<Gallery>, frames: usize) {
        for _ in 0..frames {
            rt.frame(DT);
        }
    }

    fn press(rt: &mut Runtime<Gallery>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        settle(rt, 2);
    }

    fn key(rt: &mut Runtime<Gallery>, key: Key) {
        press(rt, key, Mods::default());
    }

    fn tap(rt: &mut Runtime<Gallery>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        rt.pointer_up(x, y);
        settle(rt, 2);
    }

    fn typed(rt: &mut Runtime<Gallery>, s: &str) {
        for c in s.chars() {
            key(rt, Key::Text(c.to_string()));
        }
    }

    fn focused(rt: &Runtime<Gallery>) -> Option<String> {
        rt.elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key)
    }

    /// The accessibility node keyed `key`, as assistive technology reads it.
    fn node(rt: &Runtime<Gallery>, key: &str) -> Option<Value> {
        fn find(v: &Value, key: &str) -> Option<Value> {
            if v["key"] == key {
                return Some(v.clone());
            }
            v["children"].as_array()?.iter().find_map(|c| find(c, key))
        }
        let tree = rt.accessibility_json();
        find(&tree, key)
    }

    fn value(rt: &Runtime<Gallery>) -> String {
        node(rt, "preview-grip").expect("a grip")["value"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    fn exiting(rt: &Runtime<Gallery>, key: &str) -> Option<bool> {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map(|e| e.exiting)
    }

    #[test]
    fn left_from_the_content_returns_focus_to_the_cursor_row() {
        let mut rt = scene("c9-list-open-light");
        key(&mut rt, Key::Tab);
        key(&mut rt, Key::Right);
        assert_eq!(focused(&rt).as_deref(), Some("fake-content"));
        key(&mut rt, Key::Left);
        assert_eq!(focused(&rt).as_deref(), Some("row-1"));
    }

    #[test]
    fn the_find_keys_open_find_only_while_the_view_is_searchable() {
        let mut rt = scene("c9-list-open-light");
        key(&mut rt, Key::Tab);
        key(&mut rt, Key::Right);
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        press(&mut rt, Key::Command('f'), ctrl);
        assert!(rt.state.c9.drawer.is_finding());
        assert_eq!(
            focused(&rt).as_deref(),
            Some("input"),
            "the query field takes focus"
        );
        typed(&mut rt, "drawer");
        key(&mut rt, Key::Escape);
        assert!(!rt.state.c9.drawer.is_finding());
        assert_eq!(
            focused(&rt).as_deref(),
            Some("fake-content"),
            "keys go back to the document"
        );
        assert_eq!(
            rt.state.c9.drawer.find_query(),
            "drawer",
            "the query outlives the bar"
        );

        // Rendered, the fake document is table-only: no find, and the keys pass on.
        tap(&mut rt, "preview-view-switch");
        tap(&mut rt, "fake-content");
        press(&mut rt, Key::Command('f'), ctrl);
        assert!(!rt.state.c9.drawer.is_finding());
    }

    #[test]
    fn the_close_button_closes_reports_it_and_ends_find() {
        let mut rt = scene("c9-list-open-light");
        tap(&mut rt, "preview-find");
        typed(&mut rt, "preview");
        assert_eq!(rt.state.c9.view_status().map(|s| s.total), Some(5));
        tap(&mut rt, "preview-close");
        let d = &rt.state.c9;
        assert!(!d.drawer.is_open());
        assert!(
            d.dismissed,
            "Strata hears the close and marks the preview dismissed"
        );
        assert_eq!(
            d.view_status().map(|s| s.total),
            Some(0),
            "the view drops its matches"
        );
        assert_eq!(d.drawer.find_query(), "");
    }

    #[test]
    fn the_wrap_and_view_controls_report_their_intents() {
        let mut rt = scene("c9-list-open-light");
        let states = |rt: &Runtime<Gallery>| node(rt, "preview-wrap").map(|n| n["states"].clone());
        assert!(states(&rt).is_some_and(|s| s.to_string().contains("checked")));
        tap(&mut rt, "preview-wrap");
        assert!(!rt.state.c9.view.wrap);
        assert!(!states(&rt).is_some_and(|s| s.to_string().contains("\"checked\"")));
        tap(&mut rt, "preview-view-switch");
        assert_eq!(
            node(&rt, "preview-view-switch").unwrap()["name"],
            "View source"
        );
    }

    #[test]
    fn a_window_resize_lands_the_automatic_width_in_one_frame() {
        let mut rt = scene("c9-columns-open-light");
        assert_eq!(value(&rt), "359");
        rt.set_viewport((1100.0, 640.0), 1.0);
        rt.frame(DT);
        assert_eq!(value(&rt), "499", "the frame the window resizes");
        settle(&mut rt, 20);
        assert_eq!(value(&rt), "499", "and it stays");
        assert_eq!(exiting(&rt, "preview-drawer"), Some(false));
    }

    #[test]
    fn at_four_times_scale_the_preview_holds_its_layout() {
        let f = scenes::all_scenes()
            .into_iter()
            .find(|(n, _)| *n == "c9-columns-open-light")
            .unwrap()
            .1;
        let mut g = Gallery::default();
        f(&mut g);
        let mut rt = strata_gallery::runtime(g, Env::new(false, 4.0));
        rt.frame(0.0);
        settle(&mut rt, 5);
        let first = value(&rt);
        let commits = rt.stats.commits;
        settle(&mut rt, 60);
        assert_eq!(value(&rt), first);
        assert_eq!(
            rt.stats.commits, commits,
            "no layout alternates with another"
        );
    }

    #[test]
    fn closing_keeps_the_content_inert_until_the_slide_ends_and_reopening_turns_back() {
        let mut rt = scene("c9-list-open-light");
        key(&mut rt, Key::Text(" ".into()));
        assert_eq!(
            exiting(&rt, "preview-drawer"),
            Some(true),
            "the content leaves with the slide"
        );
        settle(&mut rt, 40);
        assert_eq!(
            exiting(&rt, "preview-drawer"),
            None,
            "and goes when it ends"
        );

        key(&mut rt, Key::Text(" ".into()));
        settle(&mut rt, 40);
        key(&mut rt, Key::Text(" ".into()));
        settle(&mut rt, 4);
        key(&mut rt, Key::Text(" ".into()));
        assert_eq!(
            exiting(&rt, "preview-drawer"),
            Some(false),
            "reopened mid-slide, it stays"
        );
        settle(&mut rt, 40);
        assert!(node(&rt, "preview-drawer").is_some());
    }
}
