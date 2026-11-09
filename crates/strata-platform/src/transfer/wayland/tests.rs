use std::os::unix::net::UnixStream;

use wayland_backend::client::Backend;

use super::*;

/// A connection with no server behind it, as in the offer tests.
fn nowhere() -> (Backend, Home) {
    let (ours, _theirs) = UnixStream::pair().expect("socket pair");
    let backend = Backend::connect(ours).expect("libwayland-client");
    let home = Home {
        backend: backend.downgrade(),
        ui: Some(std::thread::current().id()),
    };
    (backend, home)
}

/// P0's review, S3: a dead connection forgets the drag over a window and the dropped offers
/// not yet finished, as it forgets selections, so nothing on it is read or finished later.
#[test]
fn a_dead_connection_forgets_drags_and_dropped_offers() {
    let (_backend, home) = nowhere();
    let mut st = State::new(Kind::Shared, home.clone(), Arc::default());
    let mimes = vec!["text/plain".to_owned()];
    st.drag = Some(Drag {
        offer: DragOffer::closed(mimes.clone(), home.clone()),
        window: Some("main".into()),
    });
    st.dropped
        .push(DragOffer::closed(mimes.clone(), home.clone()));
    st.selections.insert(
        (Selection::Clipboard, Route::DataDevice),
        Current {
            offer: WlSelection::new(None, mimes, home),
        },
    );
    st.forget_all();
    assert!(st.drag.is_none(), "the drag over the window is forgotten");
    assert!(st.dropped.is_empty(), "dropped offers are forgotten");
    assert!(st.selections.is_empty());
}
