//! C8's page in process: what the accessibility tree doesn't show (tooltips, motion, focus
//! rings, the Trash target's key, and a drop target that holds while its gap opens). The
//! scripts in `tests/scripts/c8/` check the rest. Each test names the bullets it covers.

mod c8 {
    use std::path::PathBuf;

    use rune::{ElementInfo, Env, Key, KeyEvent, Mods, Runtime};
    use strata_components::sidebar::TRASH_TARGET;
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
        run(&mut rt, 1.0);
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

    fn element(rt: &Runtime<Gallery>, key: &str) -> Option<ElementInfo> {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }

    fn help(rt: &Runtime<Gallery>, key: &str) -> Option<String> {
        element(rt, key)
            .unwrap_or_else(|| panic!("no element {key}"))
            .help
    }

    fn press(rt: &mut Runtime<Gallery>, key: Key) {
        rt.key(KeyEvent {
            key,
            mods: Mods::default(),
        });
        rt.frame(FRAME);
    }

    fn tap(rt: &mut Runtime<Gallery>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(FRAME);
    }

    fn centre(rt: &Runtime<Gallery>, key: &str) -> (f32, f32) {
        let r = element(rt, key)
            .unwrap_or_else(|| panic!("no element {key}"))
            .rect;
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    // operations/trash/animation: "flies its icons from their rows into the sidebar Trash
    // button": C8 publishes the target's key in both widths, and none while hidden.
    #[test]
    fn the_trash_target_is_keyed_in_full_and_rail_and_gone_when_hidden() {
        assert!(element(&mount("c8-full-light"), TRASH_TARGET).is_some());
        assert!(element(&mount("c8-rail-light"), TRASH_TARGET).is_some());
        assert!(element(&mount("c8-hidden-light"), TRASH_TARGET).is_none());
    }

    // browser/sidebar/rail: "In the icon rail, rows show only icons with name tooltips, and
    // device buttons are hidden."
    #[test]
    fn rail_rows_show_their_names_as_tooltips_and_hide_device_buttons() {
        let rt = mount("c8-rail-light");
        assert_eq!(help(&rt, "places-place-home").as_deref(), Some("Home"));
        assert_eq!(
            help(&rt, "places-pin-photos").as_deref(),
            Some("Photos 2026")
        );
        assert_eq!(
            help(&rt, "places-device-sandisk").as_deref(),
            Some("SanDisk")
        );
        assert!(element(&rt, "places-device-sandisk-eject").is_none());
        let full = mount("c8-full-light");
        assert_eq!(
            help(&full, "places-place-home"),
            None,
            "full rows have no tooltip"
        );
    }

    // browser/sidebar/rail: "The toggle's tooltip names the next step"
    #[test]
    fn the_toggle_tooltip_names_the_next_step() {
        let mut rt = mount("c8-full-light");
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(help(&rt, "places-toggle").unwrap_or_default());
            press(&mut rt, Key::Command('b'));
        }
        assert_eq!(
            seen,
            [
                "Compact sidebar (Ctrl+B)",
                "Hide sidebar (Ctrl+B)",
                "Show sidebar (Ctrl+B)"
            ]
        );
    }

    // browser/sidebar: "In the icon rail, a heading's tooltip is its section name, such as
    // "PINNED"; in the full sidebar it has no tooltip."
    #[test]
    fn a_rail_heading_names_its_section_in_a_tooltip() {
        let rail = mount("c8-rail-light");
        assert_eq!(
            help(&rail, "places-heading-pinned").as_deref(),
            Some("PINNED")
        );
        assert_eq!(
            help(&rail, "places-heading-devices").as_deref(),
            Some("DEVICES")
        );
        let full = mount("c8-full-light");
        assert_eq!(help(&full, "places-heading-pinned"), None);
    }

