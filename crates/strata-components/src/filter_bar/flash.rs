//! [`Flash`]: a short footer message, such as "Unknown chord" or "No matches for “x”", that
//! goes 2 s after it shows (Strata's `show_feedback`, `FEEDBACK_FLASH`).

use std::time::Duration;

use rune::{El, LiveRegion, Tx, row, text};
use strata_kit::look::Look;

/// How long a flash shows (Strata's 2 s).
pub const FLASH_FOR: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Since {
    /// Shown; the next tick stamps the time (a key handler has no clock: Rune draft U-C1e-6).
    Pending,
    At(f64),
}

/// The footer's flash, held by the host. A new flash replaces the one showing and starts its
/// 2 s again.
///
/// The host calls [`Flash::tick`] from its poll each frame. It asks for a wake when the flash
/// is due to go, so the flash goes in a window at rest too, which polls only at the wakes it
/// asked for.
///
/// Used by: hosts, C19 (which places it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flash {
    shown: Option<(String, Since)>,
}

impl Flash {
    /// Nothing showing.
    pub fn new() -> Self {
        Self::default()
    }

    /// The words showing.
    pub fn text(&self) -> Option<&str> {
        self.shown.as_ref().map(|(t, _)| t.as_str())
    }

    /// Shows `words` for [`FLASH_FOR`] from the next tick, and asks for a wake on `tx` so that
    /// tick comes in a window at rest too: a key handler has no clock, and without a wake no
    /// poll would follow it to stamp the time. Empty words clear it.
    pub fn show(&mut self, words: impl Into<String>, tx: &mut Tx) {
        let words = words.into();
        self.shown = (!words.is_empty()).then_some((words, Since::Pending));
        if self.shown.is_some() {
            tx.wake_after(Duration::ZERO);
        }
    }

    /// Clears it at once.
    pub fn clear(&mut self) {
        self.shown = None;
    }

    /// One frame at `now` seconds on the host's clock: stamps a new flash and asks for a wake
    /// when it is due, asks again for the time left, and ends it once due. True when it ended.
    pub fn tick(&mut self, now: f64, tx: &mut Tx) -> bool {
        match self.left(now) {
            Some(left) if left <= 1e-6 => {
                self.shown = None;
                true
            }
            Some(left) => {
                tx.wake_after(Duration::from_secs_f64(left));
                false
            }
            None => false,
        }
    }

    /// The seconds left at `now`, stamping a new flash: `None` with nothing showing.
    fn left(&mut self, now: f64) -> Option<f64> {
        let (_, since) = self.shown.as_mut()?;
        let at = match *since {
            Since::Pending => {
                *since = Since::At(now);
                now
            }
            Since::At(t) => t,
        };
        Some(FLASH_FOR - (now - at))
    }
}

/// The flash's element for the footer, or `None` with nothing showing: the text colour, as
/// Strata's `.shortcut-footer-feedback`, and a polite live region.
pub fn flash(look: &Look, key: &str, state: &Flash) -> Option<El> {
    let words = state.text()?;
    Some(
        row()
            .key(key.to_owned())
            .role("status")
            .label(words.to_owned())
            .live_region(LiveRegion::Polite)
            .center_y()
            .child(
                text(words.to_owned())
                    .font(look.kit.ty.small)
                    .color(look.kit.c.text_base)
                    .truncate()
                    .a11y_hidden(),
            ),
    )
}
