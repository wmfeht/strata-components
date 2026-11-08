//! The guard of a drag source against Rune's capture, headless at the pinned Rune rev: the
//! workaround for U10 (draft, not filed) relies on Rune ending a held drag on Escape, and on
//! `Tx::send_key` delivering one. If a Rune move changes either, these fail.

use rune::{Cx, El, Env, KeyEvent, Phase, Runtime, Tx, col, el};

use super::hub::tests::FAKE_START;
use super::session::Release;
use super::*;

struct S {
    dnd: Dnd<S>,
    phases: Vec<(Phase, bool)>,
    started: u32,
    keys: Vec<String>,
}

fn view(s: &S, _: &mut Cx<S>) -> El {
    let source = DragSource::new(|_: &S, _| Some(DragItem::new().text("hello")))
        .on_start(|s: &mut S, _| s.started += 1)
        .on_drag(|s: &mut S, d: &mut rune::Drag, _| s.phases.push((d.phase, d.cancelled())));
    col()
        .w_full()
        .h_full()
        .child(s.dnd.drag_source(el().size(100.0, 60.0), "tile", source))
}

fn heard_key(s: &mut S, ev: &KeyEvent, _: &mut Tx) -> bool {
    s.keys.push(format!("{:?}", ev.key));
    false
}

fn runtime(release: Release) -> (Runtime<S>, (f32, f32)) {
    FAKE_START.set(Some(release));
    let state = S {
        dnd: Dnd::new(),
        phases: Vec::new(),
        started: 0,
        keys: Vec::new(),
    };
    let mut rt = Runtime::new(state, view, (300.0, 200.0), Env::new(true, 1.0));
    rt.on_key = Some(heard_key);
    rt.frame(0.0);
    let at = rt.find("tile").expect("the tile");
    (rt, at)
}

/// Wayland: once the system drag starts, the synthetic Escape ends Rune's capture at once
/// (no focus bounce needed), the cancelled Up it brings is swallowed, the Escape reaches no
/// key handler, and the pointer coming back with no button reaches the element no more.
#[test]
fn on_wayland_the_escape_ends_runes_capture_at_the_start() {
    let (mut rt, (x, y)) = runtime(Release::Escape);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(x + 4.0, y);
    rt.frame(0.0);
    assert_eq!(rt.state.started, 0, "under the threshold it's a press");
    rt.pointer_move(x + 12.0, y);
    rt.frame(0.0);
    rt.frame(0.0);
    assert_eq!(rt.state.started, 1, "past the threshold the drag starts");
    assert!(!rt.pressing(), "Rune's capture ended with the start");
    // The compositor takes the pointer, then gives it back with no button held.
    rt.pointer_leave();
    rt.pointer_move(x + 40.0, y + 20.0);
    rt.frame(0.0);
    rt.pointer_move(x + 45.0, y + 25.0);
    rt.frame(0.0);
    assert!(!rt.pressing());
    assert_eq!(
        rt.state.phases,
        vec![(Phase::Down, false), (Phase::Move, false)],
        "the element's own handler heard the press and the move under the threshold only"
    );
    assert!(
        rt.state.keys.is_empty(),
        "the Escape leaked: {:?}",
        rt.state.keys
    );
    // The next press is the element's again.
    rt.pointer_down(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.phases.last(), Some(&(Phase::Down, false)));
}

/// X11: Rune keeps the capture through the drag (winit has the grab), the real release is
/// swallowed, and a user's Escape cancels the press, which the guard turns into a cancel.
#[test]
fn on_x11_the_real_release_is_swallowed() {
    let (mut rt, (x, y)) = runtime(Release::RealUp);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 12.0, y);
    rt.frame(0.0);
    assert_eq!(rt.state.started, 1);
    assert!(
        rt.pressing(),
        "no Escape on X11: the capture runs to the release"
    );
    rt.pointer_move(x + 30.0, y);
    rt.pointer_up(x + 30.0, y);
    rt.frame(0.0);
    assert!(!rt.pressing());
    assert_eq!(
        rt.state.phases,
        vec![(Phase::Down, false)],
        "moves and the release after the start are the system drag's"
    );
    assert!(rt.state.keys.is_empty(), "{:?}", rt.state.keys);
}
