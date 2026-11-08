//! What the marquee machine reads on every step: the host gives it to the collection
//! (`Collection::settings`), and the machine reads it through its surface
//! (`Ctx::settings::<MarqueeSettings>()`) on each input.

use std::time::Duration;

use rune::Spring;

use super::speed::EdgeScroll;

/// The marquee machine's settings: plain data the host keeps current on the collection
/// (`Collection::settings(MarqueeSettings { .. })`). Without one, the defaults.
///
/// Fields may be added: start from [`MarqueeSettings::default`].
///
/// Used by: C2, C3, C4, and hosts.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MarqueeSettings {
    /// Where edge auto-scroll starts and how fast it gets.
    pub edge: EdgeScroll,
    /// The spring auto-scroll glides on. Rune's `GLIDE` by default, the spring of the
    /// collection element's `ScrollTo { glide: true }`.
    pub glide: Spring,
    /// The frame interval assumed for the first glide step; later steps use the interval
    /// measured between ticks. 1/60 s by default.
    pub frame: Duration,
    /// While the view scrolls, the band is applied to the selection at most this often, so
    /// the commits stay under Rune's scroll-commit warning (D1 in the module docs). 100 ms by
    /// default: 6 frames at 60 Hz, where the warning needs one commit in 4.
    pub apply_every: Duration,
    /// False when the user or the desktop turned animations off (`Look::animations`). Rune
    /// then lands each glide at once, so the machine aims one frame's travel ahead instead of
    /// a glide's lead.
    pub animations: bool,
}

impl Default for MarqueeSettings {
    fn default() -> Self {
        Self {
            edge: EdgeScroll::STRATA,
            glide: rune::tok::motion::GLIDE,
            frame: Duration::from_micros(16_667),
            apply_every: Duration::from_millis(100),
            animations: true,
        }
    }
}