    // browser/sidebar: "Expanding PINNED returns the open pin to its original slot, and the pin
    // stays visible throughout the animation."
    #[test]
    fn expanding_pinned_keeps_the_open_pin_in_view_throughout() {
        let mut rt = mount("c8-collapsed-light");
        tap(&mut rt, "places-heading-pinned");
        for frame in 0..30 {
            let pin = element(&rt, "places-pin-photos")
                .unwrap_or_else(|| panic!("the open pin is gone at frame {frame}"));
            assert!(pin.opacity > 0.99, "the open pin fades at frame {frame}");
            rt.frame(FRAME);
        }
        assert!(element(&rt, "places-pin-projects").is_some());
    }

    // browser/sidebar: "A heading focused from the keyboard shows its accent outline."
    #[test]
    fn a_heading_reached_from_the_keyboard_shows_its_ring() {
        let mut rt = mount("c8-full-light");
        press(&mut rt, Key::Tab);
        press(&mut rt, Key::Tab);
        for _ in 0..10 {
            press(&mut rt, Key::Down);
        }
        let heading = element(&rt, "places-heading-pinned").expect("heading");
        assert!(heading.focused && heading.focus_visible);
    }

    /// Collapses PINNED and reports whether DEVICES' heading is still moving a frame later.
    fn devices_move_after_collapse(animations: bool) -> bool {
        let mut rt = mount_with("c8-full-light", |g| g.look.animations = animations);
        tap(&mut rt, "places-heading-pinned");
        rt.frame(FRAME);
        element(&rt, "places-heading-devices")
            .expect("devices heading")
            .moving
    }

    // browser/sidebar: "The fold animates for 200 ms, or not at all with animations disabled."
    #[test]
    fn the_fold_animates_only_with_animations_on() {
        assert!(devices_move_after_collapse(true));
        assert!(!devices_move_after_collapse(false));
    }

    /// Drags a folder over gap 1, re-sampling the pointer every frame as a window does, and
    /// returns the drop target each frame.
    fn targets_while_gap_opens(animations: bool) -> Vec<Option<String>> {
        let mut rt = mount_with("c8-full-light", |g| g.look.animations = animations);
        let at = centre(&rt, "places-band-1");
        rt.drag_files(vec![PathBuf::from("/home/demo/Work")], Some(at));
        let mut seen = Vec::new();
        for _ in 0..30 {
            rt.frame(FRAME);
            rt.drag_files_to(Some(at));
            seen.push(rt.drop_hovered());
        }
        seen
    }

    // browser/sidebar/pins/drag-to-pin: "A folder pin drag that crosses a row's edges and gap,
    // or leaves a spring-loaded row for a gap, keeps one insertion target." The gap opening
    // under a still pointer keeps the target.
    #[test]
    fn an_opening_gap_keeps_its_drop_target() {
        for animations in [true, false] {
            let seen = targets_while_gap_opens(animations);
            assert!(
                seen.iter().all(|t| t.as_deref() == Some("places-band-1")),
                "animations {animations}: {seen:?}"
            );
        }
    }

    /// The gap's height a frame after a drag reaches it.
    fn gap_height_after_a_frame(animations: bool) -> (f32, f32) {
        let mut rt = mount_with("c8-full-light", |g| g.look.animations = animations);
        let at = centre(&rt, "places-band-1");
        rt.drag_files(vec![PathBuf::from("/home/demo/Work")], Some(at));
        rt.frame(FRAME);
        rt.frame(FRAME);
        let first = element(&rt, "places-gap-1").expect("gap").rect[3];
        run(&mut rt, 1.0);
        (first, element(&rt, "places-gap-1").expect("gap").rect[3])
    }

    // C8 work package: the open-gap animation, which with animations off opens at once.
    #[test]
    fn the_gap_opens_on_a_spring_or_at_once_with_animations_off() {
        let (first, open) = gap_height_after_a_frame(true);
        assert!(first < open, "animated: {first} then {open}");
        let (first, open) = gap_height_after_a_frame(false);
        assert_eq!(first, open, "at once");
    }
}
