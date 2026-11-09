//! W1e: every drop zone C1e, C2, C6, C7 and C8 attach reaches a keyed binder (as X2's is) with
//! the key its element carries, unique in the window or inside the scope it names. Each test
//! mounts a page's own component state with a binder that records what it hears.

mod w1e {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use rune::{Cx, El, ElementInfo, Env, Runtime, col};
    use strata_components::contract::{Dnd, ElementKey};
    use strata_components::location_bar::{LocationBar, crumb_key};
    use strata_components::sidebar::{DEFAULT_KEY as SIDEBAR, PlacesSidebar, SidebarData};
    use strata_components::tabs::{
        Accepts, BrowserTabs, BrowserTabsState, Tab, TabId, TabsData, new_tab_key, tab_key,
    };
    use strata_gallery::{Gallery, scenes};
    use strata_kit::icons::IconName;
    use strata_kit::look::Look;

    const FRAME: f64 = 1.0 / 60.0;

    /// A key a binder heard: the element's key and its scope.
    type Heard = (String, Option<String>);

    thread_local! {
        static HEARD: RefCell<Vec<Heard>> = const { RefCell::new(Vec::new()) };
    }

    /// A binder that records each zone's key and attaches nothing. A component built through
    /// it attaches no source here (none of these has one).
    fn recording<S: 'static>() -> Dnd<S> {
        let note = |key: &ElementKey| {
            HEARD.with(|h| {
                h.borrow_mut()
                    .push((key.key().to_owned(), key.scope().map(str::to_owned)))
            });
        };
        Dnd::custom_keyed(
            move |el, key, _| {
                note(key);
                el
            },
            |_, key, _| panic!("no source expected, heard {key:?}"),
        )
    }

    /// The keys heard since the last call, each once.
    fn heard() -> HashSet<Heard> {
        HEARD.with(|h| h.borrow_mut().drain(..).collect())
    }

    fn gallery(scene: &str) -> Gallery {
        let f = scenes::all_scenes()
            .into_iter()
            .find(|(n, _)| *n == scene)
            .map(|(_, f)| f)
            .unwrap_or_else(|| panic!("no scene {scene}"));
        let mut g = Gallery::default();
        f(&mut g);
        g
    }

    fn mount<S: 'static>(state: S, view: fn(&S, &mut Cx<S>) -> El) -> Runtime<S> {
        HEARD.with(|h| h.borrow_mut().clear());
        let mut rt = Runtime::new(state, view, strata_gallery::SIZE, Env::new(false, 1.0));
        for i in 0..30 {
            rt.frame(f64::from(i) * FRAME);
        }
        rt
    }

    /// The keyed segments of `path` (`#n` stands for an unkeyed element).
    fn keyed(path: &str) -> Vec<&str> {
        path.split('/')
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
            .collect()
    }

    /// Each heard key names an element in the tree, inside its scope when it has one, and no
    /// two zones share a key in one scope.
    fn check_heard_against_tree<S>(rt: &Runtime<S>, heard: &HashSet<Heard>) {
        let elements: Vec<ElementInfo> = rt.elements().into_iter().filter(|e| !e.exiting).collect();
        for (key, scope) in heard {
            let found = elements.iter().filter(|e| e.key.as_deref() == Some(key));
            let mut any = false;
            for e in found {
                any = true;
                let segs = keyed(&e.path);
                let at = segs
                    .iter()
                    .rposition(|s| s == key)
                    .expect("its own segment");
                if let Some(sc) = scope {
                    assert!(
                        segs[..at].contains(&sc.as_str()),
                        "{key} isn't under its scope {sc}: {}",
                        e.path
                    );
                }
            }
            assert!(any, "{key} names no element");
        }
        let unscoped: Vec<&String> = heard
            .iter()
            .filter(|(_, s)| s.is_none())
            .map(|(k, _)| k)
            .collect();
        let distinct: HashSet<&&String> = unscoped.iter().collect();
        assert_eq!(unscoped.len(), distinct.len());
    }

    fn assert_has(heard: &HashSet<Heard>, key: &str, scope: Option<&str>) {
        let want = (key.to_owned(), scope.map(str::to_owned));
        assert!(heard.contains(&want), "{want:?} not heard among {heard:?}");
    }

    // ---- C1e: folder rows (scoped by the collection) and the background ----

    fn c1_view(g: &Gallery, _: &mut Cx<Gallery>) -> El {
        let list = scenes::c1::collection().dnd(recording()).build(g);
        col().w_full().h_full().child(list)
    }

    #[test]
    fn a_collections_rows_carry_its_key_as_their_scope() {
        let rt = mount(gallery("c1-ten-rows-light"), c1_view);
        let heard = heard();
        check_heard_against_tree(&rt, &heard);
        let scope = scenes::c1::SCOPE;
        assert_has(&heard, scope, None);
        let rows: Vec<&Heard> = heard
            .iter()
            .filter(|(k, _)| k.starts_with("row-"))
            .collect();
        assert!(!rows.is_empty(), "folder rows have zones: {heard:?}");
        for (k, s) in rows {
            assert_eq!(s.as_deref(), Some(scope), "{k}");
        }
    }

    // ---- C2: each column's rows and background, and the blank strip ----

    fn c2_view(g: &Gallery, _: &mut Cx<Gallery>) -> El {
        let strip = scenes::c2::columns_view().dnd(recording()).build(g);
        col().w_full().h_full().child(strip)
    }

    #[test]
    fn columns_key_the_blank_strip_and_scope_each_columns_rows() {
        let rt = mount(gallery("c2-three-columns-light"), c2_view);
        let heard = heard();
        check_heard_against_tree(&rt, &heard);
        assert_has(&heard, &format!("{}-blank", scenes::c2::KEY), None);
        let backgrounds: HashSet<&String> = heard
            .iter()
            .filter(|(k, s)| s.is_none() && k.as_str() != "c2-blank")
            .map(|(k, _)| k)
            .collect();
        assert!(backgrounds.len() >= 3, "a background per column: {heard:?}");
        let scopes: HashSet<&str> = heard.iter().filter_map(|(_, s)| s.as_deref()).collect();
        assert!(scopes.len() >= 2, "rows in several columns: {heard:?}");
        for s in &scopes {
            assert!(
                backgrounds.iter().any(|b| b.as_str() == *s),
                "{s} is a column's key"
            );
        }
    }

    // ---- C6: each ancestor crumb ----

    const LOC: &str = "w1e-loc";

    fn c6_view(g: &Gallery, cx: &mut Cx<Gallery>) -> El {
        let mut layer = Vec::new();
        let bar = LocationBar::new(
            LOC,
            &g.look,
            &g.c6.trail,
            &g.c6.bar,
            |g: &mut Gallery| &mut g.c6.bar,
            |_: &mut Gallery, _, _| {},
        )
        .dnd(recording())
        .build(cx, &mut layer);
        col().w_full().h_full().child(bar).children(layer)
    }

    #[test]
    fn each_ancestor_crumb_is_keyed_for_the_binder() {
        let g = gallery("c6-deep-light");
        let ancestors = g.c6.trail.crumbs.len() - 1;
        let rt = mount(g, c6_view);
        let heard = heard();
        check_heard_against_tree(&rt, &heard);
        for i in 0..ancestors {
            assert_has(&heard, &crumb_key(LOC, i), None);
        }
        assert!(
            !heard.contains(&(crumb_key(LOC, ancestors), None)),
            "the current crumb takes no drop"
        );
    }

    // ---- C7: tabs, New tab and the strip ----

    const STRIP: &str = "w1e-tabs";

    struct Tabs {
        look: Look,
        state: BrowserTabsState,
    }

    fn tabs() -> [Tab<'static>; 3] {
        [
            Tab::new(TabId(1), "Home", IconName::Folder),
            Tab::new(TabId(2), "Trash", IconName::Trash).accepts(Accepts::NONE),
            Tab::new(TabId(3), "Music", IconName::Folder),
        ]
    }

    fn c7_view(h: &Tabs, cx: &mut Cx<Tabs>) -> El {
        let tabs = tabs();
        let data = TabsData::new(&tabs, Some(TabId(1)));
        let strip = BrowserTabs::new(
            &data,
            &h.state,
            |h: &mut Tabs| &mut h.state,
            |h| &mut h.state.drag,
        )
        .dnd(recording())
        .opens_tabs(|_, offer| offer.local)
        .build(&h.look, cx);
        col().w_full().h_full().child(strip)
    }

    #[test]
    fn tabs_new_tab_and_the_strip_are_keyed_for_the_binder() {
        let h = Tabs {
            look: Look::fallback(false),
            state: BrowserTabsState::new(STRIP),
        };
        let rt = mount(h, c7_view);
        let heard = heard();
        check_heard_against_tree(&rt, &heard);
        assert_has(&heard, &tab_key(STRIP, TabId(3)), None);
        assert_has(&heard, &new_tab_key(STRIP), None);
        assert_has(&heard, STRIP, None);
        assert!(
            !heard.contains(&(tab_key(STRIP, TabId(2)), None)),
            "Trash builds no zone"
        );
    }

    // ---- C8: the sidebar, its rows and its pin gaps ----

    fn c8_view(g: &Gallery, _: &mut Cx<Gallery>) -> El {
        let d = &g.c8;
        let data = SidebarData {
            places: &d.places,
            pins: &d.pins,
            devices: &d.devices,
            current: d.current.as_ref(),
        };
        let sidebar = PlacesSidebar::new(&d.sidebar, |g: &mut Gallery| &mut g.c8.sidebar, data)
            .dnd(recording())
            .build(&g.look);
        rune::row().w_full().h_full().child(sidebar)
    }

    #[test]
    fn the_sidebar_its_rows_and_its_gaps_are_keyed_for_the_binder() {
        let g = gallery("c8-full-light");
        let pins = g.c8.pins.len();
        let rows: Vec<String> =
            g.c8.places
                .iter()
                .map(|p| p.item())
                .chain(g.c8.pins.iter().map(|p| p.item()))
                .map(|id| id.key(SIDEBAR))
                .collect();
        let rt = mount(g, c8_view);
        let heard = heard();
        check_heard_against_tree(&rt, &heard);
        assert_has(&heard, SIDEBAR, None);
        for i in 0..=pins {
            assert_has(&heard, &format!("{SIDEBAR}-band-{i}"), None);
        }
        let rows_heard = rows
            .iter()
            .filter(|k| heard.contains(&((*k).clone(), None)))
            .count();
        assert!(rows_heard > 0, "rows that take drops are keyed: {heard:?}");
    }
}
