//! Moving from one look to the next with the colours animating.

use rune::{Spring, Tx};

use crate::look::Look;

/// The spring a theme switch animates its colours on: Rune's calm theme-change motion.
/// Rune's reduced motion still applies: colours then take its short fade.
pub const MOTION: Spring = rune::tok::motion::CALM;

/// Replaces `look` with `next` in this change, animating every colour that differs on
/// [`MOTION`]. Rune animates colours in OKLab, text leading its background, so labels keep
/// their contrast through the switch.
pub fn switch(look: &mut Look, next: Look, tx: &mut Tx) {
    tx.color_motion(MOTION);
    *look = next;
}
