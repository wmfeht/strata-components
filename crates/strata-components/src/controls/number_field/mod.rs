//! NumberField (C20a).
//!
//! Work package: C20a, `docs/work/phase-1-leaves/C20a-slider-number-field.md`.

mod state;
mod view;

pub use crate::controls::slider::Range;
pub use state::NumberFieldState;
pub use view::NumberField;

#[cfg(test)]
mod tests;
