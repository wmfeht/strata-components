//! The index page: its buttons open pages from the keyboard, and with markers on, assistive
//! technology hears whether each page is built.

mod gallery {
    use rune::{Env, Key, KeyEvent, Mods, Runtime};
    use serde_json::Value;
    use strata_gallery::{Gallery, scenes};

    /// The index as the app opens it, or with `markers` off as its golden scenes are.
    fn index(markers: bool) -> Runtime<Gallery> {
        let mut g = Gallery::default();
        if !markers {
            g.index.markers = false;
        }
        let mut rt = strata_gallery::runtime(g, Env::new(false, 1.0));
        rt.frame(0.0);
        rt
    }

    fn press(rt: &mut Runtime<Gallery>, key: Key) {
        rt.key(KeyEvent {
            key,
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
    }

    fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
        if tree["key"] == key {
            return Some(tree);
        }
        tree["children"]
            .as_array()?
            .iter()
            .find_map(|c| node(c, key))
    }

    #[test]
    fn tab_then_enter_opens_the_first_page() {
        let mut rt = index(true);
        press(&mut rt, Key::Tab);
        press(&mut rt, Key::Enter);
        assert_eq!(rt.state.page, Some(scenes::PAGES[0].id));
    }

    #[test]
    fn space_opens_the_focused_page() {
        let mut rt = index(true);
        press(&mut rt, Key::Tab);
        press(&mut rt, Key::Tab);
        press(&mut rt, Key::Text(" ".into()));
        assert_eq!(rt.state.page, Some(scenes::PAGES[1].id));
    }

    #[test]
    fn in_the_app_each_button_says_whether_its_page_is_built() {
        let tree = index(true).accessibility_json();
        for p in scenes::PAGES {
            let button = node(&tree, &format!("index-{}", p.id))
                .unwrap_or_else(|| panic!("no button for {}", p.id));
            assert_eq!(button["role"], "button", "{}", p.id);
            let heard = if p.implemented() {
                "Implemented"
            } else {
                "Not built yet"
            };
            assert_eq!(button["description"], heard, "{}", p.id);
        }
        let built = scenes::PAGES.iter().filter(|p| p.implemented()).count();
        let count = node(&tree, "index-count").expect("the count");
        assert_eq!(
            count["name"],
            format!("{built} of {} implemented", scenes::PAGES.len())
        );
    }

    #[test]
    fn without_markers_the_index_says_nothing_about_status() {
        let tree = index(false).accessibility_json();
        let button = node(&tree, "index-w1").expect("a button");
        assert!(button.get("description").is_none(), "{button}");
        assert!(node(&tree, "index-summary").is_none());
        assert!(node(&tree, "index-count").is_none());
    }
}
