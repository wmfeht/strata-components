//! Drag and drop on Wayland and X11, with any MIME types, for Rune apps.
//!
//! The API mirrors Rune's: a [`DragSource`] is `drag_out` plus `on_drag_start` and
//! `on_drag_end`, with a [`DragItem`] of any MIME types (each made lazily) and the actions it
//! allows. A [`DropTarget`] is `on_drop_files` with its hover, move and reject hooks, for any
//! MIME type, with the action negotiated as the drag moves. A drag between this process's
//! own windows carries a typed payload ([`DragItem::local`], [`DropTarget::local`]) instead
//! of serialized types. Each window has a [`Dnd`]: attach it from `App::on_window`, and
//! build targets and sources through it.
//!
//! ```ignore
//! dnd.drop_target(el().child(text("Drop here")), "zone",
//!     DropTarget::new(|s: &mut App, data, _| s.files = data.uris())
//!         .mime_types(&["text/uri-list", "text/plain;charset=utf-8"])
//!         .on_hover(|s: &mut App, hover, _, _| s.over = hover != Hover::Left))
//! dnd.drag_source(el().child(text("Drag me")), "tile",
//!     DragSource::new(|s: &App, _| Some(DragItem::new().uris(&s.selected).actions(Actions::COPY | Actions::MOVE)))
//!         .on_end(|s: &mut App, outcome, _| s.last = Some(outcome)))
//! ```
//!
//! Call [`shutdown`] on the UI thread before the event loop ends (before `Tx::quit`, and in
//! the last window's `on_close`): it ends a running drag and tears down P0's substrate while
//! winit's display is alive (see `transfer`).
//!
//! # What Rune lacks
//!
//! Rune issues: none filed yet; the drafts are in `docs/decisions/S5-upstream.md`.
//!
//! - **U1** (draft, not filed): `drag_out` starts nothing off macOS, takes no MIME types and
//!   no actions, and its live-frame element snapshot is `pub(crate)`. P1's source is an
//!   `on_drag` element, and its snapshot icon renders a second `Runtime` ([`DragIcon`]).
//! - **U2** (draft, not filed): Wayland drops reach no handler (winit 0.30 reports none),
//!   and no handler takes MIME types or a position per motion on either backend. P1 hears
//!   drags through P0's `wl_data_device`, and on X11 through `XdndProxy` (which turns off
//!   Rune's own X11 `on_drop_files`: take file drops through P1 there).
//! - **U9** (draft, not filed): no display handle, no hook before winit closes the display,
//!   no every-turn hook. P0 recovers the display and runs its own reader (S2's driver C);
//!   P1 adds neither a queue nor a thread to it (the owner's ruling on S2 Q2).
//! - **U10** (draft, not filed): no public way to end a pointer capture. After a Wayland
//!   drag starts, Rune would stay pressing. **Workaround:** P1 sends Escape through
//!   `Tx::send_key`, which Rune takes as "Escape during a drag cancels it", and swallows the
//!   cancelled `Up` that follows (the owner's ruling on S2 Q3). An Escape that finds no
//!   capture would reach the app's keys, so it goes out only under the checks in `session`:
//!   once the compositor proves it took the pointer (the drag's own `wl_data_device.enter`
//!   on an app window, or a target's `target` or `action` answer), the drag hasn't dropped
//!   or ended, the pressed element is still live where it was, and P1 saw the press. A drag
//!   no proof reaches has its capture ended when it ends. Upstream Rune `dev` (44 commits
//!   past the pin) still has no way to end it. Tests guard it at the pinned Rune rev.
//! - **U3** (draft, not filed): file-drop acceptance sees no app state and no drag identity.
//! - **U-P1-1** (draft, not filed): an element has one handler of each kind, and a shim
//!   can't read an element's key. So [`Dnd::drag_source`] takes the element's `on_drag` and
//!   [`Dnd::drop_target`] its `drawn_rect_into`, and targets are found by the key the app
//!   gives them. Ask: composable handlers, or an extension slot readable from a hit test's
//!   `Picked`.
//! - **U-P1-2** (draft, not filed): `Tx::element_at` gives a path and a key, but no rect and
//!   no point in the element's own coordinates (Rune's own drops have `drop_point`). P1 reads
//!   the target's drawn rect instead, so a point is known only once the target has drawn.
//!
//! # Known limits
//!
//! - **Ctrl and Shift on Wayland** (the owner's ruling): a drag from another client brings
//!   no modifiers (the target has no keyboard focus, and wlroots' drag grab passes none), so
//!   its action is the compositor's: sway and Hyprland don't map modifiers to actions;
//!   mutter and KWin are hand tests pending. Ctrl and Shift are honoured on X11 in both
//!   directions (throughout the drag), at the start of a Wayland drag out (a source's
//!   actions are set once, before `start_drag`), and in a drag between this process's own
//!   windows (as held at its start).
//! - **Link is X11 only:** Wayland's data device has no link action.
//! - **Escape on Wayland:** the compositor's drag grab takes the keyboard, and wlroots
//!   compositors (sway) don't cancel on Escape; [`Dnd::cancel_drag`] is the only route
//!   there. Other compositors' Escape is a hand test pending.
//! - **X11 cursor:** the source is grab-less (winit holds the grab), so the cursor shows no
//!   copy or move feedback; the icon follows the pointer.
//! - **libwayland < 1.23** (Ubuntu 24.04): winit's display is out of reach, and so is drag
//!   and drop on Wayland ([`Status::unsupported`]).
//!
//! # Rune's capture on Wayland
//!
//! Every path where Rune's capture of a drag's press ends late, or P1's Escape could reach
//! the app's keys:
//!
//! 1. **Focus lost before the proof's update.** Keyboard focus leaves the window for a
//!    reason other than the drop (the user switching windows, a window mapping and taking
//!    focus) between the drag's start and the update that handles the compositor's proof
//!    (normally a few milliseconds: one P0 dispatch and one window update). Rune ends the
//!    capture at that focus loss; if its cancelled `Up` hasn't reached P1 by the update, the
//!    Escape finds no capture and the app's keys hear one Escape.
//! 2. **Focus lost before the late Escape.** For a drag no proof reached: the same, for a
//!    focus loss in the late wait (150 ms) after the drag ended whose cancelled `Up`
//!    hasn't reached P1 when the late Escape goes out. sway's own focus change at the drop
//!    comes about 14 ms after it, and its `Up` is handled in the next frame, well inside
//!    the wait.
//! 3. **No proof, no focus change: the capture outlives the drag by the late wait.** For
//!    those 150 ms after a drag no proof reached, hover stays frozen and a user's Escape
//!    ends the capture instead of reaching the app's keys.
//! 4. **The source left its place.** The element the drag started from leaves the tree,
//!    starts exiting, or moves to another path (the app hides what it drags, a tab torn
//!    off, a popover closing), or is laid out so that its centre at the start is no longer
//!    inside it, before P1's Escape. P1 sends none, so the app's keys hear nothing, but
//!    Rune's capture stays: it then ends at the compositor's focus change at the drop where
//!    there is one (sway), else at the window's next press or focus loss. Until then hover
//!    is frozen, and the user's first Escape ends the capture instead of reaching the app.
//!    **Keep the source mounted at the same path, not exiting, until the drag ends.**
//! 5. **A compositor that accepts `start_drag` after the button was released.** wlroots
//!    refuses one. Elsewhere, winit would have had the release, and an Escape sent before
//!    Rune ran the release's handler would reach the app's keys. Hand tests (mutter, KWin)
//!    pending.
//! 6. **A drag source the guard didn't see pressed** (an app's own `on_drag` that calls
//!    [`Dnd::start_drag`] without routing the press through [`Dnd::owns`]): no Escape, so
//!    its capture ends as in 4.
//!
//! X11 has none of these: winit keeps the grab, and Rune hears the real release.
//!
//! # The in-process token
//!
//! A drag lists `application/x-local-drag;id=<pid>.<counter>.<nonce>` (the owner's ruling)
//! so a drop in this process knows the drag as its own. On X11 it is listed only for a drag
//! with a [`DragItem::local`] payload. On Wayland it is listed on every drag: it is also how
//! P1 recognises its own drag entering its windows, the compositor's proof above. So other
//! Wayland clients see it, and with it this process's id.
//!
//! # For X2 (wiring components to P1)
//!
//! - Zones need keys: a W1 `DropZone` carries none, and [`Dnd::drop_target`] needs one
//!   unique in the window (U-P1-1). Key the element and pass the same key.
//! - C1b's pointer intent owns the row's `on_drag`, so a row can't take
//!   [`Dnd::drag_source`]. From C1b's begin-drag intent call [`Dnd::start_drag`] with the
//!   row's `Drag` and its key, and route every phase through [`Dnd::owns`] first, ignoring
//!   the ones it claims.
//! - A row the drag starts from must stay mounted at the same path, not exiting, until the
//!   drag ends (path 4 above): a virtualized row scrolled out, a row removed when its entry
//!   moves, or a list re-keyed during the drag leaves Rune's capture held. Hide a dragged
//!   row by styling it, not by dropping it from the tree, until `on_end`.
//! - W1's `choose_action` maps onto [`DropTarget::action`], which wins over P1's modifier
//!   rule.
//!
//! Work package: P1, `docs/work/phase-2-composites/P1-drag-and-drop.md`.

mod api;
mod hub;
mod icon;
mod local;
pub mod negotiate;
mod registry;
mod session;
mod wayland;
mod window;
mod x11;

pub use api::{
    Actions, AskReply, DRAG_THRESHOLD, DragAction, DragItem, DragSource, DropData, DropOffer,
    DropPoint, DropTarget, Error, Hover, Status, TEXT_TYPES,
};
pub use icon::{DragIcon, DragImage};
pub use local::LOCAL_MIME;
pub use window::Dnd;

/// Ends a running drag and tears down drag and drop with P0's substrate, on the UI thread
/// before the event loop ends. True if a substrate was bound.
pub fn shutdown() -> Result<bool, Error> {
    hub::shutdown()
}

/// Where problems that no call could return go (a drop that couldn't be finished, a read
/// that failed while peeking): stderr by default. On the UI thread.
pub fn on_diagnostic(f: impl Fn(&str) + 'static) {
    hub::set_diagnostics(std::rc::Rc::new(f));
}

#[cfg(test)]
mod tests;
