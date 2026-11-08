//! The archive tree's key map and pointer controller, on C1e's seam.
//!
//! One member is highlighted at a time, so the map has no Shift ranges, no Ctrl+A or Ctrl+Space
//! and no Escape (that's the host's: Columns closes the preview). Left and Right have no
//! neighbour in a list, so C1c reports them as edges, and the view opens a folder or goes up.

use rune::PointerButton;

use crate::collection::element::{
    Activation, Ctx, Effect, Flow, Intent, PointerController, PointerInput, PointerPhase,
};
use crate::collection::keyboard::{Command, KeyMap, KeySettings, Letters, Target, Vertical};
use crate::contract::Direction;

/// How far (points) a release may land from its press and still click: Rune's click slop.
const CLICK_SLOP: f32 = 4.0;

/// C1c's default map less everything that selects more than one member or clears the
/// highlight: the arrows, Home, End, the page keys, Ctrl+Up and Ctrl+Down, and Enter. `h j k l`
/// stay aliases, which act with [`Letters::Arrows`] (Type to search off).
pub fn key_map() -> KeyMap {
    let mut map = KeyMap::default_map();
    let targets = [
        Target::Step(Direction::Up),
        Target::Step(Direction::Down),
        Target::Step(Direction::Left),
        Target::Step(Direction::Right),
        Target::First,
        Target::Last,
        Target::Page(Vertical::Up),
        Target::Page(Vertical::Down),
    ];
    for t in targets {
        map.unbind(Command::Extend(t));
    }
    map.unbind(Command::Nudge(Direction::Left))
        .unbind(Command::Nudge(Direction::Right))
        .unbind(Command::Toggle)
        .unbind(Command::SelectAll)
        .unbind(Command::Clear);
    map
}

/// The keyboard settings for the archive tree: its map, with `h j k l` moving when `letters`.
pub fn key_settings(letters: bool) -> KeySettings {
    KeySettings {
        map: key_map(),
        ..KeySettings::default()
    }
    .letters(if letters {
        Letters::Arrows
    } else {
        Letters::Pass
    })
}

/// Members highlight on the release of a click, never on a press or a hover, as Strata's
/// archive list does (lgse/strata#1570): a click on a member highlights it alone, whatever
/// modifiers are held, and a click on a folder also opens it ([`Intent::Activate`] with
/// [`Activation::Click`]). Only a run's first click counts: opening a folder replaces every
/// row, so a double click's second press lands on a row of the folder just opened, and must not
/// open it too. A drag selects nothing, and starts no marquee and no drag out.
#[derive(Debug, Default)]
pub struct ArchivePointer {
    pressed: Option<usize>,
}

impl PointerController for ArchivePointer {
    fn pointer(&mut self, input: &PointerInput, cx: &mut Ctx<'_>) -> Flow {
        if input.button != PointerButton::Primary {
            return Flow::Pass;
        }
        match input.phase {
            PointerPhase::Down => {
                self.pressed = input.hit.index;
            }
            PointerPhase::Move => {}
            PointerPhase::Up => {
                let pressed = self.pressed.take();
                let still =
                    input.travel.x.abs() <= CLICK_SLOP && input.travel.y.abs() <= CLICK_SLOP;
                if let Some(i) = input.hit.index
                    && pressed == Some(i)
                    && still
                    && !input.cancelled
                    && input.clicks == 1
                    && i < cx.source.len()
                {
                    cx.apply(|sel, src| sel.select_only(src, i));
                    let entry = cx.source.entry(i);
                    if entry.kind.is_folder_like() {
                        cx.emit(Effect::Intent(Intent::Activate {
                            id: entry.id,
                            how: Activation::Click,
                        }));
                    }
                }
            }
        }
        Flow::Handled
    }

    fn reset(&mut self) {
        self.pressed = None;
    }
}
