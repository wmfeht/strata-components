//! [`Dnd`]: how a component attaches its drop zones and drag sources to elements, as the host
//! chooses.

use std::cell::Cell;
use std::rc::Rc;

use rune::{DragItem, DragOperation, El};

use super::{
    DragPayload, DragSource, DropActions, DropData, DropHover, DropOffer, DropZone, Dropped,
};
use super::{URI_LIST, file_uri, uri_list_only, uri_to_path};

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
    /// item, with `tx.mods()` and `tx.drop_point()`.
    ///
    /// - Rune's hover handlers hear no paths, so offers carry [`DropOffer::uris`] only on the
    ///   drop.
    /// - Rune reports no drag starts, so a drag gets a new [`DropOffer::serial`] each time it
    ///   reaches a zone. Its drop shares the serial of the hover before it.
    /// - Rune routes drags with no state, so [`DropZone::action`] is asked only at the drop. A
    ///   refusal there goes to the zone's `on_reject`, or nowhere.
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

thread_local! {
    /// The serial of the Rune file drag last heard, and whether it is still live (no drop yet).
    /// Shared by every zone the Rune binder wires on this thread, which is the UI thread.
    static RUNE_DRAG: Cell<(u64, bool)> = const { Cell::new((0, false)) };
}

/// Rune reports no drag starts, so a drag's serial is taken from what zones hear. A zone
/// reached (`on_drop_hover(true)`, or its rejected twin) starts a new serial: one drag moving
/// across zones gets several, but two drags never share one. A leave keeps it, so the drop that
/// follows a hover shares the hover's serial.
fn hover_serial(over: bool) -> u64 {
    RUNE_DRAG.with(|d| {
        let (serial, _) = d.get();
        if over {
            d.set((serial + 1, true));
            serial + 1
        } else {
            serial
        }
    })
}

/// The serial of a drop: the live drag's, else a new one (a drop no hover came before, which
/// only headless runs send). The drag ends with it.
fn drop_serial() -> u64 {
    RUNE_DRAG.with(|d| {
        let (serial, live) = d.get();
        let serial = if live { serial } else { serial + 1 };
        d.set((serial, false));
        serial
    })
}

/// What a Rune file drag offers: files, as copy or move, since Rune doesn't say what the
/// source allows.
fn file_offer(serial: u64) -> DropOffer {
    DropOffer::new(uri_list_only(), DropActions::COPY | DropActions::MOVE).serial(serial)
}

fn rune_drop_zone<S: 'static>(el: El, zone: DropZone<S>) -> El {
    // Rune's drags all offer `text/uri-list` and come from another process, so reading it is
    // the whole acceptance test, with no offer to build per element.
    if zone.reads(URI_LIST) {
        let z = zone.clone();
        let mut el = el.on_drop_files(move |s: &mut S, files, tx| {
            let uris = files.iter().map(|p| file_uri(p)).collect();
            let offer = file_offer(drop_serial()).uris(uris);
            if z.choose(s, &offer, tx.mods()).is_none() {
                // Refused by its `action`: Rune has already routed the drop here, so it can't
                // pass it on. Only `on_reject` hears it.
                z.reject(s, offer, tx);
                return;
            }
            let dropped =
                Dropped::new(offer, DropData::files(files), tx.mods()).point(tx.drop_point());
            z.deliver(s, dropped, tx);
        });
        // Wired even for a zone that doesn't listen, so every drag reaching a zone gets a serial.
        let z = zone.clone();
        el = el.on_drop_hover(move |s: &mut S, over, tx| {
            let offer = file_offer(hover_serial(over));
            z.hover(
                s,
                if over {
                    DropHover::Accepting
                } else {
                    DropHover::Left
                },
                &offer,
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
            .on_drop_rejected(move |s: &mut S, files, tx| {
                let uris = files.iter().map(|p| file_uri(p)).collect();
                z.reject(s, file_offer(drop_serial()).uris(uris), tx);
            })
            .on_drop_rejected_hover(move |s: &mut S, over, tx| {
                let offer = file_offer(hover_serial(over));
                let hover = if over {
                    DropHover::Rejecting
                } else {
                    DropHover::Left
                };
                zone.hover(s, hover, &offer, tx);
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
