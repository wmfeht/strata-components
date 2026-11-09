//! C17's page in process: what scripts can't reach (an input method composing, every frame of
//! a superseded query, a window at rest stepped from wake to wake, a tooltip, the new-tab
//! chords and the middle click, the states with no feature-map bullet). The scripts in
//! `tests/scripts/c17/` check the rest. Each test names the bullets it covers, or says it
//! covers none.

mod c17 {
    use rune::{ElementInfo, Env, Ime, Key, KeyEvent, Mods, PointerButton, Runtime};
    use strata_components::contract::fuzzy;
    use strata_components::search_palette::{Phase, option_key};
    use strata_gallery::scenes::c17::KEY;
    use strata_gallery::{Gallery, scenes};

    const FRAME: f64 = 1.0 / 60.0;

    fn mount_with(name: &str, change: impl FnOnce(&mut Gallery)) -> Runtime<Gallery> {
        let scene = scenes::all_scenes()
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, f)| f)
            .unwrap_or_else(|| panic!("no scene {name}"));
        let mut g = Gallery::default();
        scene(&mut g);
        change(&mut g);
        let env = Env::new(g.dark, 1.0);
        let mut rt = strata_gallery::runtime(g, env);
        rt.frame(0.0);
        rt
    }

    fn mount(name: &str) -> Runtime<Gallery> {
        mount_with(name, |_| {})
    }

    fn run(rt: &mut Runtime<Gallery>, seconds: f64) {
        for _ in 0..(seconds / FRAME).round() as usize {
            rt.frame(FRAME);
        }
    }

    fn press(rt: &mut Runtime<Gallery>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        rt.frame(FRAME);
    }

    fn ctrl(shift: bool) -> Mods {
        Mods {
            ctrl: true,
            cmd: true,
            shift,
            ..Mods::default()
        }
    }

    fn typ(rt: &mut Runtime<Gallery>, text: &str) {
        for c in text.chars() {
            press(rt, Key::Text(c.to_string()), Mods::default());
        }
    }

    fn element(rt: &Runtime<Gallery>, key: &str) -> Option<ElementInfo> {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }

    fn centre(rt: &Runtime<Gallery>, key: &str) -> (f32, f32) {
        let r = element(rt, key)
            .unwrap_or_else(|| panic!("no element {key}"))
            .rect;
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    fn focused(rt: &Runtime<Gallery>) -> Option<String> {
        rt.elements()
            .into_iter()
            .find(|e| e.focused && !e.exiting)
            .and_then(|e| e.key)
    }

    /// The accessible name of the node keyed `key`.
    fn name(rt: &Runtime<Gallery>, key: &str) -> Option<String> {
        fn find(v: &serde_json::Value, key: &str) -> Option<String> {
            if v["key"] == key {
                return v["name"].as_str().map(str::to_owned);
            }
            v["children"].as_array()?.iter().find_map(|c| find(c, key))
        }
        find(&rt.accessibility_json(), key)
    }

    fn status(rt: &Runtime<Gallery>) -> Option<String> {
        name(rt, &format!("{KEY}-status"))
    }

    fn ids(rt: &Runtime<Gallery>) -> Vec<String> {
        rt.state
            .c17
            .palette
            .items()
            .iter()
            .map(|it| it.id.clone())
            .collect()
    }

    /// Steps a resting window: a frame only at each requested wake, none in between, for at
    /// most `limit` seconds, calling `each` after every frame. Reduced motion holds the
    /// spinner still, so the window rests between wakes.
    fn rest_with(rt: &mut Runtime<Gallery>, limit: f64, mut each: impl FnMut(&Runtime<Gallery>)) {
        let start = rt.state.c17.now;
        for _ in 0..400 {
            let Some(deadline) = rt.wake_deadline() else {
                break;
            };
            let now = rt.state.c17.now;
            if deadline - start > limit {
                break;
            }
            rt.frame((deadline - now).max(0.001));
            each(rt);
        }
    }

    fn resting(name: &str) -> Runtime<Gallery> {
        let mut rt = mount(name);
        rt.set_system_reduced_motion(true);
        run(&mut rt, 0.5);
        rt
    }

    // No bullet names it (the WP's "a superseded query's results never show"): a query typed
    // over another before its answers land shows none of them, in any frame, in a window at
    // rest stepped from wake to wake.
    #[test]
    fn a_superseded_query_s_answers_never_show() {
        let mut rt = resting("c17-closed-light");
        press(&mut rt, Key::Command('k'), ctrl(false));
        rest_with(&mut rt, 1.0, |_| {});
        typ(&mut rt, "re");
        // "rep" supersedes "re" (and "r") before their first batch lands.
        typ(&mut rt, "p");
        let mut saw = 0;
        rest_with(&mut rt, 2.0, |rt| {
            for it in rt.state.c17.palette.items() {
                assert!(
                    fuzzy("rep", &it.label).is_some() || fuzzy("rep", &it.detail).is_some(),
                    "{} answers a superseded query",
                    it.id
                );
            }
            saw = saw.max(rt.state.c17.palette.items().len());
        });
        assert!(saw > 0, "the current query's answers landed");
        assert_eq!(rt.state.c17.palette.phase(), Some(Phase::Results));
        assert!(
            !rt.state.c17.palette.busy(),
            "the last batch ended the walk"
        );
    }

    // browser/search: "A spinner shows in the search bar while indexing runs." In a window at
    // rest the three batches land at the wakes the fake worker asked for, and the count is
    // announced at the first results and when the query is done (owner ruling).
    #[test]
    fn streamed_batches_land_in_a_resting_window() {
        let mut rt = resting("c17-closed-light");
        press(&mut rt, Key::Command('k'), ctrl(false));
        typ(&mut rt, "notes");
        let busy = format!("{KEY}-busy");
        assert!(element(&rt, &busy).is_some(), "the spinner while it runs");
        let mut counts = Vec::new();
        rest_with(&mut rt, 1.0, |rt| {
            let n = rt.state.c17.palette.items().len();
            if counts.last() != Some(&n) {
                counts.push(n);
            }
        });
        assert!(counts.len() >= 3, "batches grew the list: {counts:?}");
        assert!(
            element(&rt, &busy).is_none(),
            "the spinner goes with the walk"
        );
        let n = rt.state.c17.palette.items().len();
        assert_eq!(status(&rt), Some(format!("{n} results")));
    }

    // No bullet names the loading or error states (the WP's "each state is distinct"): each
    // state's status reads its own words, results read a count.
    #[test]
    fn each_state_reads_its_own_words() {
        let at_one = |name: &str| {
            let mut rt = mount(name);
            run(&mut rt, 1.0);
            status(&rt).unwrap_or_default()
        };
        let idle = at_one("c17-idle-light");
        let loading = at_one("c17-loading-light");
        let results = at_one("c17-results-light");
        let none = at_one("c17-no-results-light");
        let unavailable = at_one("c17-error-light");
        let mut rt = mount("c17-loading-light");
        rt.update(|g, _| {
            let generation = g.c17.palette.generation();
            assert!(
                g.c17
                    .palette
                    .fail(generation, "The search index could not be read.")
            );
        });
        run(&mut rt, 0.1);
        let failed = status(&rt).unwrap_or_default();
        assert_eq!(idle, "Type to search Home and mounted local drives");
        assert_eq!(loading, "Searching…");
        assert!(results.ends_with(" results"), "{results}");
        assert_eq!(none, "No matching files or folders");
        assert_eq!(unavailable, "No local search locations available.");
        assert_eq!(failed, "The search index could not be read.");
        let mut all = vec![&idle, &loading, &results, &none, &unavailable, &failed];
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 6, "distinct");
    }

    // browser/search: "With a pinyin input method composing in the query field, Escape
    // cancels only the composition and search stays open."
    #[test]
    fn escape_while_composing_keeps_the_palette_open() {
        let mut rt = mount("c17-idle-light");
        run(&mut rt, 0.5);
        rt.ime(Ime::Preedit("ni".into(), None));
        rt.frame(FRAME);
        press(&mut rt, Key::Escape, Mods::default());
        assert!(rt.state.c17.palette.is_open());
        assert_eq!(
            focused(&rt).as_deref(),
            Some(format!("{KEY}-field").as_str())
        );
    }

    // browser/search: "With an input method composing in the query field, Enter that the input
    // method passes back after committing opens the highlighted result."
    #[test]
    fn an_enter_after_the_composition_commits_opens_the_highlighted_result() {
        let mut rt = mount("c17-results-light");
        run(&mut rt, 1.0);
        let first = ids(&rt)[0].clone();
        rt.ime(Ime::Preedit("ni".into(), None));
        rt.frame(FRAME);
        press(&mut rt, Key::Enter, Mods::default());
        assert!(rt.state.c17.palette.is_open(), "the composition took Enter");
        rt.ime(Ime::Commit(String::new()));
        rt.frame(FRAME);
        press(&mut rt, Key::Enter, Mods::default());
        assert_eq!(rt.state.c17.log, format!("Opened {first}"));
        assert!(!rt.state.c17.palette.is_open());
    }

    // No bullet: Strata's palette has no new-tab key (owner ruling, a deviation). The host's
    // open_in_new_tab chord (Ctrl+Enter, Shift ignored) and a middle click open in a new tab.
    #[test]
    fn ctrl_enter_and_a_middle_click_open_in_a_new_tab() {
        for mods in [ctrl(false), ctrl(true)] {
            let mut rt = mount("c17-results-light");
            run(&mut rt, 1.0);
            let first = ids(&rt)[0].clone();
            press(&mut rt, Key::Enter, mods);
            assert_eq!(rt.state.c17.log, format!("Opened {first} in a new tab"));
        }
        let mut rt = mount("c17-results-light");
        run(&mut rt, 1.0);
        let second = ids(&rt)[1].clone();
        let (x, y) = centre(&rt, &option_key(KEY, &second));
        rt.pointer_move(x, y);
        rt.button_down(x, y, PointerButton::Middle);
        rt.frame(FRAME);
        rt.button_up(x, y, PointerButton::Middle);
        rt.frame(FRAME);
        assert_eq!(rt.state.c17.log, format!("Opened {second} in a new tab"));
        // Alt+Ctrl+Enter is neither.
        let mut rt = mount("c17-results-light");
        run(&mut rt, 1.0);
        press(
            &mut rt,
            Key::Enter,
            Mods {
                alt: true,
                ..ctrl(false)
            },
        );
        assert!(rt.state.c17.palette.is_open());
    }

    // browser/search: "Hovering Partial results shows a tooltip with the same causes as its
    // description; complete results clear the tooltip."
    #[test]
    fn the_partial_notice_shows_its_causes_on_hover_until_results_are_complete() {
        let mut rt = mount("c17-partial-light");
        run(&mut rt, 1.0);
        let partial = format!("{KEY}-partial");
        let (x, y) = centre(&rt, &partial);
        rt.pointer_move(x, y);
        run(&mut rt, 1.0);
        let causes = "Some folders could not be read: ~/Private";
        assert_eq!(rt.help_tooltip().map(|(t, _)| t), Some(causes));
        // A rescan that completes: the same query's answer, complete.
        rt.update(|g, _| {
            let generation = g.c17.palette.generation();
            let items = g.c17.palette.items().to_vec();
            let answer = strata_components::search_palette::Answer::new(generation, items).done();
            assert!(g.c17.palette.deliver(answer).changed);
        });
        run(&mut rt, 1.0);
        assert!(element(&rt, &partial).is_none());
        assert_eq!(rt.help_tooltip(), None);
    }

    // browser/search: "Up and Down move the highlight while the caret stays in the query
    // field". The pointer only tints: a pointer resting on a row never moves the highlight,
    // even as the rows scroll under it (C6's review round 1).
    #[test]
    fn a_resting_pointer_never_moves_the_highlight() {
        let mut rt = mount("c17-results-light");
        run(&mut rt, 1.0);
        let third = ids(&rt)[3].clone();
        let (x, y) = centre(&rt, &option_key(KEY, &third));
        rt.pointer_move(x, y);
        run(&mut rt, 0.2);
        assert_eq!(rt.state.c17.palette.highlighted(), Some(0));
        for i in 1..12 {
            press(&mut rt, Key::Down, Mods::default());
            run(&mut rt, 0.2);
            assert_eq!(rt.state.c17.palette.highlighted(), Some(i));
        }
    }

    // browser/search: "Opening a result, or Open containing folder, hands focus to the
    // revealed item in the browser rather than the control that opened search." In a window
    // at rest: the leaving frame ends at the wake the palette asked for (probe P2).
    #[test]
    fn opening_a_result_hands_focus_to_the_view_in_a_resting_window() {
        let mut rt = resting("c17-closed-light");
        let (x, y) = centre(&rt, "c17-search");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(FRAME);
        typ(&mut rt, "notes");
        rest_with(&mut rt, 1.0, |_| {});
        press(&mut rt, Key::Enter, Mods::default());
        rest_with(&mut rt, 1.0, |_| {});
        assert!(!rt.state.c17.palette.is_open());
        assert!(element(&rt, &format!("{KEY}-dialog")).is_none(), "gone");
        assert_eq!(focused(&rt).as_deref(), Some("c17-files"));
        assert!(rt.state.c17.log.starts_with("Opened "));
    }

    // browser/search: "Each result shows its name, its full path, and a thumbnail for images."
    // The thumbnails land through the C25 store at the wakes the fake worker asks for.
    #[test]
    fn image_results_get_their_thumbnails_in_a_resting_window() {
        let mut rt = resting("c17-closed-light");
        press(&mut rt, Key::Command('k'), ctrl(false));
        typ(&mut rt, "trip-photo");
        rest_with(&mut rt, 2.0, |_| {});
        let d = &rt.state.c17;
        let key = d.palette.items()[0].thumb.expect("an image");
        assert!(
            matches!(
                d.thumbs.state(key),
                strata_components::thumbnails::State::Ready { .. }
            ),
            "{:?}",
            d.thumbs.stats()
        );
    }
}
