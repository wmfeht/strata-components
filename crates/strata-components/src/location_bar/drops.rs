//! An ancestor crumb's drop zone, attached through the host's `Dnd`, and its spring.
//!
//! Spring-loading is the crumb's own model, so a host and X2's binder need nothing more than
//! hover reports: an accepting hover arms it and asks for a wake now, `LocationBarState::tick`
//! stamps its start on the host's clock at that wake, asks for one when the delay ends, and
//! opens the crumb ([`super::Via::Spring`]) then; a leave, a drop or a navigation disarms it. C8 and X2 can adopt the same model for rows and
//! sidebar places.

use std::time::Duration;

use rune::Tx;

use super::build::Ctx;
use super::intent::LocationIntent;
use crate::contract::dnd::{DropHover, DropOffer, DropZone};

pub(super) fn zone<S: 'static>(cx: &Ctx<'_, S>, index: usize, id: &str) -> DropZone<S> {
    let access = cx.access;
    let delay = cx.spring;
    let (drop_emit, hover_emit) = (cx.emit.clone(), cx.emit.clone());
    let (drop_id, hover_id) = (id.to_owned(), id.to_owned());
    let mut zone = DropZone::new(move |s: &mut S, dropped, tx: &mut Tx| {
        access(s).drag_ended();
        let id = drop_id.clone();
        drop_emit(s, LocationIntent::DropInto { id, dropped }, tx);
    })
    .on_hover(move |s: &mut S, hover, _: &DropOffer, tx: &mut Tx| {
        access(s).drag_over(index, &hover_id, hover, delay);
        if let (DropHover::Accepting, Some(_)) = (hover, delay) {
            // The next poll stamps the spring's start on the host's clock and asks for the wake
            // that opens it: a wake asked here, a whole delay out, would be stamped late.
            tx.wake_after(Duration::ZERO);
        }
        let id = hover_id.clone();
        hover_emit(s, LocationIntent::DropHover { id, hover }, tx);
    })
    .on_reject(move |s: &mut S, _, _| access(s).drag_ended());
    if let Some(action) = cx.action {
        let action = action.clone();
        let id = id.to_owned();
        zone = zone.action(move |s: &S, offer, mods| action(s, &id, offer, mods));
    }
    zone
}
