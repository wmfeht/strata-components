//! The two backend-neutral pieces of a drag: the press guard on a drag source, and which
//! target a drag over a window lands on.
//!
//! # The press guard, and Rune's capture (U10)
//!
//! A drag source is an `on_drag` element: Rune's `drag_out` starts nothing on Linux (draft
//! U1, not filed). Once the system drag starts, the compositor or X server has the pointer,
//! and Rune's capture of the press is left behind:
//!
//! - **Wayland:** winit gets a pointer leave and never the release, so Rune stays pressing
//!   and later moves reach `on_drag` with no button held (S2, Q3). Rune has no public way to
//!   end the capture (draft U10, not filed). The workaround: right after `start_drag`, P1
//!   sends Escape through `Tx::send_key`, which Rune handles as "Escape during a drag cancels
//!   it" (`Drag::cancelled`'s documented cause): the element hears `Up` with `cancelled`,
//!   and the capture ends. The guard swallows that `Up`. It is sent only while the guard
//!   holds the press it saw go down; if something ended the capture in the same instant,
//!   the Escape would reach the app's key handlers once.
//! - **X11:** the source is grab-less and winit keeps its implicit grab, so Rune hears every
//!   move and the real release. The guard swallows them. A cancelled `Up` (the user's Escape,
//!   or focus lost) cancels the drag.

use std::any::Any;
use std::rc::Rc;

use rune::{Drag, Mods, Phase};

use super::api::{Actions, DragAction, DropOffer, Hover};
use super::negotiate;
use super::registry::Registry;

/// How the platform ends Rune's capture of a press that became a system drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Release {
    /// Wayland: P1 sends Escape; a cancelled `Up` follows, which is ours.
    Escape,
    /// X11: Rune hears the real release; a cancelled `Up` before it cancels the drag.
    RealUp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Idle,
    Pressed { x: f32, y: f32 },
    Started(Release),
}

/// What a source's `on_drag` does with a phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// Not the system drag's: the element's own handler hears it.
    Pass,
    /// The pointer crossed the threshold: start the drag.
    Start,
    /// The system drag's: swallowed.
    Own,
    /// X11: the press was cancelled under a running drag (Escape, focus): cancel it.
    Cancel,
}

#[derive(Debug)]
pub(crate) struct Guard {
    state: State,
}

impl Default for Guard {
    fn default() -> Self {
        Guard { state: State::Idle }
    }
}

/// One phase of a press, as the guard reads it from Rune's `Drag`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Step {
    pub(crate) phase: Phase,
    /// The pointer, in window points.
    pub(crate) at: [f32; 2],
    pub(crate) cancelled: bool,
}

impl Step {
    pub(crate) fn of(d: &Drag) -> Step {
        Step {
            phase: d.phase,
            at: d.window_point(),
            cancelled: d.cancelled(),
        }
    }
}

impl Guard {
    /// A source element's phase, with its threshold in points.
    pub(crate) fn phase(&mut self, d: Step, threshold: f32) -> Decision {
        if let (Phase::Move, State::Pressed { x, y }) = (d.phase, self.state) {
            let [px, py] = d.at;
            return if (px - x).hypot(py - y) > threshold {
                Decision::Start
            } else {
                Decision::Pass
            };
        }
        self.owns(d)
    }

    /// The drag started from the press the guard holds (or, for an app's own `on_drag`
    /// calling `start_drag`, from whatever press is down).
    pub(crate) fn started(&mut self, release: Release) {
        self.state = State::Started(release);
    }

    /// For [`super::Dnd::owns`]: whether `d` belongs to the system drag.
    pub(crate) fn owns(&mut self, d: Step) -> Decision {
        match (d.phase, self.state) {
            (Phase::Down, _) => {
                let [x, y] = d.at;
                self.state = State::Pressed { x, y };
                Decision::Pass
            }
            (Phase::Move, State::Started(_)) => Decision::Own,
            (Phase::Move, _) => Decision::Pass,
            (Phase::Up, State::Started(release)) => {
                self.state = State::Idle;
                if release == Release::RealUp && d.cancelled {
                    Decision::Cancel
                } else {
                    Decision::Own
                }
            }
            (Phase::Up, _) => {
                self.state = State::Idle;
                Decision::Pass
            }
        }
    }
}

/// Where a drag over a window lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Landing {
    /// No target takes it here.
    Nowhere,
    /// `key` takes it with this action, reading `accept` (the in-process token for a local
    /// payload).
    Accepting {
        key: String,
        action: DragAction,
        accept: String,
    },
    /// `key` refuses it and stops it ([`DropTarget::on_reject`](super::DropTarget::on_reject)).
    Rejecting { key: String },
}

impl Landing {
    pub(crate) fn key(&self) -> Option<&str> {
        match self {
            Landing::Nowhere => None,
            Landing::Accepting { key, .. } | Landing::Rejecting { key } => Some(key),
        }
    }

    pub(crate) fn hover(&self) -> Hover {
        match self {
            Landing::Nowhere => Hover::Left,
            Landing::Accepting { action, .. } => Hover::Accepting(*action),
            Landing::Rejecting { .. } => Hover::Rejecting,
        }
    }
}

/// The actions to negotiate with: the source's, or copy while they aren't known yet
/// (Wayland says just after the drag enters).
pub(crate) fn allowed(offer: &DropOffer) -> Actions {
    if offer.actions.is_empty() {
        Actions::COPY
    } else {
        offer.actions
    }
}

/// The target of the drag at a hit test's `path`: the innermost that takes it.
pub(crate) fn land<S: 'static>(
    registry: &Registry<S>,
    path: Option<&str>,
    s: &S,
    offer: &DropOffer,
    local: Option<(&str, &Rc<dyn Any>)>,
    mods: Option<Mods>,
) -> Landing {
    let Some(path) = path else {
        return Landing::Nowhere;
    };
    let payload = local.map(|(_, p)| p);
    for key in registry.along(path) {
        let Some(entry) = registry.get(key) else {
            continue;
        };
        let t = &entry.target;
        let stops = t.on_reject.is_some();
        if !t.takes(offer, payload) {
            if stops {
                return Landing::Rejecting {
                    key: key.to_owned(),
                };
            }
            continue;
        }
        let pick = t.action.as_ref().map(|f| f(s, offer, mods));
        let action = negotiate::drop_action(pick, allowed(offer), mods);
        let accept = match local {
            Some((token, p)) if t.takes_local(Some(p)) => Some(token.to_owned()),
            _ => t.mimes.iter().find(|m| offer.offers(m)).cloned(),
        };
        match (action, accept) {
            (Some(action), Some(accept)) => {
                return Landing::Accepting {
                    key: key.to_owned(),
                    action,
                    accept,
                };
            }
            _ if stops => {
                return Landing::Rejecting {
                    key: key.to_owned(),
                };
            }
            _ => continue,
        }
    }
    Landing::Nowhere
}

#[cfg(test)]
mod tests;
