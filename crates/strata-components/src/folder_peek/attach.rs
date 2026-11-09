//! The folder rows: C1e's rest hook, and the anchor in the row a peek is open from.

use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::Duration;

use rune::el;

use super::{Shared, anchor_key};
use crate::collection::element::{Collection, Rest, RowCx};

/// `base` with its folder rows waiting on the pointer while peeking is on.
pub(super) fn rows<S: 'static>(sh: &Rc<Shared<S>>, base: Collection<S>) -> Collection<S> {
    let (delays, report, version, part) = (sh.clone(), sh.clone(), sh.clone(), sh.clone());
    base.rest(
        Rest::new(
            move |s: &S, rcx: &RowCx<'_>| {
                let st = (delays.get)(s);
                if !st.enabled() || !rcx.entry.kind.is_folder_like() {
                    return None;
                }
                let b = delays.behavior;
                // Back from the card onto its folder, the peek stays at once.
                let enter = if st.is_anchor(rcx.scope, rcx.entry.id) {
                    Duration::ZERO
                } else {
                    b.open
                };
                Some((enter, b.close))
            },
            move |s: &mut S, r, tx| {
                let change = (report.get_mut)(s).row_rest(&r);
                report.changed(s, change, tx);
            },
        )
        .part(
            move |s: &S, rcx: &RowCx<'_>| {
                let mut h = std::hash::DefaultHasher::new();
                anchor_dx(&version, s, rcx).map(f32::to_bits).hash(&mut h);
                h.finish()
            },
            move |s: &S, rcx: &RowCx<'_>| {
                let dx = anchor_dx(&part, s, rcx)?;
                Some(
                    el().key(anchor_key(rcx.scope))
                        .pin(Some(dx), Some(0.0), None, Some(0.0))
                        .w(0.0),
                )
            },
        ),
    )
}

/// The anchor's offset in this row, when a peek is open from it.
fn anchor_dx<S>(sh: &Shared<S>, s: &S, rcx: &RowCx<'_>) -> Option<f32> {
    let st = (sh.get)(s);
    if !st.is_anchor(rcx.scope, rcx.entry.id) {
        return None;
    }
    st.placement().map(|p| p.dx)
}
