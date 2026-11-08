//! C14b's page: what its viewers do in process, beside its goldens and scripts. These cover the
//! bullets a script can't check: motion, a theme change, a press at a given point.

mod c14b {
    use rune::{ElementInfo, Env, Runtime};
    use strata_components::viewers::model::CameraView;
    use strata_gallery::{Gallery, scenes};
    use strata_kit::look::Look;

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
        frames(&mut rt, 30);
        rt
    }

    fn frames(rt: &mut Runtime<Gallery>, n: usize) {
        for _ in 0..n {
            rt.frame(DT);
        }
    }

    fn element(rt: &Runtime<Gallery>, key: &str) -> ElementInfo {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("no element {key}"))
    }

    fn click(rt: &mut Runtime<Gallery>, x: f32, y: f32) {
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        rt.pointer_up(x, y);
        rt.frame(DT);
    }

    fn click_key(rt: &mut Runtime<Gallery>, key: &str) {
        let r = element(rt, key).rect;
        click(rt, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    }

    /// Presses Next, then reports whether the indicator's dot was turning 0.1 s on, and lets the
    /// render land.
    fn next_sweeps(rt: &mut Runtime<Gallery>) -> bool {
        click_key(rt, "c14b-model-next");
        frames(rt, 5);
        let moving = element(rt, "c14b-model-camera-dot").moving;
        frames(rt, 40);
        moving
    }

    /// `preview/preview-panel/model-views`: "With animations on, the dot sweeps to the new
    /// position in 240 ms along the shorter turn." (The shorter turn is the state's unwrapped
    /// angle: `viewers::model::tests`.)
    #[test]
    fn the_indicator_sweeps_to_the_new_view() {
        let mut rt = scene("c14b-model-light");
        assert!(next_sweeps(&mut rt), "the dot turns");
        assert_eq!(rt.state.c14b.model.camera(), CameraView::BackLeft);
        assert!(
            !element(&rt, "c14b-model-camera-dot").moving,
            "and comes to rest"
        );
    }

    /// `preview/preview-panel/model-views`: "With Reduce motion on, system animations off, or the
    /// preview unmapped, the dot jumps to the new position."
    #[test]
    fn the_indicator_jumps_without_motion() {
        let mut rt = scene("c14b-model-light");
        rt.set_system_reduced_motion(true);
        assert!(!next_sweeps(&mut rt), "reduced motion: the dot jumps");

        let mut rt = scene("c14b-model-light");
        rt.state.look.animations = false;
        assert!(!next_sweeps(&mut rt), "animations off: the dot jumps");

        // A new preview mounts its indicator at the view, with nothing to sweep from.
        let rt = scene("c14b-model-rendering-light");
        assert!(!element(&rt, "c14b-model-camera-dot").moving);
    }

    /// `preview/preview-panel/model-views`: "Changing the theme re-renders the open model preview
    /// with the new accent and keeps the selected camera view." The render is the host's; the
    /// view keeps the camera across the new look.
    #[test]
    fn a_theme_change_keeps_the_camera_view() {
        let mut rt = scene("c14b-model-light");
        next_sweeps(&mut rt);
        rt.update(|g, _| g.look = Look::fallback(true));
        frames(&mut rt, 30);
        assert_eq!(rt.state.c14b.model.camera(), CameraView::BackLeft);
        let tree = rt.accessibility_json().to_string();
        assert!(tree.contains("Back left"), "the indicator reads Back left");
        assert!(!element(&rt, "c14b-model-next").moving);
    }

    /// `operations/archives/preview`: "A click at a row's top edge selects it and opens a
    /// folder." Rows hit-test by index geometry, so a row's first point is the row.
    #[test]
    fn a_click_at_a_rows_top_edge_opens_its_folder() {
        let mut rt = scene("c14b-archive-light");
        let docs = element(&rt, "row-8000000006").rect;
        click(&mut rt, docs[0] + docs[2] / 2.0, docs[1] + 0.5);
        frames(&mut rt, 5);
        let a = rt.state.c14b.archive.as_ref().expect("the archive");
        assert_eq!(a.listing().crumbs(), ["docs"]);
    }
}
