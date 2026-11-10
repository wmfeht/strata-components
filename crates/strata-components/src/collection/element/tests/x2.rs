//! The hooks X2 binds the system drag through: the container's drag phases
//! ([`Collection::on_drag_phase`]) and the drop zones' action ([`Collection::drop_action`]).

use std::cell::RefCell;

use rune::{Cx, El, Env, Mods, Runtime};
use strata_kit::look::Look;

use super::fixture::FastSource;
use crate::collection::element::{Collection, CollectionState, Intent};
use crate::collection::pointer::PointerIntent;
use crate::contract::{Dnd, DropAction, DropActions, DropOffer, DropZone, EntryId, URI_LIST};

const SCOPE: &str = "pane";

struct Host {
    src: FastSource,
    coll: CollectionState,
    look: Look,
    /// Every phase the hook heard, with the key it was given.
    phases: Vec<(rune::Phase, String)>,
    /// The hook takes phases from this one on, once a drag began (as P1's `owns` does after
    /// `start_drag`).
    taking: bool,
    began: usize,
    intents: Vec<String>,
}

thread_local! {
    /// The zones the recording binder attached, by key.
    static ZONES: RefCell<Vec<(String, DropZone<Host>)>> = const { RefCell::new(Vec::new()) };
}

/// Records each zone attached, the latest for each key: rows are memoised, so a view run that
/// rebuilds none attaches none.
fn binder() -> Dnd<Host> {
    Dnd::custom_keyed(
        |el, key, zone| {
            ZONES.with(|z| {
                let mut z = z.borrow_mut();
                z.retain(|(k, _)| k != key.key());
                z.push((key.key().to_owned(), zone));
            });
            el
        },
        |el, _, _| el,
    )
}

fn view(s: &Host, _: &mut Cx<Host>) -> El {
    let c = Collection::new(
        SCOPE,
        |s: &Host| (&s.src, &s.coll),
        |s: &mut Host| (&s.src, &mut s.coll),
        |s: &Host| &s.look,
        |_: &Host| 1,
    )
    .dnd(binder())
    .on_drop(|_: &mut Host, _, _, _| {})
    .drop_action(|_: &Host, target, offer, mods| match target {
        // Folder 0 is the drag's own: refused; others move unless Ctrl.
        Some(id) if id == EntryId::new(0) => None,
        Some(_) if mods.ctrl => Some(DropAction::Copy),
        Some(_) => Some(DropAction::Move),
        None if offer.local => None,
        None => Some(DropAction::Copy),
    })
    .on_drag_phase(|s: &mut Host, d, key, _| {
        s.phases.push((d.phase, key.to_owned()));
        s.taking
    })
    .on_intent(|s: &mut Host, intent, _| {
        if let Intent::BeginDrag { .. } = intent {
            s.began += 1;
            s.taking = true;
        }
        s.intents.push(format!("{intent:?}"));
    });
    rune::col().size(400.0, 300.0).child(c.build(s))
}

