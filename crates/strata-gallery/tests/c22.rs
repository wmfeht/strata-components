//! C22's page in process: the peek's waits stepped from wake to wake in a resting window
//! (animations on and off), what the host hears, and what assistive technology hears of a
//! keyboard peek. The scripts in `tests/scripts/c22/` check the bullets' end states.

mod c22 {
    use rune::runtime::ElementInfo;
    use rune::{Env, Key, KeyEvent, Mods, Runtime};
    use strata_components::collection::element::row_key;
    use strata_components::contract::EntryId;
    use strata_components::folder_peek::{Origin, PeekStatus, card_key};
    use strata_gallery::scenes::c22::{ICONS, LIST, Mode};
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
        for _ in 0..60 {
            rt.frame(FRAME);
        }
        rt
    }

    fn element(rt: &Runtime<Gallery>, key: &str) -> Option<ElementInfo> {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }

    fn centre(rt: &Runtime<Gallery>, key: &str) -> (f32, f32) {
        let [x, y, w, h] = element(rt, key)
            .unwrap_or_else(|| panic!("no element {key}"))
            .visible;
        (x + w / 2.0, y + h / 2.0)
    }

    /// Moves the pointer to the middle of `key`, and runs the frame that hears it.
    fn point(rt: &mut Runtime<Gallery>, key: &str) {
        let (x, y) = centre(rt, key);
        rt.pointer_move(x, y);
        rt.frame(FRAME);
    }

    /// Moves the pointer to a blank spot below the rows.
    fn point_away(rt: &mut Runtime<Gallery>) {
        rt.pointer_move(200.0, 600.0);
        rt.frame(FRAME);
    }

    fn command(rt: &mut Runtime<Gallery>, c: char) {
        rt.key(KeyEvent {
            key: Key::Command(c),
            mods: Mods::default(),
        });
        rt.frame(FRAME);
    }

    /// Steps a still window for at most `limit` seconds: a frame only at each wake the page or
    /// Rune asked for (or for queued input), none in between and none for motion, which would
    /// hide a wait that asked for no wake (C7's review 2). Stops once `done` holds; returns the
    /// seconds from the start to the frame it stopped at.
    fn wake_until(rt: &mut Runtime<Gallery>, limit: f64, done: impl Fn(&Gallery) -> bool) -> f64 {
        let start = rt.state.c22.clock;
        for _ in 0..400 {
            let now = rt.state.c22.clock;
            if now - start > limit || done(&rt.state) {
                break;
            }
            if rt.has_pending_input() {
                rt.frame(FRAME);
                continue;
            }
            let Some(deadline) = rt.wake_deadline() else {
                break;
            };
            if deadline - start > limit {
                break;
            }
            rt.frame((deadline - now).max(0.001));
        }
        rt.state.c22.clock - start
    }

    fn open(g: &Gallery) -> Option<EntryId> {
        g.c22.peek.target().map(|t| t.folder)
    }

    /// Home's entry ids: Projects 1, Music 2, Photos 3, Empty 4, Locked 5, Slow 6,
    /// Archive 7, Downloads 8, then files.
    fn id(n: u64) -> EntryId {
        EntryId::new(n)
    }

    fn with_animations(on: bool) -> impl FnOnce(&mut Gallery) {
        move |g: &mut Gallery| g.look.animations = on
    }

    /// The row (List) or tile (Icons) of entry `n`.
    fn item(n: u64) -> String {
        row_key(id(n))
    }

    // preview/quick-preview/folder-peek: "With Folder peeking on, hovering a folder in Icons or
    // List for 500 ms opens a popover headed by the folder's name." In a resting window, which
    // runs frames only at the wakes asked for (C11's, C6's and C7's timer reviews), in both
    // views, with animations on and off; and the host's listing lands on its own wake.
    #[test]
    fn a_folder_peeks_500_ms_after_the_pointer_rests_on_it_in_a_resting_window() {
        for (scene, scope) in [("c22-list-light", LIST), ("c22-icons-light", ICONS)] {
            for animations in [true, false] {
                let mut rt = mount_with(scene, with_animations(animations));
                point(&mut rt, &item(2));
                let took = wake_until(&mut rt, 2.0, |g| open(g).is_some());
                assert!(
                    (0.48..0.52).contains(&took),
                    "{scene}, animations {animations}: opened after {took:.3} s"
                );
                let t = rt.state.c22.peek.target().unwrap();
                assert_eq!(
                    (t.folder, &*t.collection, t.origin),
                    (id(2), scope, Origin::Hover)
                );
                assert_eq!(rt.state.c22.heard, "Peek Music");
                let took = wake_until(&mut rt, 1.0, |g| g.c22.peeked.status == PeekStatus::Ready);
                assert!((0.09..0.12).contains(&took), "listed after {took:.3} s");
                rt.frame(FRAME);
                let heading = element(&rt, "peek-heading").expect("the card's heading");
                assert_eq!(heading.text.as_deref(), Some("Music"));
                assert!(element(&rt, &card_key(scope)).is_some());
            }
        }
    }

    // preview/quick-preview/folder-peek: "Moving the pointer from the folder onto the popover
    // keeps it open; leaving both closes it after 80 ms."
    #[test]
    fn the_card_holds_the_peek_and_leaving_both_closes_it_80_ms_later() {
        for animations in [true, false] {
            let mut rt = mount_with("c22-icons-light", with_animations(animations));
            point(&mut rt, &item(2));
            wake_until(&mut rt, 1.0, |g| open(g).is_some());
            rt.frame(FRAME);
            point(&mut rt, &card_key(ICONS));
            wake_until(&mut rt, 1.0, |_| false);
            assert_eq!(open(&rt.state), Some(id(2)), "the card holds it");

            // Back onto its folder, then off the card: still open.
            point(&mut rt, &item(2));
            wake_until(&mut rt, 1.0, |_| false);
            assert_eq!(open(&rt.state), Some(id(2)), "its folder holds it again");

            point_away(&mut rt);
            let took = wake_until(&mut rt, 1.0, |g| open(g).is_none());
            assert!(
                (0.06..0.1).contains(&took),
                "animations {animations}: closed after {took:.3} s"
            );
            assert_eq!(rt.state.c22.heard, "Peek closed");
            wake_until(&mut rt, 1.0, |_| false);
            assert_eq!(rt.wake_deadline(), None, "nothing left waiting");
        }
    }

    // preview/quick-preview/folder-peek: "Leaving a folder before the delay elapses opens no
    // peek." Nothing is left waiting either.
    #[test]
    fn a_pass_shorter_than_the_delay_opens_nothing_and_leaves_no_wake() {
        let mut rt = mount_with("c22-list-light", |_| {});
        point(&mut rt, &item(2));
        rt.frame(0.3);
        point(&mut rt, &item(9));
        wake_until(&mut rt, 2.0, |_| false);
        assert_eq!(open(&rt.state), None);
        assert_eq!(rt.wake_deadline(), None);
    }

    // preview/quick-preview/folder-peek: "Turning Folder peeking off closes an open peek and
    // cancels a pending one."
    #[test]
    fn turning_peeking_off_closes_the_peek_and_ends_a_pending_wait() {
        let mut rt = mount_with("c22-list-light", |_| {});
        point(&mut rt, &item(2));
        wake_until(&mut rt, 1.0, |g| open(g).is_some());
        command(&mut rt, 'p');
        assert_eq!(open(&rt.state), None);
        assert!(!rt.state.c22.peek.enabled());
        command(&mut rt, 'p');

        // Pending: off 0.2 s in, then the pointer stays.
        point(&mut rt, &item(3));
        rt.frame(0.2);
        command(&mut rt, 'p');
        wake_until(&mut rt, 2.0, |_| false);
        assert_eq!(open(&rt.state), None);
        // On again with the pointer still there: the row's wait starts over (its handler
        // came back, and Rune counts the pointer as arriving).
        command(&mut rt, 'p');
        let took = wake_until(&mut rt, 2.0, |g| open(g).is_some());
        assert!((0.48..0.52).contains(&took), "after {took:.3} s");
        assert_eq!(open(&rt.state), Some(id(3)));
    }

    // preview/quick-preview/folder-peek: "Columns never shows a folder peek, whatever the saved
    // preference, and switching to Columns closes an open or pending peek."
    #[test]
    fn switching_to_columns_closes_the_peek_and_columns_never_peeks() {
        let mut rt = mount_with("c22-list-light", |_| {});
        point(&mut rt, &item(2));
        wake_until(&mut rt, 1.0, |g| open(g).is_some());
        command(&mut rt, '3');
        // The commit unmounts the rows; Rune runs the leave it queued on the next frame.
        rt.frame(FRAME);
        assert_eq!(rt.state.c22.mode, Mode::Columns);
        assert_eq!(open(&rt.state), None);
        assert_eq!(rt.state.c22.heard, "Peek closed");

        // Pending, then Columns, where nothing waits on a folder.
        command(&mut rt, '1');
        rt.frame(FRAME);
        point(&mut rt, &item(3));
        rt.frame(0.2);
        command(&mut rt, '3');
        wake_until(&mut rt, 2.0, |_| false);
        assert_eq!(open(&rt.state), None);
        point(&mut rt, &item(2));
        wake_until(&mut rt, 2.0, |_| false);
        assert_eq!(open(&rt.state), None, "no peek in Columns");
    }

    // integration/10xer-mode: "In Icons and List, `i` on a directory toggles the folder peek
    // without moving focus" (the key is the page's, as Strata's 10xer keymap): it opens at
    // once, stays when the pointer is elsewhere, and is announced (owner ruling: a keyboard
    // peek is a polite live region). Escape closes it first.
    #[test]
    fn the_keyboard_peek_opens_at_once_is_announced_and_escape_closes_it_first() {
        let mut rt = mount_with("c22-list-light", |g| {
            let d = &mut g.c22;
            d.list.selection.select_only(&d.home, 1);
        });
        let i = KeyEvent {
            key: Key::Text("i".into()),
            mods: Mods::default(),
        };
        rt.key(i.clone());
        rt.frame(FRAME);
        let t = rt.state.c22.peek.target().expect("open at once");
        assert_eq!((t.folder, t.origin), (id(2), Origin::Keyboard));
        wake_until(&mut rt, 1.0, |g| g.c22.peeked.status == PeekStatus::Ready);
        rt.frame(FRAME);
        let tree = format!("{:?}", rt.accessibility_tree());
        assert!(tree.contains("Polite"), "a live region: {tree}");
        point_away(&mut rt);
        wake_until(&mut rt, 1.0, |_| false);
        assert_eq!(open(&rt.state), Some(id(2)), "the pointer doesn't close it");

        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Mods::default(),
        });
        rt.frame(FRAME);
        assert_eq!(open(&rt.state), None);
        assert_eq!(rt.state.page, Some("c22"), "Escape stopped at the peek");

        // `i` on a file opens nothing.
        rt.update(|g, _| {
            let d = &mut g.c22;
            d.list.selection.select_only(&d.home, 9);
        });
        rt.key(i);
        rt.frame(FRAME);
        assert_eq!(open(&rt.state), None);
    }

    // A click on the card opens its folder (Strata's commit_peek).
    #[test]
    fn a_click_on_the_card_opens_its_folder() {
        let mut rt = mount_with("c22-icons-light", |_| {});
        point(&mut rt, &item(2));
        wake_until(&mut rt, 1.0, |g| g.c22.peeked.status == PeekStatus::Ready);
        rt.frame(FRAME);
        let (x, y) = centre(&rt, "peek-open");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(FRAME);
        assert_eq!(rt.state.c22.heard, "Open Music");
        assert_eq!(open(&rt.state), None);
    }
}
