//! [`Dnd`]: how a component attaches its drop zones and drag sources to elements, as the host
//! chooses.

use std::rc::Rc;

use rune::{DragItem, DragOperation, El};

use super::{
    DragPayload, DragSource, DropActions, DropData, DropHover, DropOffer, DropZone, Dropped,
};
use super::{URI_LIST, uri_to_path};

type DropBind<S> = Rc<dyn Fn(El, DropZone<S>) -> El>;
type DragBind<S> = Rc<dyn Fn(El, DragSource<S>) -> El>;

/// Attaches [`DropZone`]s and [`DragSource`]s to elements. The host passes one to each
/// component that has drop targets or drag sources, and the component calls it for each.
///
/// Components never depend on the system drag-and-drop shim (work README, rule 1). This is
/// how its handlers reach their elements anyway: X2 passes a [`Dnd::custom`] binder that wires
/// the shim.
///
/// - [`Dnd::rune`], the default, wires Rune's file drops (`on_drop_files` and its hover, move
///   and reject hooks) and `drag_out`. Rune takes file drags on X11 and macOS, and in headless
///   runs, where scripts drop with `drag_files` and `drop_files`; winit reports none on
///   Wayland. Its `drag_out` starts system drags on macOS only, and elsewhere leaves the element
///   as it was.
/// - [`Dnd::none`] attaches nothing, for a read-only collection (C14b's archive preview).
///
/// Used by: C1e, C6, C7, C8, C14b, X2.
pub struct Dnd<S> {
    drop: DropBind<S>,
    drag: DragBind<S>,
}

impl<S> Clone for Dnd<S> {
    fn clone(&self) -> Self {
        Self {
            drop: self.drop.clone(),
            drag: self.drag.clone(),
        }
    }
}

impl<S: 'static> Default for Dnd<S> {
    fn default() -> Self {
        Self::rune()
    }
}

impl<S: 'static> Dnd<S> {
    /// Rune's own file drops and drag-out.
    ///
    /// A file drop offers `text/uri-list`, comes from another process, and allows copy and
    /// move: Rune doesn't say what the source allows. The zone gets the files as one uri-list
    /// item, with `tx.mods()` and `tx.drop_point()`. Rune negotiates no action, so
    /// [`DropZone::action`] is never asked.
    pub fn rune() -> Self {
        Self::custom(rune_drop_zone, rune_drag_source)
    }

    /// Attaches nothing: elements keep only their own handlers.
    pub fn none() -> Self {
        Self::custom(|el, _| el, |el, _| el)
    }

    /// A binder of the host's own: `drop` attaches a zone to an element, `drag` a source.
    pub fn custom(
        drop: impl Fn(El, DropZone<S>) -> El + 'static,
        drag: impl Fn(El, DragSource<S>) -> El + 'static,
    ) -> Self {
        Self {
            drop: Rc::new(drop),
            drag: Rc::new(drag),
        }
    }

    /// `el`, as a drop target for `zone`.
    pub fn drop_zone(&self, el: El, zone: DropZone<S>) -> El {
        (self.drop)(el, zone)
    }

    /// `el`, as a drag source for `source`.
    pub fn drag_source(&self, el: El, source: DragSource<S>) -> El {
        (self.drag)(el, source)
    }
}

/// What a Rune file drag offers.
fn file_offer() -> DropOffer {
    DropOffer::new(
        vec![URI_LIST.to_owned()],
        DropActions::COPY | DropActions::MOVE,
    )
}

fn rune_drop_zone<S: 'static>(el: El, zone: DropZone<S>) -> El {
    let offer = file_offer();
    if zone.accepts(&offer) {
        let z = zone.clone();
        let mut el = el.on_drop_files(move |s: &mut S, files, tx| {
            let dropped = Dropped::new(file_offer(), DropData::files(files), tx.mods())
                .point(tx.drop_point());
            z.deliver(s, dropped, tx);
        });
        let z = zone.clone();
        el = el.on_drop_hover(move |s: &mut S, over, tx| {
            z.hover(
                s,
                if over {
                    DropHover::Accepting
                } else {
                    DropHover::Left
                },
                tx,
            );
        });
        if zone.follows_moves() {
            el = el.on_drop_move(move |s: &mut S, point, tx| zone.moved(s, point, tx));
        }
        el
    } else if zone.rejects() {
        // A target that takes none of the files, and stops the drag rather than passing it on.
        let z = zone.clone();
        el.on_drop_files(|_: &mut S, _, _| {})
            .accepts_files(|_| false)
            .on_drop_rejected(move |s: &mut S, _, tx| z.reject(s, file_offer(), tx))
            .on_drop_rejected_hover(move |s: &mut S, over, tx| {
                zone.hover(
                    s,
                    if over {
                        DropHover::Rejecting
                    } else {
                        DropHover::Left
                    },
                    tx,
                );
            })
    } else {
        el
    }
}

fn rune_drag_source<S: 'static>(el: El, source: DragSource<S>) -> El {
    let src = source.clone();
    let el = el.drag_out(move |s: &S| src.payload(s).and_then(|p| drag_item(&p)));
    if source.hears_end() {
        el.on_drag_end(move |s: &mut S, outcome, tx| source.end(s, outcome, tx))
    } else {
        el
    }
}

/// Rune's drag item for `payload`: its local files, any other URIs as text, and its actions.
/// `None` when it allows nothing or carries nothing Rune can offer.
pub(super) fn drag_item(payload: &DragPayload) -> Option<DragItem> {
    let ops = [
        (DropActions::COPY, DragOperation::Copy),
        (DropActions::MOVE, DragOperation::Move),
        (DropActions::LINK, DragOperation::Link),
    ];
    let mut allowed = ops
        .iter()
        .filter(|(a, _)| payload.actions.contains(*a))
        .map(|&(_, op)| op);
    let mut item = DragItem::new().operation(allowed.next()?);
    for op in allowed {
        item = item.allow(op);
    }
    let mut others = Vec::new();
    for uri in &payload.uris {
        match uri_to_path(uri) {
            Some(path) => item = item.file(path),
            None => others.push(uri.as_str()),
        }
    }
    if !others.is_empty() {
        item = item.text(others.join("\n"));
    }
    (!item.is_empty()).then_some(item)
}
