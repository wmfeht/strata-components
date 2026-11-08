//! What the marquee machine reads on every step, shared live with the host through a
//! [`MarqueeHandle`].

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use rune::Spring;

use super::speed::EdgeScroll;

/// The marquee machine's settings: plain data the host keeps current through its
/// [`MarqueeHandle`].
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

/// The settings one marquee machine reads, shared with the host that keeps them current.
///
/// Clones share one value: the host keeps a clone and changes it with [`MarqueeHandle::set`] or
/// [`MarqueeHandle::update`], and the machine's next step reads the change. A handle rather
/// than an accessor, as C1b's `PointerHandle` (D6 in the module docs).
///
/// Used by: C2, C3, C4, and hosts.
#[derive(Clone, Debug, Default)]
pub struct MarqueeHandle(Rc<Cell<MarqueeSettings>>);

impl MarqueeHandle {
    /// A handle holding `settings`.
    pub fn new(settings: MarqueeSettings) -> Self {
        Self(Rc::new(Cell::new(settings)))
    }

    /// The settings now.
    pub fn get(&self) -> MarqueeSettings {
        self.0.get()
    }

    /// Replaces the settings.
    pub fn set(&self, settings: MarqueeSettings) {
        self.0.set(settings);
    }

    /// Changes the settings in place: `handle.update(|s| s.animations = look.animations)`.
    pub fn update(&self, f: impl FnOnce(&mut MarqueeSettings)) {
        let mut settings = self.get();
        f(&mut settings);
        self.set(settings);
    }
}
