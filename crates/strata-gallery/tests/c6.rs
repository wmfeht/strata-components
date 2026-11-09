//! C6's page in process: what scripts can't reach (an input method composing, modifiers on a
//! middle click, the crumbs' scroll position, the overflow bar's fade, every frame of a reopen),
//! and window activation, on a host of the test's own whose root hears it (the gallery's root
//! is shared, so its pages can't). The scripts in `tests/scripts/c6/` check the rest. Each test
//! names the bullets it covers.

mod c6 {
    use rune::{
        Cx, El, ElementInfo, Env, Ime, Key, KeyEvent, Mods, PhysicalInput, PointerButton, Runtime,
        Tx, WheelEvent, col, el, row,
    };
    use strata_components::location_bar::{
        Candidate, Completions, Crumb, LocationBar, LocationBarState, LocationIntent, Polled, Trail,
    };
    use strata_gallery::{Gallery, scenes};
    use strata_kit::look::Look;

    const FRAME: f64 = 1.0 / 60.0;
    const LOC: &str = "c6-loc";

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
        run(&mut rt, 0.5);
        rt
    }

    fn mount(name: &str) -> Runtime<Gallery> {
        mount_with(name, |_| {})
    }

    fn run<S: 'static>(rt: &mut Runtime<S>, seconds: f64) {
        for _ in 0..(seconds / FRAME).round() as usize {
            rt.frame(FRAME);
        }
    }

    fn key<S: 'static>(rt: &mut Runtime<S>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        rt.frame(FRAME);
    }

    fn ctrl() -> Mods {
        Mods {
            ctrl: true,
            cmd: true,
            ..Mods::default()
        }
    }

    fn typ<S: 'static>(rt: &mut Runtime<S>, text: &str) {
        for c in text.chars() {
            key(rt, Key::Text(c.to_string()), Mods::default());
        }
    }

    fn element<S: 'static>(rt: &Runtime<S>, key: &str) -> Option<ElementInfo> {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }

    fn centre<S: 'static>(rt: &Runtime<S>, key: &str) -> (f32, f32) {
        let r = element(rt, key)
            .unwrap_or_else(|| panic!("no element {key}"))
            .rect;
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    /// Whether the element shows any of itself through the crumbs' viewport.
    fn shown<S: 'static>(rt: &Runtime<S>, key: &str) -> bool {
        element(rt, key).is_some_and(|e| e.visible[2] > 1.0 && e.visible[3] > 1.0)
    }

    fn middle_click(rt: &mut Runtime<Gallery>, key: &str, mods: Mods) {
        let (x, y) = centre(rt, key);
        rt.pointer_move(x, y);
        rt.physical_input(PhysicalInput::Modifiers(mods));
        rt.button_down(x, y, PointerButton::Middle);
        rt.frame(FRAME);
        rt.button_up(x, y, PointerButton::Middle);
        rt.frame(FRAME);
        rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
        rt.frame(FRAME);
    }

    fn wheel(rt: &mut Runtime<Gallery>, key: &str, dy: f32) {
        let (x, y) = centre(rt, key);
        rt.pointer_move(x, y);
        rt.wheel_event(WheelEvent::new(0.0, dy).at(x, y).precise(false));
        rt.frame(FRAME);
    }

    fn crumb(i: usize) -> String {
        format!("{LOC}-crumb-{i}")
    }

    /// The deep scene's last item: its current crumb.
    fn last_item(rt: &Runtime<Gallery>) -> String {
        format!("{LOC}-item-{}", rt.state.c6.trail.crumbs.len() - 1)
    }

    // browser/navigation/location-bar: "With an input method composing in the location field,
    // Escape cancels only the composition and the editor stays open." Rune gives Escape to the
    // composition first; the bar never hears it.
    #[test]
    fn escape_while_composing_keeps_the_entry_open() {
        let mut rt = mount("c6-short-light");
        key(&mut rt, Key::Command('l'), ctrl());
        rt.ime(Ime::Preedit("か".into(), Some((3, 3))));
        rt.frame(FRAME);
        key(&mut rt, Key::Escape, Mods::default());
        run(&mut rt, 0.2);
        let bar = &rt.state.c6.bar;
        assert!(bar.editing(), "the entry stays open");
        assert!(bar.entry_focused(), "and keeps the keyboard");
        assert_ne!(rt.state.c6.log, "Cancelled");
    }

    // browser/navigation/location-bar: "Ctrl+middle-click or Shift+middle-click on an ancestor
    // crumb opens that folder in a new window." Scripts' pointer events carry no modifiers.
    #[test]
    fn ctrl_or_shift_middle_click_opens_a_window() {
        for mods in [
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            Mods {
                shift: true,
                ..Mods::default()
            },
        ] {
            let mut rt = mount("c6-short-light");
            middle_click(&mut rt, &crumb(1), mods);
            assert_eq!(rt.state.c6.log, "New window ~/Projects", "{mods:?}");
            assert_eq!(
                rt.state.c6.cwd, "/home/demo/Projects/strata",
                "nothing navigates"
            );
        }
    }

    // browser/navigation/location-bar: "In a file chooser, middle-clicking a crumb opens
    // nothing."
    #[test]
    fn a_chooser_s_middle_click_opens_nothing() {
        let mut rt = mount_with("c6-short-light", |g| g.c6.chooser = true);
        middle_click(&mut rt, &crumb(1), Mods::default());
        middle_click(&mut rt, &crumb(0), Mods::default());
        assert_eq!(rt.state.c6.log, "");
        assert_eq!(rt.state.c6.cwd, "/home/demo/Projects/strata");
        assert!(!rt.state.c6.bar.editing());
    }

    // browser/navigation/location-bar: "When crumbs overflow, the wheel scrolls them
    // horizontally and edge fades mark the hidden part." The scroll position isn't in the
    // accessibility tree; the fades are the PNG golden's (`c6-deep`).
    #[test]
    fn the_wheel_scrolls_overflowing_crumbs() {
        let mut rt = mount("c6-deep-light");
        let current = last_item(&rt);
        assert!(
            shown(&rt, &current),
            "the deep path opens on its current crumb"
        );
        assert!(!shown(&rt, &crumb(0)), "and hides its start");
        for _ in 0..40 {
            wheel(&mut rt, &format!("{LOC}-crumbs"), 60.0);
        }
        run(&mut rt, 0.3);
        assert!(
            shown(&rt, &crumb(0)),
            "a plain wheel brings the start into view"
        );
        assert!(!shown(&rt, &current));
        for _ in 0..40 {
            wheel(&mut rt, &format!("{LOC}-crumbs"), -60.0);
        }
        run(&mut rt, 0.3);
        assert!(shown(&rt, &current), "and back to the end");
    }

    // browser/navigation/location-bar: "After navigation, or after submitting or cancelling the
    // entry, the crumbs scroll to show the current crumb."
    #[test]
    fn the_current_crumb_shows_after_a_navigation_a_submit_and_a_cancel() {
        let mut rt = mount("c6-deep-light");
        let away = |rt: &mut Runtime<Gallery>| {
            for _ in 0..40 {
                wheel(rt, &format!("{LOC}-crumbs"), 60.0);
            }
            run(rt, 0.3);
            assert!(!shown(rt, &last_item(rt)));
        };
        // Cancelled.
        away(&mut rt);
        key(&mut rt, Key::Command('l'), ctrl());
        key(&mut rt, Key::Escape, Mods::default());
        run(&mut rt, 0.3);
        assert!(shown(&rt, &last_item(&rt)), "after a cancel");
        // Submitted: an unknown path leaves the folder as it was.
        away(&mut rt);
        key(&mut rt, Key::Command('l'), ctrl());
        typ(&mut rt, "/nowhere");
        key(&mut rt, Key::Enter, Mods::default());
        run(&mut rt, 0.3);
        assert!(rt.state.c6.log.starts_with("Unable to open location"));
        assert!(shown(&rt, &last_item(&rt)), "after a submit");
        // Navigated: a crumb near the start opens, and its trail ends in view.
        away(&mut rt);
        let (x, y) = centre(&rt, &crumb(5));
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        run(&mut rt, 0.3);
        assert_eq!(rt.state.c6.trail.crumbs.len(), 6);
        assert!(shown(&rt, &last_item(&rt)), "after a navigation");
    }

    // browser/navigation/location-bar: "The overflow scrollbar sits in its own row below the
    // crumbs, hidden at rest and shown on hover or scroll, never covering labels." Where it
    // sits is the PNG golden's; here, when it shows.
    #[test]
    fn the_overflow_bar_shows_on_hover_and_scroll_only() {
        let bar = format!("{LOC}-crumbs-bar");
        let opacity = |rt: &Runtime<Gallery>| element(rt, &bar).map_or(0.0, |e| e.opacity);
        let mut rt = mount("c6-deep-light");
        let (x, y) = centre(&rt, "c6-files");
        rt.pointer_move(x, y);
        run(&mut rt, 2.5);
        assert!(opacity(&rt) < 0.01, "hidden at rest: {}", opacity(&rt));
        let (x, y) = centre(&rt, &format!("{LOC}-crumbs"));
        rt.pointer_move(x, y);
        run(&mut rt, 0.3);
        assert!(
            opacity(&rt) > 0.5,
            "shown while the pointer moves over the crumbs"
        );
        rt.pointer_move(x + 4.0, y);
        let (x, y) = centre(&rt, "c6-files");
        rt.pointer_move(x, y);
        run(&mut rt, 2.5);
        assert!(opacity(&rt) < 0.01, "hidden again");
        wheel(&mut rt, &format!("{LOC}-crumbs"), 60.0);
        let (x, y) = centre(&rt, "c6-files");
        rt.pointer_move(x, y);
        run(&mut rt, 0.2);
        assert!(opacity(&rt) > 0.5, "shown while the crumbs scroll");
        // A short path doesn't overflow: no bar at all.
        let mut rt = mount("c6-short-light");
        let (x, y) = centre(&rt, &format!("{LOC}-crumbs"));
        rt.pointer_move(x, y);
        run(&mut rt, 0.3);
        assert!(opacity(&rt) < 0.01);
    }

    // browser/navigation/location-bar: "Closing and reopening the editor shows no stale or
    // flashing suggestion list." Every frame after the reopen, until the new list lands.
    #[test]
    fn a_reopened_entry_never_shows_the_old_list() {
        let mut rt = mount("c6-short-light");
        key(&mut rt, Key::Command('l'), ctrl());
        typ(&mut rt, "~/D");
        run(&mut rt, 0.3);
        assert_eq!(rt.state.c6.bar.suggestions()[0].label, "Desktop/");
        key(&mut rt, Key::Escape, Mods::default());
        rt.state.c6.worker.clear();
        rt.key(KeyEvent {
            key: Key::Command('l'),
            mods: ctrl(),
        });
        for _ in 0..30 {
            rt.frame(FRAME);
            let bar = &rt.state.c6.bar;
            let old = bar.suggestions().iter().any(|c| c.label == "Desktop/");
            assert!(!(bar.suggestions_open() && old), "the old list showed");
            assert!(
                element(&rt, &format!("{LOC}-option-0"))
                    .is_none_or(|e| e.label.as_deref() != Some("Desktop/"))
            );
        }
        assert!(rt.state.c6.bar.suggestions_open(), "the fresh list landed");
    }

    // A paste is an edit: it asks for suggestions like typing does.
    #[test]
    fn a_paste_lists_suggestions() {
        let mut rt = mount("c6-short-light");
        key(&mut rt, Key::Command('l'), ctrl());
        rt.paste("~/Mu");
        run(&mut rt, 0.3);
        let bar = &rt.state.c6.bar;
        assert_eq!(bar.edit.text, "~/Mu");
        assert!(bar.suggestions_open());
        assert_eq!(bar.suggestions()[0].label, "Music/");
    }

    // A host whose root hears the window's activation, as Strata's window does.

    #[derive(Default)]
    struct Host {
        look: Look,
        bar: LocationBarState,
        trail: Trail,
        heard: Vec<String>,
        /// Answers, each with the frames left before it lands.
        due: Vec<(u32, Completions)>,
        delay: u32,
    }

    impl Host {
        fn route(&mut self, polled: Polled) -> bool {
            for intent in polled.intents {
                self.hear(intent);
            }
            polled.changed
        }

        fn hear(&mut self, intent: LocationIntent) {
            match intent {
                LocationIntent::Complete(q) => {
                    self.heard.push(format!("complete {}", q.text));
                    let found = ["Alpha", "Beta"]
                        .iter()
                        .map(|n| Candidate::new(format!("{n}/"), format!("{}{n}/", q.text)))
                        .collect();
                    self.due
                        .push((self.delay, Completions::new(q.generation, found)));
                }
                LocationIntent::CancelCompletion => self.heard.push("cancel".into()),
                other => self.heard.push(format!("{other:?}")),
            }
        }
    }

    fn host_view(h: &Host, cx: &mut Cx<Host>) -> El {
        let mut layer = Vec::new();
        let bar = LocationBar::new(
            "loc",
            &h.look,
            &h.trail,
            &h.bar,
            |h: &mut Host| &mut h.bar,
            |h: &mut Host, i, _: &mut Tx| h.hear(i),
        )
        .build(cx, &mut layer);
        rune_kit::root(
            col()
                .w_full()
                .h_full()
                .child(row().w_full().h(40.0).child(bar))
                .child(el().key("pane").w_full().h(200.0)),
            layer,
        )
        .on_window_focus(|h: &mut Host, focused, _: &mut Tx| {
            let polled = h.bar.set_window_focused(focused);
            h.route(polled);
        })
    }

    fn host_poll(h: &mut Host, now: f64, tx: &mut Tx) -> bool {
        let polled = h.bar.tick(now, tx);
        let mut changed = h.route(polled);
        let mut landed = Vec::new();
        h.due.retain_mut(|(left, answer)| {
            if *left == 0 {
                landed.push(answer.clone());
                false
            } else {
                *left -= 1;
                true
            }
        });
        // The host answers even a cancelled query: the bar must drop it.
        for answer in landed {
            let polled = h.bar.deliver(answer);
            changed |= h.route(polled);
        }
        changed
    }

    fn host(delay: u32) -> Runtime<Host> {
        let h = Host {
            look: Look::fallback(false),
            trail: Trail::new(
                vec![
                    Crumb::new("/home/demo", "~"),
                    Crumb::new("/home/demo/x", "x"),
                ],
                "~/x",
            ),
            delay,
            ..Host::default()
        };
        let mut rt = Runtime::new(h, host_view, (800.0, 300.0), Env::new(false, 1.0));
        rt.poll = Some(host_poll);
        rt.frame(0.0);
        rt.window_focus(true);
        rt.frame(FRAME);
        rt.update(|h, _| {
            let polled = h.bar.begin_edit("~/x");
            h.route(polled);
        });
        run(&mut rt, 0.2);
        rt
    }

    // browser/navigation/location-bar/completion: "Switching to another window, minimising,
    // or hiding Strata while suggestions show closes the popover. The typed path and location
    // editing stay." Minimising and hiding reach the window as deactivation too.
    #[test]
    fn losing_the_window_closes_the_suggestions_and_keeps_the_edit() {
        let mut rt = host(0);
        typ(&mut rt, "/a");
        run(&mut rt, 0.1);
        assert!(rt.state.bar.suggestions_open());
        rt.window_focus(false);
        run(&mut rt, 0.1);
        let bar = &rt.state.bar;
        assert!(!bar.suggestions_open());
        assert!(element(&rt, "loc-suggestions").is_none());
        assert!(bar.editing());
        assert_eq!(bar.edit.text, "/a");
    }

    // browser/navigation/location-bar/completion: "A listing still pending when the window
    // loses focus is cancelled, so its result does not reopen the popover." and "Refocusing
    // the window does not reopen suggestions; the next text change shows them again."
    #[test]
    fn a_listing_pending_at_deactivation_is_cancelled_and_refocus_reopens_nothing() {
        let mut rt = host(20);
        typ(&mut rt, "/a");
        rt.state.heard.clear();
        rt.window_focus(false);
        rt.frame(FRAME);
        assert_eq!(rt.state.heard, ["cancel"]);
        run(&mut rt, 0.6);
        assert!(rt.state.due.is_empty(), "the answer came anyway");
        assert!(!rt.state.bar.suggestions_open());
        rt.window_focus(true);
        run(&mut rt, 0.6);
        assert!(
            !rt.state.bar.suggestions_open(),
            "refocusing reopens nothing"
        );
        typ(&mut rt, "b");
        run(&mut rt, 0.6);
        assert!(
            rt.state.bar.suggestions_open(),
            "the next change shows them"
        );
    }

    // browser/navigation/location-bar/completion: "Editing the entry text while the window is
    // not focused shows no suggestions."
    #[test]
    fn editing_while_the_window_is_inactive_asks_for_nothing() {
        let mut rt = host(0);
        rt.window_focus(false);
        rt.frame(FRAME);
        rt.state.heard.clear();
        typ(&mut rt, "/a");
        run(&mut rt, 0.2);
        assert!(
            !rt.state.heard.iter().any(|h| h.starts_with("complete")),
            "{:?}",
            rt.state.heard
        );
        assert!(!rt.state.bar.suggestions_open());
        assert_eq!(rt.state.bar.edit.text, "/a");
    }

    // browser/navigation/location-bar/completion: "Hiding the location entry closes the
    // popover and cancels a pending listing." The host hides it (a tab switch) through
    // `dismiss_completion`; when is Strata's.
    #[test]
    fn dismissing_closes_the_list_and_cancels_the_listing() {
        let mut rt = host(0);
        typ(&mut rt, "/a");
        run(&mut rt, 0.1);
        assert!(rt.state.bar.suggestions_open());
        rt.state.delay = 20;
        typ(&mut rt, "b");
        rt.state.heard.clear();
        rt.update(|h, _| {
            let polled = h.bar.dismiss_completion();
            h.route(polled);
        });
        run(&mut rt, 0.6);
        assert_eq!(rt.state.heard, ["cancel"]);
        assert!(!rt.state.bar.suggestions_open());
        assert!(rt.state.bar.editing());
    }
}
