use super::*;

fn offered() -> Vec<String> {
    vec!["text/uri-list".into(), "text/plain".into()]
}

#[test]
fn finish_before_a_drop_or_action_is_refused() {
    let mut s = DropState::new(3);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    assert!(s.finish().is_err(), "no drop yet");
    s.on_drop();
    assert!(s.finish().is_err(), "no action yet");
    s.on_action(0);
    assert!(s.finish().is_err(), "the action is none");
    s.on_action(Action::Ask.bits());
    assert!(s.finish().is_err(), "ask isn't settled");
    s.on_action(Action::Copy.bits());
    assert_eq!(s.finish(), Ok(true));
}

#[test]
fn finish_after_accepting_nothing_is_refused() {
    let mut s = DropState::new(3);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    s.accept(None, &offered()).expect("accept nothing");
    s.on_drop();
    s.on_action(Action::Copy.bits());
    assert!(s.finish().is_err());
}

#[test]
fn an_action_outside_the_mask_is_refused() {
    let mut s = DropState::new(3);
    let (copy, mv) = (Action::Copy.bits(), Action::Move.bits());
    assert!(s.set_actions(copy | mv, copy).is_ok());
    assert!(s.set_actions(copy, 0).is_ok(), "no preference is allowed");
    assert!(
        s.set_actions(copy, mv).is_err(),
        "preferred not in the mask"
    );
    assert!(
        s.set_actions(copy | mv, copy | mv).is_err(),
        "two preferred"
    );
    assert!(s.set_actions(8, 0).is_err(), "an unknown bit");
    assert!(
        s.accept(Some("image/png"), &offered()).is_err(),
        "a type not offered"
    );
    assert!(
        DropState::new(2).set_actions(copy, copy).is_err(),
        "version 2 has no actions"
    );
}

#[test]
fn nothing_after_finish() {
    let mut s = DropState::new(3);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    s.on_drop();
    s.on_action(Action::Move.bits());
    assert_eq!(s.finish(), Ok(true));
    assert!(s.finish().is_err());
    assert!(s.accept(Some("text/plain"), &offered()).is_err());
    assert!(s.set_actions(1, 1).is_err());
}

#[test]
fn an_old_offer_is_destroyed_without_finish() {
    let mut s = DropState::new(1);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    s.on_drop();
    assert_eq!(s.finish(), Ok(false));
}

#[test]
fn an_ask_drop_settles_on_the_preferred_action() {
    let mut s = DropState::new(3);
    s.accept(Some("text/plain"), &offered()).expect("accept");
    s.on_action(Action::Ask.bits());
    s.on_drop();
    assert!(s.finish().is_err(), "ask isn't settled yet");
    let (copy, mv) = (Action::Copy.bits(), Action::Move.bits());
    // After the drop the target picks; no further action event comes.
    s.set_actions(mv, mv).expect("settle on move");
    assert_eq!(s.finish(), Ok(true));
    let mut before = DropState::new(3);
    before.on_action(Action::Ask.bits());
    before
        .set_actions(copy, copy)
        .expect("set_actions before the drop");
    before
        .accept(Some("text/plain"), &offered())
        .expect("accept");
    before.on_drop();
    assert!(
        before.finish().is_err(),
        "only a request after the drop settles ask"
    );
}

// ---- offers without a compositor: closed proxies on a connection to nothing ----

use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

use wayland_backend::client::Backend;

/// A connection with no server behind it: offers on it can only be closed ones.
fn nowhere(ui: Option<std::thread::ThreadId>) -> (Backend, Home) {
    let (ours, _theirs) = UnixStream::pair().expect("socket pair");
    let backend = Backend::connect(ours).expect("libwayland-client");
    let home = Home {
        backend: backend.downgrade(),
        ui,
    };
    (backend, home)
}

fn other_thread() -> std::thread::ThreadId {
    std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("thread")
}

#[test]
fn a_read_that_fails_at_once_may_read_again_from_done() {
    let (_backend, home) = nowhere(None);
    let offer = WlSelection::new(None, vec!["text/plain".into()], home);
    let (tx, rx) = mpsc::channel();
    let again = offer.clone();
    // On a thread of its own, so a deadlock fails the test instead of hanging it.
    std::thread::spawn(move || {
        offer.read(
            "text/plain",
            ReadLimits::default(),
            Box::new(move |first| {
                again.read(
                    "text/plain",
                    ReadLimits::default(),
                    Box::new(move |second| {
                        let _ = tx.send((first, second));
                    }),
                );
            }),
        );
    });
    let got = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("no deadlock");
    assert_eq!(got, (Err(ReadError::Gone), Err(ReadError::Gone)));
}

#[test]
fn a_drag_offer_may_be_called_from_a_done_that_ran_at_once() {
    let (_backend, home) = nowhere(None);
    let offer = DragOffer::closed(vec!["text/plain".into()], home);
    let (tx, rx) = mpsc::channel();
    let again = offer.clone();
    std::thread::spawn(move || {
        offer.read("text/plain", ReadLimits::default(), move |first| {
            let finished = again.finish();
            let _ = tx.send((first, finished.is_err()));
        });
    });
    let got = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("no deadlock");
    assert_eq!(got, (Err(ReadError::Gone), true));
}

#[test]
fn offers_on_winits_display_refuse_other_threads() {
    let (_backend, home) = nowhere(Some(other_thread()));
    let selection = WlSelection::new(None, vec!["text/plain".into()], home.clone());
    let (tx, rx) = mpsc::channel();
    selection.read(
        "text/plain",
        ReadLimits::default(),
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)),
        Ok(Err(ReadError::OffUiThread))
    );
    let drag = DragOffer::closed(vec!["text/plain".into()], home);
    assert!(matches!(
        drag.accept(Some("text/plain")),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        drag.set_actions(&[Action::Copy], None),
        Err(Error::Refused(_))
    ));
    assert!(matches!(drag.finish(), Err(Error::Refused(_))));
    // The UI thread (here, this one) may.
    let (_backend, mine) = nowhere(Some(std::thread::current().id()));
    let drag = DragOffer::closed(vec!["text/plain".into()], mine);
    assert!(matches!(
        drag.accept(Some("text/plain")),
        Err(Error::Refused("the drag is gone"))
    ));
}