fn runtime() -> Runtime<Host> {
    let mut coll = CollectionState::new();
    coll.controllers.pointer = Some(Box::new(PointerIntent::new()));
    let host = Host {
        src: FastSource::new(30),
        coll,
        look: Look::fallback(false),
        phases: Vec::new(),
        taking: false,
        began: 0,
        intents: Vec::new(),
    };
    let mut rt = Runtime::new(host, view, (400.0, 300.0), Env::new(false, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    rt
}

/// A point on row `key`'s icon: content, where a press and a move start a file drag.
fn on_icon(rt: &Runtime<Host>, key: &str) -> (f32, f32) {
    let e = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no {key}"));
    let [x, y, _, h] = e.visible;
    (x + 20.0, y + h / 2.0)
}

fn step(rt: &mut Runtime<Host>) {
    rt.frame(1.0 / 60.0);
}

/// A drag the host starts from `BeginDrag` owns the phases after it: the hook hears every
/// phase with the collection's key, and the release it takes is no click.
#[test]
fn a_taken_release_after_a_drag_begins_is_no_click_and_the_hook_hears_the_key() {
    let mut rt = runtime();
    let (x, y) = on_icon(&rt, "row-3");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    step(&mut rt);
    rt.pointer_move(x, y + 30.0);
    step(&mut rt);
    rt.pointer_move(x, y + 60.0);
    step(&mut rt);
    rt.pointer_up(x, y + 60.0);
    step(&mut rt);
    assert_eq!(rt.state.began, 1, "{:?}", rt.state.intents);
    let phases: Vec<rune::Phase> = rt.state.phases.iter().map(|(p, _)| *p).collect();
    assert_eq!(phases.first(), Some(&rune::Phase::Down));
    assert_eq!(phases.last(), Some(&rune::Phase::Up));
    assert!(rt.state.phases.iter().all(|(_, k)| k == SCOPE));
    // Only the press moved the selection: the drag selected row 3, nothing after it did.
    let sel = rt.state.coll.selection.selected_ids(&rt.state.src);
    assert_eq!(sel, [EntryId::new(3)]);
    assert!(
        !rt.state.intents.iter().any(|i| i.starts_with("Clicked")),
        "{:?}",
        rt.state.intents
    );
}

/// A release the hook takes is a cancelled one even with no drag: a plain press that the
/// host's drag owns finishes with no click.
#[test]
fn a_release_the_hook_takes_is_cancelled() {
    let mut rt = runtime();
    let (x, y) = on_icon(&rt, "row-4");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    step(&mut rt);
    rt.state.taking = true;
    rt.pointer_up(x, y);
    step(&mut rt);
    assert!(
        !rt.state.intents.iter().any(|i| i.starts_with("Clicked")),
        "{:?}",
        rt.state.intents
    );
    // Without the hook taking it, the same press is a click.
    rt.state.taking = false;
    rt.pointer_down(x, y);
    step(&mut rt);
    rt.pointer_up(x, y);
    step(&mut rt);
    assert!(
        rt.state.intents.iter().any(|i| i.starts_with("Clicked")),
        "{:?}",
        rt.state.intents
    );
}

fn zone(key: &str) -> DropZone<Host> {
    ZONES.with(|z| {
        z.borrow()
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, z)| z.clone())
            .unwrap_or_else(|| panic!("no zone {key}"))
    })
}

/// The host's action reaches each folder row's zone with the row's id, and the background's
/// with none: a binder that negotiates while hovering (P1's) shows it, or refuses the drag.
#[test]
fn the_drop_action_answers_for_each_folder_row_and_the_background() {
    let rt = runtime();
    let both = DropActions::COPY | DropActions::MOVE;
    let files = DropOffer::new(vec![URI_LIST.into()], both);
    let plain = Mods::default();
    let ctrl = Mods {
        ctrl: true,
        ..Mods::default()
    };
    let s = &rt.state;
    assert_eq!(
        zone("row-0").choose(s, &files, plain),
        None,
        "its own folder"
    );
    assert_eq!(
        zone("row-10").choose(s, &files, plain),
        Some(DropAction::Move)
    );
    assert_eq!(
        zone("row-10").choose(s, &files, ctrl),
        Some(DropAction::Copy)
    );
    assert_eq!(zone(SCOPE).choose(s, &files, plain), Some(DropAction::Copy));
    assert_eq!(
        zone(SCOPE).choose(s, &files.clone().local(true), plain),
        None
    );
}

/// A move the hook takes never reaches the controllers: past the threshold, no drag begins.
#[test]
fn moves_the_hook_takes_start_nothing() {
    let mut rt = runtime();
    let (x, y) = on_icon(&rt, "row-5");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    step(&mut rt);
    rt.state.taking = true;
    rt.pointer_move(x, y + 40.0);
    step(&mut rt);
    rt.pointer_move(x, y + 80.0);
    step(&mut rt);
    assert_eq!(rt.state.began, 0, "{:?}", rt.state.intents);
}
