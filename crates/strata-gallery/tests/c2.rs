//! C2's page: what the columns view does in process, beside its goldens and scripts. Each test
//! names the feature-map Behavior bullets it checks, where a script can't see them: scroll
//! offsets, widths, timing, modifiers held on a pointer, and paint.

mod c2 {
    use rune::{ElementInfo, Env, Key, KeyEvent, Mods, PhysicalInput, Runtime};
    use serde_json::Value;
    use strata_components::columns::keys;
    use strata_components::contract::EntrySource;
    use strata_gallery::Gallery;
    use strata_gallery::scenes::{self, c2::id_in};

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
        settle(&mut rt, 40);
        rt
    }

    fn settle(rt: &mut Runtime<Gallery>, frames: usize) {
        for _ in 0..frames {
            rt.frame(DT);
        }
    }

    fn info(rt: &Runtime<Gallery>, key: &str) -> Option<ElementInfo> {
        rt.elements()
            .into_iter()
            .find(|e| !e.exiting && e.key.as_deref() == Some(key))
    }

    fn at(rt: &Runtime<Gallery>, key: &str) -> (f32, f32) {
        let [x, y, w, h] = info(rt, key)
            .unwrap_or_else(|| panic!("no element {key}"))
            .visible;
        (x + w / 2.0, y + h / 2.0)
    }

    fn click_at(rt: &mut Runtime<Gallery>, x: f32, y: f32) {
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        rt.pointer_up(x, y);
        rt.frame(DT);
    }

    fn tap(rt: &mut Runtime<Gallery>, key: &str) {
        let (x, y) = at(rt, key);
        click_at(rt, x, y);
        settle(rt, 2);
    }

    /// A press on `key`'s name, near the start of its row.
    fn tap_name(rt: &mut Runtime<Gallery>, key: &str) {
        let [x, y, _, h] = info(rt, key).expect("row").visible;
        click_at(rt, x + 40.0, y + h / 2.0);
        settle(rt, 2);
    }

    fn press(rt: &mut Runtime<Gallery>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        rt.frame(DT);
        rt.frame(DT);
    }

    fn key(rt: &mut Runtime<Gallery>, k: Key) {
        press(rt, k, Mods::default());
    }

    fn ctrl() -> Mods {
        Mods {
            ctrl: true,
            ..Mods::default()
        }
    }

    fn row(parent: &str, name: &str) -> String {
        format!("row-{}", id_in(parent, name).get())
    }

    fn path(rt: &Runtime<Gallery>) -> String {
        rt.state
            .c2
            .listings
            .0
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join("/")
    }

    fn selected(rt: &Runtime<Gallery>, depth: usize) -> Vec<u64> {
        let d = &rt.state.c2;
        let c = &d.columns.column(depth).expect("column").collection;
        c.selection
            .selected_ids(&d.listings.0[depth].source)
            .into_iter()
            .map(|id| id.get())
            .collect()
    }

    fn cursor(rt: &Runtime<Gallery>, depth: usize) -> Option<u64> {
        let c = &rt
            .state
            .c2
            .columns
            .column(depth)
            .expect("column")
            .collection;
        c.selection.cursor().map(|id| id.get())
    }

    fn id(parent: &str, name: &str) -> u64 {
        id_in(parent, name).get()
    }

    /// The accessibility node keyed `key`.
    fn node(rt: &Runtime<Gallery>, key: &str) -> Option<Value> {
        fn find(v: &Value, key: &str) -> Option<Value> {
            if v["key"] == key {
                return Some(v.clone());
            }
            v["children"].as_array()?.iter().find_map(|c| find(c, key))
        }
        find(&rt.accessibility_json(), key)
    }

    fn focus_strip(rt: &mut Runtime<Gallery>) {
        // A press on the blank space after the last column focuses the strip and clears the
        // selection; a press on a column's title focuses it and keeps it.
        let (x, y) = at(rt, &keys::title("c2-col-1"));
        click_at(rt, x, y);
    }

    #[test]
    fn a_click_on_a_folder_opens_it_as_the_next_column() {
        // browser/view-modes/columns: With default click settings, a single click on a folder
        // opens it as the next column.
        let mut rt = scene("c2-home-light");
        tap_name(&mut rt, &row("Home", "Pictures"));
        assert_eq!(path(&rt), "Home/Pictures");
        assert_eq!(rt.state.c2.columns.active(), 1, "the new column is active");
        assert_eq!(cursor(&rt, 1), None, "with nothing selected in it");
    }

    #[test]
    fn right_and_left_move_the_cursor_between_columns_and_only_one_shows_it() {
        let mut rt = scene("c2-keyboard-light");
        focus_strip(&mut rt);
        assert_eq!(rt.state.c2.columns.active(), 0);
        // Right on Pictures, whose column is open: into it, keeping its multi-selection.
        key(&mut rt, Key::Right);
        assert_eq!(rt.state.c2.columns.active(), 1);
        assert_eq!(selected(&rt, 1).len(), 4, "its multi-selection stays");
        // Down moves Pictures' cursor, not Home's.
        let home = cursor(&rt, 0);
        key(&mut rt, Key::Down);
        assert_eq!(cursor(&rt, 0), home);
        // Left returns to Home, whose cursor stays on Pictures.
        key(&mut rt, Key::Left);
        assert_eq!(rt.state.c2.columns.active(), 0);
        assert_eq!(cursor(&rt, 0), Some(id("Home", "Pictures")));
        let strip = node(&rt, "c2").expect("strip");
        assert_eq!(strip["role"], "group");
        let pictures = format!("row-{}", id("Home", "Pictures"));
        assert_eq!(strip["activeDescendant"], pictures.as_str(), "{strip}");
    }

    #[test]
    fn the_close_button_closes_its_column_and_it_narrows_away() {
        let mut rt = scene("c2-three-columns-light");
        assert_eq!(path(&rt), "Home/Documents/Reports");
        tap(&mut rt, &keys::close("c2-col-2"));
        assert_eq!(path(&rt), "Home");
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.exiting && e.key.as_deref() == Some("c2-slot-2")),
            "it leaves on a transition"
        );
        settle(&mut rt, 40);
        assert!(
            !rt.elements()
                .iter()
                .any(|e| e.key.as_deref() == Some("c2-slot-2")),
            "then it's gone"
        );
        let _ = ctrl();
        let _ = PhysicalInput::Modifiers(Mods::default());
    }

    #[test]
    fn a_click_on_a_folder_whose_column_is_open_focuses_and_reveals_that_column() {
        // browser/view-modes/columns: Clicking a folder whose child column is already open
        // focuses and reveals that column instead of closing it.
        let mut rt = scene("c2-three-columns-light");
        tap(&mut rt, &keys::title("c2-col-1"));
        assert_eq!(rt.state.c2.columns.active(), 0);
        // On its icon: a click on the name of the folder selected alone renames it (below).
        let [x, y, _, h] = info(&rt, &row("Home", "Documents")).expect("row").visible;
        click_at(&mut rt, x + 18.0, y + h / 2.0);
        settle(&mut rt, 2);
        assert_eq!(path(&rt), "Home/Documents/Reports", "nothing closes");
        assert_eq!(
            rt.state.c2.columns.active(),
            1,
            "Documents' column is active"
        );
        assert_ne!(
            rt.state.c2.intent, "Opened Documents",
            "the host isn't asked to open it"
        );
    }

    #[test]
    fn a_slow_second_click_on_an_open_folders_name_still_renames_it() {
        // browser/view-modes/columns: A slow second click on the name of a selected, open
        // folder still starts a rename.
        let mut rt = scene("c2-three-columns-light");
        tap_name(&mut rt, &row("Home", "Documents"));
        settle(&mut rt, 30);
        assert_eq!(rt.state.c2.intent, "Renaming Documents");
        assert_eq!(path(&rt), "Home/Documents/Reports");
    }

    #[test]
    fn escape_clears_the_active_column_then_closes_the_deepest() {
        // browser/selection: In Columns, Escape clears only the active column; parent
        // selections and open columns stay. And: Outside 10xer mode, in Columns, Escape with
        // nothing selected in the active column closes the deepest column.
        let mut rt = scene("c2-three-columns-light");
        tap_name(&mut rt, &row("Documents", "cv.pdf"));
        // The click on a file closes the deeper column (the page's stand-in): reopen Reports.
        tap_name(&mut rt, &row("Documents", "Reports"));
        tap_name(&mut rt, &row("Reports", "report-q0002.pdf"));
        assert_eq!(rt.state.c2.columns.active(), 2);
        key(&mut rt, Key::Escape);
        assert!(selected(&rt, 2).is_empty());
        assert_eq!(
            selected(&rt, 1),
            [id("Documents", "Reports")],
            "the parent keeps its own"
        );
        assert_eq!(path(&rt), "Home/Documents/Reports");
        key(&mut rt, Key::Escape);
        assert_eq!(
            path(&rt),
            "Home/Documents",
            "nothing selected: the deepest closes"
        );
    }

    #[test]
    fn a_click_on_empty_space_in_an_inactive_column_closes_deeper_and_picks_its_first_entry() {
        // browser/view-modes/columns: Clicking empty space in an inactive column closes deeper
        // columns, focuses and reveals that column, and selects its first visible entry.
        let mut rt = scene("c2-three-columns-light");
        let [x, y, w, h] = info(&rt, "c2-slot-1").expect("Home").visible;
        click_at(&mut rt, x + w / 2.0, y + h - 20.0);
        settle(&mut rt, 2);
        assert_eq!(path(&rt), "Home");
        assert_eq!(rt.state.c2.columns.active(), 0);
        assert_eq!(selected(&rt, 0), [id("Home", "Documents")]);
    }

    #[test]
    fn a_click_on_empty_space_in_the_active_column_clears_every_column() {
        // browser/view-modes/columns: Clicking empty space in the active column clears the
        // selection in every column and closes deeper columns.
        let mut rt = scene("c2-three-columns-light");
        assert!(!selected(&rt, 0).is_empty());
        let [x, y, w, h] = info(&rt, "c2-slot-2").expect("Documents").visible;
        // Below Documents' 27 rows there is no blank space at this size: use Reports.
        let _ = (x, y, w, h);
        tap_name(&mut rt, &row("Reports", "report-q0001.pdf"));
        let [x, y, w, h] = info(&rt, "c2-slot-3").expect("Reports").visible;
        click_at(&mut rt, x + w / 2.0, y + h - 20.0);
        settle(&mut rt, 2);
        for depth in 0..3 {
            assert!(selected(&rt, depth).is_empty(), "column {depth}");
        }
        assert_eq!(
            path(&rt),
            "Home/Documents/Reports",
            "no deeper column to close"
        );
    }

    #[test]
    fn a_click_on_the_blank_strip_clears_the_selection() {
        // browser/view-modes/columns: Clicking the blank strip right of the last column clears
        // the selection.
        let mut rt = scene("c2-three-columns-light");
        tap(&mut rt, &keys::blank("c2"));
        for depth in 0..3 {
            assert!(selected(&rt, depth).is_empty(), "column {depth}");
        }
        assert_eq!(path(&rt), "Home/Documents/Reports");
    }

    #[test]
    fn a_click_on_a_title_focuses_its_column_keeping_its_selection_and_deeper_columns() {
        // browser/view-modes/columns: Clicking a column's title focuses that column, keeps its
        // selection, and leaves deeper columns open. And: Clicking a column heading focuses
        // that column without changing its selection or closing its child column.
        let mut rt = scene("c2-three-columns-light");
        tap(&mut rt, &keys::title("c2-col-1"));
        assert_eq!(rt.state.c2.columns.active(), 0);
        assert_eq!(selected(&rt, 0), [id("Home", "Documents")]);
        assert_eq!(path(&rt), "Home/Documents/Reports");
        // The header actions move with the target: Home has no close button.
        assert!(info(&rt, &keys::close("c2-col-1")).is_none());
        assert!(info(&rt, "c2-refresh-0").is_some());
        assert!(info(&rt, &keys::close("c2-col-2")).is_none());
    }

    #[test]
    fn a_secondary_press_on_a_header_opens_its_folder_menu_and_not_on_its_buttons() {
        // browser/view-modes/columns: Right-clicking a column's title or empty header space
        // opens that column's folder menu at the click; right-clicking a header button opens
        // no folder menu.
        let mut rt = scene("c2-three-columns-light");
        let (x, y) = at(&rt, &keys::title("c2-col-3"));
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        settle(&mut rt, 2);
        assert_eq!(rt.state.c2.intent, "Folder menu for Reports");
        assert_eq!(rt.state.c2.columns.active(), 2);
        rt.state.c2.intent.clear();
        let (x, y) = at(&rt, "c2-refresh-2");
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        settle(&mut rt, 2);
        assert_eq!(rt.state.c2.intent, "", "the button keeps its own");
    }

    #[test]
    fn a_loading_column_shows_a_spinner_until_it_is_listed() {
        // browser/view-modes/columns: The header spinner shows only while that column is
        // loading and is hidden when idle.
        let mut rt = scene("c2-home-light");
        tap_name(&mut rt, &row("Home", "Slow"));
        assert!(info(&rt, &keys::spinner("c2-col-2")).is_some());
        settle(&mut rt, 130);
        assert!(info(&rt, &keys::spinner("c2-col-2")).is_none());
        assert!(info(&rt, &row("Slow", "late-0001.txt")).is_some());
    }

    #[test]
    fn ctrl_a_page_and_home_act_in_the_active_column_only() {
        // browser/selection: Ctrl+A selects every entry in the focused column, not the deepest
        // open column. browser/scrolling: Page Up and Page Down move the focus and selection
        // by one page in the focused listing; in Columns only the focused column moves.
        // browser/view-modes/columns: In a column with no keyboard cursor yet, Home or Ctrl+Up
        // lands on the first entry and End or Ctrl+Down on the last.
        let mut rt = scene("c2-three-columns-light");
        tap(&mut rt, &keys::title("c2-col-1"));
        press(&mut rt, Key::Text("a".into()), ctrl());
        assert_eq!(selected(&rt, 0).len(), 13, "Home: all of it");
        assert_eq!(
            selected(&rt, 1),
            [id("Documents", "Reports")],
            "Documents untouched"
        );
        assert!(selected(&rt, 2).is_empty(), "Reports untouched");
        // Reports has no cursor: End lands on its last entry.
        tap(&mut rt, &keys::title("c2-col-3"));
        assert_eq!(cursor(&rt, 2), None);
        key(&mut rt, Key::End);
        assert_eq!(cursor(&rt, 2), Some(id("Reports", "report-q0008.pdf")));
        key(&mut rt, Key::Home);
        assert_eq!(cursor(&rt, 2), Some(id("Reports", "report-q0001.pdf")));
        // Page Down in Documents moves Documents' cursor only.
        tap(&mut rt, &keys::title("c2-col-2"));
        let before = cursor(&rt, 0);
        key(&mut rt, Key::PageDown);
        assert_ne!(cursor(&rt, 1), Some(id("Documents", "Reports")));
        assert_eq!(cursor(&rt, 0), before);
        assert_eq!(cursor(&rt, 2), Some(id("Reports", "report-q0001.pdf")));
    }

    fn strip_offset(rt: &Runtime<Gallery>) -> f32 {
        rt.state.c2.columns.strip_offset()
    }

    fn listing_offset(rt: &Runtime<Gallery>, depth: usize) -> f32 {
        rt.state
            .c2
            .columns
            .column(depth)
            .expect("column")
            .collection
            .scroll_offset()
    }

    #[test]
    fn a_double_click_on_a_closed_folder_ends_as_a_single_click() {
        // browser/view-modes/columns: Double-clicking a closed folder ends as a single click
        // does: its column opens, focused and scrolled into view, with nothing selected in it.
        // And: Pressing a folder does not shift the strip and shift it back before its column
        // takes focus.
        let mut rt = scene("c2-overflow-light");
        let before = strip_offset(&rt);
        assert!(before > 0.0, "scrolled to the active column");
        // level-5's column is last; its folder level-6 is closed.
        let [rx, ry, _, rh] = info(&rt, &row("level-5", "level-6")).expect("row").visible;
        let (x, y) = (rx + 18.0, ry + rh / 2.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        assert_eq!(strip_offset(&rt), before, "the press moves no strip");
        rt.pointer_up(x, y);
        rt.frame(DT);
        rt.pointer_down(x, y);
        rt.frame(DT);
        rt.pointer_up(x, y);
        settle(&mut rt, 40);
        assert!(path(&rt).ends_with("level-5/level-6"), "{}", path(&rt));
        let depth = rt.state.c2.listings.0.len() - 1;
        assert_eq!(rt.state.c2.columns.active(), depth, "focused");
        assert_eq!(cursor(&rt, depth), None, "nothing selected in it");
        assert!(strip_offset(&rt) > before, "scrolled into view");
    }

    #[test]
    fn with_double_click_folders_a_double_click_opens_it_from_a_clipped_parent() {
        // browser/view-modes/columns: The same holds when the first press scrolls the strip,
        // with double-click folders, and from a partly clipped parent column.
        let mut rt = scene("c2-three-columns-light");
        rt.state.c2.folder_clicks = strata_components::collection::pointer::ClickCount::Two;
        // A strip 870 wide, scrolled 30 points: Home is clipped by 30.
        rt.state.c2.width = Some(870.0);
        settle(&mut rt, 2);
        rt.update(|_, tx| tx.scroll_to_x(keys::strip("c2"), 30.0));
        settle(&mut rt, 2);
        assert!(
            strip_offset(&rt) > 20.0,
            "Home is clipped: {}",
            strip_offset(&rt)
        );
        let [rx, ry, _, rh] = info(&rt, &row("Home", "Pictures")).expect("row").visible;
        let (x, y) = (rx + 30.0, ry + rh / 2.0);
        click_at(&mut rt, x, y);
        assert_eq!(
            path(&rt),
            "Home/Documents/Reports",
            "one click opens no folder"
        );
        click_at(&mut rt, x, y);
        settle(&mut rt, 40);
        assert_eq!(path(&rt), "Home/Pictures");
        assert_eq!(rt.state.c2.columns.active(), 1);
        assert_eq!(cursor(&rt, 1), None);
    }

    #[test]
    fn a_plain_click_on_a_folder_in_a_multi_selection_collapses_it_and_opens_it() {
        // browser/view-modes/columns: With folders opening on one click, a plain click on a
        // folder inside a multi-selection collapses the selection to it and opens its column.
        let mut rt = scene("c2-home-light");
        tap_name(&mut rt, &row("Home", "notes.txt"));
        rt.physical_input(PhysicalInput::Modifiers(ctrl()));
        tap_name(&mut rt, &row("Home", "Music"));
        tap_name(&mut rt, &row("Home", "Projects"));
        rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
        assert_eq!(selected(&rt, 0).len(), 3);
        assert_eq!(path(&rt), "Home");
        let [x, y, _, h] = info(&rt, &row("Home", "Music")).expect("row").visible;
        click_at(&mut rt, x + 18.0, y + h / 2.0);
        settle(&mut rt, 2);
        assert_eq!(selected(&rt, 0), [id("Home", "Music")]);
        assert_eq!(path(&rt), "Home/Music");
    }

    #[test]
    fn a_file_drag_or_a_modified_click_in_a_parent_leaves_the_deeper_columns_open() {
        // browser/view-modes/columns: A press on a file that turns into a drag leaves the
        // deeper columns open, so the file can be dragged into the open column. And: Ctrl+click
        // and Shift+click on a file leave the deeper columns open. (The plain click that closes
        // them is Strata's policy, which the page stands in for.)
        let mut rt = scene("c2-three-columns-light");
        let [x, y, _, h] = info(&rt, &row("Home", "notes.txt")).expect("row").visible;
        let (x, y) = (x + 40.0, y + h / 2.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        for k in 1..=6 {
            rt.pointer_move(x + 8.0 * k as f32, y);
            rt.frame(DT);
        }
        rt.pointer_up(x + 48.0, y);
        settle(&mut rt, 2);
        assert_eq!(rt.state.c2.intent, "Dragging 1 from column 0");
        assert_eq!(path(&rt), "Home/Documents/Reports");
        for mods in [
            ctrl(),
            Mods {
                shift: true,
                ..Mods::default()
            },
        ] {
            rt.physical_input(PhysicalInput::Modifiers(mods));
            tap_name(&mut rt, &row("Home", "todo.md"));
            rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
            assert_eq!(path(&rt), "Home/Documents/Reports", "{mods:?}");
        }
        // The page's stand-in for Strata: a plain click closes them.
        tap_name(&mut rt, &row("Home", "photo.jpg"));
        assert_eq!(path(&rt), "Home");
    }

    #[test]
    fn a_press_that_moves_less_than_the_threshold_starts_no_drag() {
        // browser/view-modes/columns: Pressing a row and moving less than GTK's drag threshold
        // starts no file drag.
        let mut rt = scene("c2-three-columns-light");
        let (x, y) = at(&rt, &row("Documents", "cv.pdf"));
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        rt.pointer_move(x + 5.0, y + 3.0);
        rt.frame(DT);
        rt.pointer_up(x + 5.0, y + 3.0);
        settle(&mut rt, 2);
        assert!(
            !rt.state.c2.intent.starts_with("Dragging"),
            "{}",
            rt.state.c2.intent
        );
    }

    #[test]
    fn with_animations_off_a_closed_column_goes_at_once() {
        // browser/view-modes/columns: With reduced motion or animations off, a closing column
        // is removed at once.
        let mut rt = scene("c2-three-columns-light");
        rt.state.look.animations = false;
        settle(&mut rt, 2);
        tap(&mut rt, &keys::close("c2-col-2"));
        assert!(
            !rt.elements()
                .iter()
                .any(|e| e.key.as_deref() == Some("c2-slot-2")),
            "gone with no exit"
        );
    }

    #[test]
    fn a_closing_column_takes_no_presses_while_it_leaves() {
        // browser/view-modes/columns: A closing column takes no clicks, hovers, or drops while
        // it leaves.
        let mut rt = scene("c2-three-columns-light");
        let (x, y) = at(&rt, &row("Reports", "report-q0003.pdf"));
        rt.update(|g, tx| {
            let look = g.look.clone();
            g.c2.listings.0.truncate(2);
            g.c2.columns.truncate(2, &look, tx);
        });
        rt.frame(DT);
        rt.state.c2.intent.clear();
        click_at(&mut rt, x, y);
        assert!(
            !rt.state.c2.intent.contains("report-q0003"),
            "{}",
            rt.state.c2.intent
        );
    }

    #[test]
    fn a_rebuilt_strip_replaces_its_columns_with_no_exit() {
        // browser/view-modes/columns: Opening another location, rebuilding the strip, or
        // switching away from Columns replaces the columns with no exit animation.
        let mut rt = scene("c2-three-columns-light");
        rt.update(|g, tx| {
            let look = g.look.clone();
            let d = &mut g.c2;
            d.listings.0.truncate(1);
            d.columns.replace(&d.listings, 0, &|_| None, &look, tx);
        });
        rt.frame(DT);
        assert!(
            !rt.elements().iter().any(|e| e.exiting),
            "nothing leaves on a transition"
        );
        assert_eq!(path(&rt), "Home");
    }

    #[test]
    fn a_keyboard_move_reveals_a_clipped_column_and_leaves_a_shown_one() {
        // browser/view-modes/columns: A keyboard move reveals a column clipped by up to 4 px
        // fully and leaves an already visible column in place.
        let mut rt = scene("c2-overflow-light");
        let end = strip_offset(&rt);
        // Scroll the strip 3 points back: the active (last) column is clipped by 3 points.
        rt.update(|_, tx| tx.scroll_to_x(keys::strip("c2"), end - 3.0));
        settle(&mut rt, 2);
        assert!((strip_offset(&rt) - (end - 3.0)).abs() < 0.5);
        tap(&mut rt, &keys::title("c2-col-7"));
        assert!(
            (strip_offset(&rt) - (end - 3.0)).abs() < 0.5,
            "a press reveals nothing"
        );
        key(&mut rt, Key::Down);
        settle(&mut rt, 30);
        assert!((strip_offset(&rt) - end).abs() < 0.5, "revealed fully");
        key(&mut rt, Key::Down);
        settle(&mut rt, 30);
        assert!(
            (strip_offset(&rt) - end).abs() < 0.5,
            "a shown column stays"
        );
    }

    #[test]
    fn a_column_wider_than_the_strip_stays_while_it_fills_it() {
        // browser/view-modes/columns: A column wider than the viewport that already fills it
        // stays in place; a partly off-screen one scrolls to its left edge.
        let mut rt = scene("c2-wide-column-light");
        tap(&mut rt, &keys::title("c2-col-2"));
        settle(&mut rt, 30);
        // Music's column (1,100 wide) is active; the strip shows its left edge.
        let left = strip_offset(&rt);
        assert!(left > 0.0);
        rt.update(|_, tx| tx.scroll_to_x(keys::strip("c2"), left + 100.0));
        settle(&mut rt, 2);
        key(&mut rt, Key::Down);
        settle(&mut rt, 30);
        assert!(
            (strip_offset(&rt) - (left + 100.0)).abs() < 0.5,
            "it fills the strip"
        );
        // Scrolled so it shows only partly: back to its left edge.
        rt.update(|_, tx| tx.scroll_to_x(keys::strip("c2"), 0.0));
        settle(&mut rt, 2);
        key(&mut rt, Key::Down);
        settle(&mut rt, 30);
        assert!(
            (strip_offset(&rt) - left).abs() < 0.5,
            "{} vs {left}",
            strip_offset(&rt)
        );
    }

    #[test]
    fn back_to_an_ancestor_selects_the_folder_it_came_from_with_its_column_closed() {
        // browser/view-modes/columns: Back, Forward, or Alt+Up to an ancestor selects the folder
        // you came from, at any depth, with the cursor on it. And: The came-from folder's own
        // column stays closed.
        let mut rt = scene("c2-three-columns-light");
        tap(&mut rt, &keys::title("c2-col-3"));
        press(
            &mut rt,
            Key::Up,
            Mods {
                alt: true,
                ..Mods::default()
            },
        );
        assert_eq!(path(&rt), "Home/Documents");
        assert_eq!(rt.state.c2.columns.active(), 1);
        assert_eq!(selected(&rt, 1), [id("Documents", "Reports")]);
        assert_eq!(cursor(&rt, 1), Some(id("Documents", "Reports")));
    }

    #[test]
    fn left_into_a_column_with_no_cursor_puts_it_on_the_folder_it_has_open() {
        // browser/view-modes/columns: When Back leaves ancestor columns with no cursor, Left
        // into an ancestor column puts the cursor on the folder that column has open.
        let mut rt = scene("c2-three-columns-light");
        rt.update(|g, _| {
            let d = &mut g.c2;
            let src = &d.listings.0[1].source;
            let c = &mut d.columns.column_mut(1).unwrap().collection;
            c.selection
                .restore(&strata_components::collection::selection::Position::default());
            c.note_selection(src);
        });
        tap(&mut rt, &keys::title("c2-col-3"));
        key(&mut rt, Key::Down);
        key(&mut rt, Key::Left);
        assert_eq!(rt.state.c2.columns.active(), 1);
        assert_eq!(cursor(&rt, 1), Some(id("Documents", "Reports")));
    }

    #[test]
    fn left_into_a_loading_column_places_the_cursor_once_it_is_listed() {
        // browser/view-modes/columns: If that column is still loading, the cursor lands when it
        // finishes. If the folder is gone or hidden by then, nothing is selected.
        let mut rt = scene("c2-home-light");
        tap_name(&mut rt, &row("Home", "Slow"));
        // Slow is still listing; pretend a folder of it is open in a third column.
        rt.update(|g, tx| {
            let look = g.look.clone();
            let d = &mut g.c2;
            let mut listed = strata_gallery::scenes::c2::Listed {
                name: "late-0003.txt".into(),
                source: Default::default(),
                version: 1,
                due: None,
                from: Some(id_in("Slow", "late-0003.txt")),
            };
            listed.source = strata_components::contract::ListSource::new(Vec::new());
            d.listings.0.push(listed);
            d.columns.push(
                strata_components::columns::Opening::new(
                    &d.listings.0[2].source,
                    strata_components::collection::selection::Listing::Complete,
                ),
                strata_components::columns::Focus::Column,
                &look,
                tx,
            );
        });
        settle(&mut rt, 2);
        assert_eq!(rt.state.c2.columns.active(), 2);
        key(&mut rt, Key::Left);
        assert_eq!(rt.state.c2.columns.active(), 1);
        assert_eq!(cursor(&rt, 1), None, "Slow is still listing");
        settle(&mut rt, 130);
        assert_eq!(cursor(&rt, 1), Some(id("Slow", "late-0003.txt")));
    }

    #[test]
    fn a_click_on_empty_space_keeps_the_columns_scroll() {
        // browser/view-modes/columns: Clicking empty space in a column keeps that column's
        // vertical scroll position instead of returning to the selected row.
        let mut rt = scene("c2-hundred-k-light");
        let before = listing_offset(&rt, 1);
        assert!(before > 1_000.0);
        tap(&mut rt, &keys::blank("c2"));
        let [x, y, w, h] = info(&rt, "c2-slot-2").expect("Big").visible;
        // The blank strip beside Big's rows: its right margin.
        click_at(&mut rt, x + w - 3.0, y + h / 2.0);
        settle(&mut rt, 10);
        assert_eq!(listing_offset(&rt, 1), before);
    }

    #[test]
    fn a_sideways_wheel_over_a_listing_scrolls_the_strip_and_a_vertical_one_the_listing() {
        // browser/view-modes/columns: Horizontal touchpad gestures over a listing scroll the
        // column strip. And the vertical half of: Shift+wheel over a listing scrolls the column
        // strip horizontally; vertical wheel ticks scroll only the listing. (Rune's headless
        // wheel doesn't turn Shift+wheel sideways: Rune draft U-C24-7.)
        let mut rt = scene("c2-overflow-light");
        let strip = strip_offset(&rt);
        let (x, y) = at(&rt, &row("level-4", "level-5"));
        rt.wheel_xy(x, y, 120.0, 0.0);
        settle(&mut rt, 20);
        assert!(strip_offset(&rt) < strip, "the strip moved back");
        let mut rt = scene("c2-hundred-k-light");
        let (strip, listing) = (strip_offset(&rt), listing_offset(&rt, 1));
        let [x, y, w, h] = info(&rt, "c2-slot-2").expect("Big").visible;
        rt.wheel(x + w / 2.0, y + h / 2.0, -200.0);
        settle(&mut rt, 20);
        assert_eq!(strip_offset(&rt), strip);
        assert!(listing_offset(&rt, 1) != listing);
    }

    /// Drags column `serial`'s edge by `dx`, with `cancel` pressing Escape before the release.
    fn drag_edge(rt: &mut Runtime<Gallery>, serial: u64, dx: f32, from: f32) {
        let [x, y, w, h] = info(rt, &keys::edge(&format!("c2-col-{serial}")))
            .expect("edge")
            .visible;
        let _ = w;
        let (x, y) = (x + 6.0 + from, y + h / 2.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        for k in 1..=4 {
            rt.pointer_move(x + dx * k as f32 / 4.0, y);
            rt.frame(DT);
        }
    }

    fn width(rt: &Runtime<Gallery>, depth: usize) -> f32 {
        let c = &rt.state.c2.columns.column(depth).expect("column");
        let [_, _, w, _] = info(rt, &format!("c2-slot-{}", depth + 1))
            .expect("shell")
            .rect;
        let _ = c;
        w
    }

    #[test]
    fn dragging_an_edge_resizes_only_its_column_never_below_one_standard_column() {
        // browser/view-modes/columns: Dragging a column's right edge resizes only that column
        // while the pointer moves, never below 300 px. And: A resize ... is saved as
        // `browser_column_width`; new columns open at that width and open columns keep theirs
        // (the view hands the host the width; saving it is the host's).
        let mut rt = scene("c2-three-columns-light");
        drag_edge(&mut rt, 1, 100.0, 0.0);
        assert_eq!(width(&rt, 0), 400.0, "it follows the pointer");
        assert_eq!(width(&rt, 1), 300.0, "the others keep theirs");
        let (x, y) = rt.pointer().expect("pointer");
        rt.pointer_up(x, y);
        settle(&mut rt, 2);
        assert_eq!(rt.state.c2.intent, "Width 400");
        assert_eq!(width(&rt, 2), 300.0, "open columns keep theirs");
        drag_edge(&mut rt, 2, -150.0, 0.0);
        assert_eq!(width(&rt, 1), 300.0, "never below one standard column");
        let (x, y) = rt.pointer().expect("pointer");
        rt.pointer_up(x, y);
        settle(&mut rt, 2);
        // A new column opens at the width handed over.
        tap_name(&mut rt, &row("Reports", "report-q0001.pdf"));
        tap(&mut rt, &keys::title("c2-col-1"));
        tap_name(&mut rt, &row("Home", "Music"));
        settle(&mut rt, 40);
        assert_eq!(width(&rt, 1), 400.0, "{}", path(&rt));
    }

    #[test]
    fn a_press_either_side_of_the_line_resizes_the_left_column() {
        // browser/view-modes/columns: A press within 6 px either side of the line between two
        // columns resizes the left column, even when partly scrolled out of view.
        for from in [-5.0, 5.0] {
            let mut rt = scene("c2-three-columns-light");
            drag_edge(&mut rt, 1, 50.0, from);
            assert_eq!(width(&rt, 0), 350.0, "from {from}");
            assert_eq!(width(&rt, 1), 300.0);
        }
    }

    #[test]
    fn a_cancelled_edge_drag_puts_the_width_back() {
        // browser/view-modes/columns: A cancelled edge drag restores the column's previous
        // width.
        let mut rt = scene("c2-three-columns-light");
        drag_edge(&mut rt, 1, 120.0, 0.0);
        assert_eq!(width(&rt, 0), 420.0);
        key(&mut rt, Key::Escape);
        let (x, y) = rt.pointer().expect("pointer");
        rt.pointer_up(x, y);
        settle(&mut rt, 2);
        assert_eq!(width(&rt, 0), 300.0);
        assert_ne!(rt.state.c2.intent, "Width 420", "nothing handed over");
    }

    #[test]
    fn an_edge_drag_stops_at_the_area_and_the_cap_never_goes_below_a_standard_column() {
        // browser/view-modes/columns: Autofit and edge drags stop at the visible browser area
        // (the drag half; the area itself is the host's). And: The width cap never goes below
        // the 300 px standard column, scaled by text size.
        let mut rt = scene("c2-three-columns-light");
        rt.state.c2.columns.set_width_cap(Some(500.0));
        drag_edge(&mut rt, 1, 400.0, 0.0);
        assert_eq!(width(&rt, 0), 500.0, "stops at the area");
        let (x, y) = rt.pointer().expect("pointer");
        rt.pointer_up(x, y);
        let mut rt = scene("c2-three-columns-light");
        rt.state.c2.columns.set_width_cap(Some(200.0));
        drag_edge(&mut rt, 1, 100.0, 0.0);
        assert_eq!(width(&rt, 0), 300.0, "a cap below a standard column is one");
    }

    #[test]
    fn a_saved_width_wider_than_the_area_opens_columns_narrowed_to_it() {
        // browser/view-modes/columns: A saved width wider than the area, such as
        // `browser_column_width = 1729` in a 1200 px window, opens columns narrowed to the area.
        // The setting is unchanged.
        let mut rt = scene("c2-home-light");
        rt.state.c2.columns.set_default_width(1_729.0);
        rt.state.c2.columns.set_width_cap(Some(700.0));
        tap_name(&mut rt, &row("Home", "Music"));
        settle(&mut rt, 40);
        assert_eq!(width(&rt, 1), 700.0);
        rt.state.c2.columns.set_width_cap(None);
        tap(&mut rt, &keys::title("c2-col-1"));
        tap_name(&mut rt, &row("Home", "Projects"));
        settle(&mut rt, 40);
        assert_eq!(width(&rt, 1), 1_729.0, "the saved width is unchanged");
    }

    #[test]
    fn a_rest_on_a_column_edge_shows_the_width_caption() {
        // browser/view-modes/columns: Resting the pointer on a column edge for 400 ms shows a
        // "Column width" caption, which follows the edge during a drag.
        let mut rt = scene("c2-three-columns-light");
        let (x, y) = at(&rt, &keys::edge("c2-col-1"));
        rt.pointer_move(x, y);
        settle(&mut rt, 12);
        assert!(
            node(&rt, &keys::caption("c2-col-1")).is_none(),
            "not before 400 ms"
        );
        settle(&mut rt, 20);
        let caption = node(&rt, &keys::caption("c2-col-1")).expect("caption");
        assert_eq!(caption["name"], "Column width");
        let [before, ..] = info(&rt, &keys::caption("c2-col-1")).expect("caption").rect;
        rt.pointer_down(x, y);
        rt.frame(DT);
        rt.pointer_move(x + 60.0, y);
        settle(&mut rt, 2);
        let [after, ..] = info(&rt, &keys::caption("c2-col-1"))
            .expect("during the drag")
            .rect;
        assert_eq!(after - before, 60.0, "it rides the edge");
        rt.pointer_up(x + 60.0, y);
    }

    #[test]
    fn the_edge_of_the_column_beside_the_preview_lights_the_divider_line() {
        // browser/view-modes/columns: Hovering the edge of the column that meets the preview
        // also lights the preview divider line.
        let mut rt = scene("c2-preview-slot-light");
        rt.update(|g, _| {
            if let Some(d) = g.c2.drawer.as_mut() {
                d.open_at_once();
            }
        });
        settle(&mut rt, 10);
        // The last column active, so its edge meets the preview.
        rt.update(|g, tx| {
            let look = g.look.clone();
            g.c2.columns.set_active(2, &look, tx);
        });
        settle(&mut rt, 40);
        let lit = |rt: &Runtime<Gallery>| info(rt, "preview-divider-lit").map(|e| e.opacity);
        assert_eq!(lit(&rt), Some(0.0));
        let (x, y) = at(&rt, &keys::edge("c2-col-3"));
        rt.pointer_move(x - 3.0, y);
        settle(&mut rt, 60);
        assert_eq!(lit(&rt), Some(1.0));
        rt.pointer_move(x - 200.0, y);
        settle(&mut rt, 40);
        assert_eq!(lit(&rt), Some(0.0));
    }

    #[test]
    fn a_secondary_press_on_a_header_targets_its_column_for_the_keys() {
        // browser/view-modes/columns: Escape on a header-opened folder menu returns arrow-key
        // navigation to that column (the menu is C21's; the column it returns to is the view's).
        // And: While an item or background context menu is open in a column, that column keeps
        // the header actions even when the pointer moves over another column.
        let mut rt = scene("c2-three-columns-light");
        let (x, y) = at(&rt, &keys::title("c2-col-1"));
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        settle(&mut rt, 2);
        rt.state.c2.columns.hold_target(Some(0));
        // The pointer presses another column's title: the held one keeps the actions.
        let (x2, y2) = at(&rt, &keys::title("c2-col-3"));
        rt.pointer_move(x2, y2);
        settle(&mut rt, 2);
        assert!(info(&rt, "c2-refresh-0").is_some());
        rt.state.c2.columns.hold_target(None);
        // The menu closed (Escape, C21's): the arrows move Home's cursor.
        let before = cursor(&rt, 0);
        focus_strip(&mut rt);
        key(&mut rt, Key::Down);
        assert_ne!(cursor(&rt, 0), before);
    }

    // Column autoscroll during a drag (operations/drag-and-drop/column-autoscroll).

    fn drag_files(rt: &mut Runtime<Gallery>, x: f32, y: f32) {
        rt.drag_files(vec!["/tmp/a.txt".into()], Some((x, y)));
        rt.frame(DT);
    }

    #[test]
    fn a_drag_near_the_strips_edge_scrolls_the_columns_toward_it_and_stops_when_it_leaves() {
        // operations/drag-and-drop/column-autoscroll: Holding a file drag within 44 px of the
        // column strip's left or right edge scrolls the columns toward that edge. And:
        // Autoscroll stops when the drag leaves the strip, drops, or ends, or the content
        // cannot scroll further.
        let mut rt = scene("c2-overflow-light");
        let start = strip_offset(&rt);
        let strip = rt.state.c2.columns.strip_rect();
        let y = strip.y + strip.h / 2.0;
        drag_files(&mut rt, strip.x + 10.0, y);
        settle(&mut rt, 20);
        let moved = strip_offset(&rt);
        assert!(moved < start - 20.0, "{moved} from {start}");
        // Away from the edge: it stops where it is.
        rt.drag_files_to(Some((strip.x + strip.w / 2.0, y)));
        settle(&mut rt, 3);
        let stopped = strip_offset(&rt);
        settle(&mut rt, 20);
        assert_eq!(strip_offset(&rt), stopped);
        // Back at the edge, then the drag goes: it stops.
        rt.drag_files_to(Some((strip.x + 10.0, y)));
        settle(&mut rt, 10);
        rt.drag_files_leave();
        settle(&mut rt, 2);
        let left = strip_offset(&rt);
        settle(&mut rt, 20);
        assert_eq!(strip_offset(&rt), left);
        // At the start it can't go further: it rests there.
        drag_files(&mut rt, strip.x + 10.0, y);
        settle(&mut rt, 200);
        assert!(strip_offset(&rt) < 1.0, "{}", strip_offset(&rt));
        rt.drag_files_leave();
        rt.frame(DT);
    }

    #[test]
    fn a_drag_near_a_listings_bottom_scrolls_that_listing() {
        // operations/drag-and-drop/column-autoscroll: Holding a file drag within 44 px of the
        // top or bottom edge of the listing under the pointer scrolls that listing.
        let mut rt = scene("c2-three-columns-light");
        let v = rt.state.c2.columns.column(1).unwrap().collection.viewport();
        drag_files(&mut rt, v.x + v.w / 2.0, v.y + v.h - 5.0);
        settle(&mut rt, 30);
        assert!(listing_offset(&rt, 1) > 20.0);
        assert_eq!(listing_offset(&rt, 0), 0.0);
        assert_eq!(listing_offset(&rt, 2), 0.0);
        rt.drag_files_leave();
        rt.frame(DT);
    }

    #[test]
    fn the_column_under_a_held_drag_is_washed_including_one_autoscroll_revealed() {
        // operations/drag-and-drop/column-autoscroll: The column under a held file drag shows
        // an accent wash and ring, including a column just revealed by autoscroll. And: The
        // highlight stays while autoscroll is at its limit and clears when the drag leaves the
        // column.
        let mut rt = scene("c2-overflow-light");
        let strip = rt.state.c2.columns.strip_rect();
        let y = strip.y + strip.h / 2.0;
        drag_files(&mut rt, strip.x + 10.0, y);
        settle(&mut rt, 2);
        let first = rt.state.c2.columns.drop_column();
        assert!(first.is_some());
        settle(&mut rt, 200);
        assert_eq!(
            rt.state.c2.columns.drop_column(),
            Some(0),
            "Home, revealed at the start"
        );
        assert!(
            info(&rt, &keys::wash("c2-col-1")).is_some(),
            "washed at the limit"
        );
        // Onto the blank strip... there is none here: leave the window instead.
        rt.drag_files_leave();
        settle(&mut rt, 2);
        assert_eq!(rt.state.c2.columns.drop_column(), None);
        assert!(info(&rt, &keys::wash("c2-col-1")).is_none());
    }

    #[test]
    fn a_drop_on_a_partly_shown_column_goes_there_and_the_strip_stays() {
        // operations/drag-and-drop/column-autoscroll: Dropping onto a partially visible column
        // moves the file there, and the strip does not scroll back to the source column.
        let mut rt = scene("c2-overflow-light");
        let start = strip_offset(&rt);
        let strip = rt.state.c2.columns.strip_rect();
        // The column clipped on the left (level-2's), 50 points in from the strip's edge.
        let at = (strip.x + 50.0, strip.y + strip.h - 60.0);
        rt.drop_files(vec!["/tmp/a.txt".into()], Some(at));
        settle(&mut rt, 20);
        assert_eq!(rt.state.c2.intent, "Dropped 1 into level-2");
        assert_eq!(strip_offset(&rt), start);
    }

    fn focused(rt: &Runtime<Gallery>) -> Vec<String> {
        rt.elements()
            .into_iter()
            .filter(|e| e.focused)
            .filter_map(|e| e.key)
            .collect()
    }

    #[test]
    fn after_the_close_button_the_keys_still_reach_the_strip() {
        // browser/view-modes/columns: Each column after the first has a "Close this pane" X
        // button that closes it (and the keyboard goes on working: the strip keeps focus). And
        // bullet 58's half the view holds: the view moves no focus, so the strip keeps it.
        let mut rt = scene("c2-three-columns-light");
        tap(&mut rt, &keys::title("c2-col-2"));
        tap(&mut rt, &keys::close("c2-col-2"));
        assert_eq!(path(&rt), "Home");
        assert_eq!(focused(&rt), ["c2"], "the strip keeps focus");
        let before = cursor(&rt, 0);
        key(&mut rt, Key::Down);
        assert_ne!(cursor(&rt, 0), before, "Down moves Home's cursor");
        // A host's header action (the page's Refresh) leaves the keys with the strip too.
        tap(&mut rt, "c2-refresh-0");
        assert_eq!(rt.state.c2.intent, "Refreshed column 0");
        assert_eq!(focused(&rt), ["c2"]);
    }

    #[test]
    fn a_late_listing_in_a_column_left_behind_places_no_cursor_and_moves_no_strip() {
        // browser/view-modes/columns: If that column is still loading, the cursor lands when it
        // finishes (while the column holds the keyboard; one the person has left places none,
        // as Strata's restore gives up once focus leaves the list).
        let mut rt = scene("c2-home-light");
        rt.state.c2.width = Some(500.0);
        settle(&mut rt, 5);
        tap(&mut rt, &keys::title("c2-col-1"));
        rt.update(|g, _| {
            let d = &mut g.c2;
            let src = &d.listings.0[0].source;
            let c = &mut d.columns.column_mut(0).unwrap().collection;
            let i = src.index_of(id_in("Home", "Slow")).unwrap();
            let _ = c.selection.select_only(src, i);
            c.note_selection(src);
        });
        key(&mut rt, Key::Right);
        settle(&mut rt, 20);
        assert_eq!(path(&rt), "Home/Slow");
        assert_eq!(rt.state.c2.columns.active(), 1);
        key(&mut rt, Key::Left);
        settle(&mut rt, 20);
        assert_eq!(rt.state.c2.columns.active(), 0);
        let offset = strip_offset(&rt);
        settle(&mut rt, 130);
        assert_eq!(
            cursor(&rt, 1),
            None,
            "no cursor lands in the column left behind"
        );
        assert_eq!(
            strip_offset(&rt),
            offset,
            "the strip stays on the active column"
        );
    }

    #[test]
    fn a_keyboard_reveal_glides_the_strip() {
        // browser/view-modes/columns: A keyboard move reveals a column clipped by up to 4 px
        // fully (on the view's reveal glide, not by the row's own reveal jumping the strip).
        let mut rt = scene("c2-overflow-light");
        tap(&mut rt, &keys::title("c2-col-7"));
        let end = strip_offset(&rt);
        rt.update(|_, tx| tx.scroll_to_instant(keys::strip("c2"), 0.0, 0.0));
        settle(&mut rt, 5);
        assert_eq!(strip_offset(&rt), 0.0);
        rt.key(KeyEvent {
            key: Key::Down,
            mods: Mods::default(),
        });
        rt.frame(DT);
        rt.frame(DT);
        let early = strip_offset(&rt);
        assert!(
            early > 0.0 && early < end - 50.0,
            "gliding: {early} of {end}"
        );
        settle(&mut rt, 40);
        assert!(
            (strip_offset(&rt) - end).abs() < 0.5,
            "{} vs {end}",
            strip_offset(&rt)
        );
    }

    /// Column `depth`'s laid-out width and how much of it shows.
    fn shown(rt: &Runtime<Gallery>, depth: usize) -> (f32, f32) {
        let slot = rt
            .state
            .c2
            .columns
            .column(depth)
            .map(|_| depth + 1)
            .unwrap();
        let i = info(rt, &format!("c2-slot-{slot}")).expect("slot");
        (i.rect[2], i.visible[2])
    }

    #[test]
    fn a_reveal_in_the_same_handler_as_a_narrower_strip_still_shows_the_column() {
        // browser/view-modes/columns: A keyboard move reveals a column ... fully (the reveal is
        // worked out again when the strip's width changes right after it).
        let mut rt = scene("c2-overflow-light");
        rt.update(|g, tx| {
            let look = g.look.clone();
            g.c2.columns.set_active(0, &look, tx);
        });
        settle(&mut rt, 60);
        assert!(strip_offset(&rt) < 0.5);
        rt.update(|g, tx| {
            let look = g.look.clone();
            g.c2.width = Some(700.0);
            g.c2.columns.set_active(6, &look, tx);
        });
        settle(&mut rt, 60);
        let (w, visible) = shown(&rt, 6);
        assert!((w - visible).abs() < 0.5, "level-5 shows {visible} of {w}");
    }

    #[test]
    fn a_drawer_opening_in_the_same_handler_as_a_close_keeps_the_active_column_whole() {
        // browser/view-modes/columns: A keyboard move reveals a column ... fully; here the host
        // closes the deepest column and opens an unreserved drawer in one handler.
        let mut rt = scene("c2-overflow-light");
        rt.update(|g, _| g.c2.drawer = Some(strata_components::preview_drawer::DrawerState::new()));
        settle(&mut rt, 20);
        rt.update(|g, tx| {
            let look = g.look.clone();
            g.c2.listings.0.truncate(6);
            g.c2.columns.truncate(6, &look, tx);
            g.c2.drawer.as_mut().unwrap().open(&look, tx);
        });
        settle(&mut rt, 90);
        assert_eq!(rt.state.c2.columns.active(), 5);
        let (w, visible) = shown(&rt, 5);
        assert!((w - visible).abs() < 0.5, "level-4 shows {visible} of {w}");
    }

    #[test]
    fn the_blank_strip_is_no_control_assistive_technology_hears() {
        // What assistive technology hears: the strip and its named lists, no unnamed button for
        // the blank space after the last column (a press there clears the selection).
        let rt = scene("c2-three-columns-light");
        assert!(
            info(&rt, &keys::blank("c2")).is_some(),
            "it is there to press"
        );
        assert!(
            node(&rt, &keys::blank("c2")).is_none(),
            "{:?}",
            node(&rt, &keys::blank("c2"))
        );
    }
}
