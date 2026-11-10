//! C23's page in process: the demo's flight from a real C1e row into C8's real Trash, and the
//! page's own part of the dissolve (survivors hold still until it ends). The scripts in
//! `tests/scripts/c23/` check the bullets' end states.

mod c23 {
    use rune::{Env, Runtime};
    use strata_components::motion::{flyer_key, launch_key};
    use strata_components::sidebar::TRASH_TARGET;
    use strata_gallery::scenes::c23::Op;
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
        for _ in 0..30 {
            rt.frame(FRAME);
        }
        rt
    }

    /// The drawn rect of the element whose key path ends in `path` (`row-4/icon`).
    fn find(rt: &Runtime<Gallery>, path: &str) -> Option<[f32; 4]> {
        let path = format!("/{path}");
        rt.elements()
            .into_iter()
            .find(|e| !e.exiting && e.path.ends_with(&path))
            .map(|e| e.rect)
    }

    fn rect(rt: &Runtime<Gallery>, path: &str) -> [f32; 4] {
        find(rt, path).unwrap_or_else(|| panic!("no element {path}"))
    }

    fn middle(r: [f32; 4]) -> [f32; 2] {
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }

    fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
        (a[0] - b[0]).hypot(a[1] - b[1])
    }

    /// Selects the entry at `index` and asks for `op`, as its button does.
    fn ask(rt: &mut Runtime<Gallery>, index: usize, op: Op) {
        rt.update(|g, _| {
            let d = &mut g.c23;
            d.coll.selection.select_only(&d.listing, index);
            d.ask = Some((op, 0.0));
        });
        rt.frame(FRAME);
    }

    #[test]
    fn the_demo_flies_from_a_list_row_into_the_sidebar_trash() {
        let mut rt = mount("c23-rest-light");
        let row = "rows/row-4";
        let icon = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("icon") && e.path.contains("/row-4/"))
            .expect("row 4's icon")
            .rect;
        ask(&mut rt, 3, Op::Trash);
        rt.frame(FRAME);
        let id = rt.state.c23.owed.as_ref().expect("a flight plays").0;
        let launch = rect(&rt, &launch_key(id, 0));
        assert!(
            distance(middle(launch), middle(icon)) < 1.0,
            "it leaves from the row's icon: {launch:?} over {icon:?}"
        );
        let trash = middle(rect(&rt, TRASH_TARGET));
        let mut last = f32::MAX;
        let mut flew = 0;
        for _ in 0..30 {
            rt.frame(FRAME);
            let Some(r) = find(&rt, &flyer_key(id, 0)) else {
                continue;
            };
            let d = distance(middle(r), trash);
            assert!(d <= last + 0.01, "it closes on the Trash: {d} after {last}");
            last = d;
            flew += 1;
        }
        assert!(
            flew > 10 && last < 4.0,
            "it reached the Trash ({flew} frames, {last} off)"
        );
        assert!(find(&rt, row).is_none(), "the row went at launch");
        for _ in 0..30 {
            rt.frame(FRAME);
        }
        assert!(rt.state.c23.owed.is_none(), "the flight ended");
        assert_eq!(rt.state.c23.trashed.len(), 1);
    }

    #[test]
    fn survivors_hold_their_place_until_the_dissolve_ends() {
        let mut rt = mount("c23-rest-light");
        let below = "rows/row-8";
        let before = rect(&rt, below);
        ask(&mut rt, 3, Op::Delete);
        for _ in 0..30 {
            rt.frame(FRAME);
            assert_eq!(rect(&rt, below), before, "the rows below hold still");
        }
        assert!(rt.state.c23.owed.is_some(), "still dissolving");
        for _ in 0..10 {
            rt.frame(FRAME);
        }
        assert!(rt.state.c23.owed.is_none());
        let after = rect(&rt, below);
        assert!(
            after[1] < before[1] - 1.0,
            "then they take the deleted row's place at once: {after:?}"
        );
    }
}
