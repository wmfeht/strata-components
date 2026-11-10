//! Slider (C20a).
//!
//! Work package: C20a, `docs/work/phase-1-leaves/C20a-slider-number-field.md`.

mod range;
mod state;
mod view;

pub use range::Range;
pub use state::{SliderChange, SliderState};
pub use view::{Slider, Snap};

#[cfg(test)]
mod tests;
