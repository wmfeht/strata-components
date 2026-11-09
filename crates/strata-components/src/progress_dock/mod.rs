//! ProgressDock (C16): running operations as cards in the bottom-right dock, brief notices
//! beside them, the blocking progress dialog, and the Jobs dashboard's footer element
//! (`operations/progress`, `.../dock`, `.../jobs`). The component owns no operation:
//! operations arrive as data, and Cancel is an intent.
//!
//! Work package: C16, `docs/work/phase-2-composites/C16-progress-dock.md`.
//! Behaviour follows lgse/strata `upstream/main`: `src/ui/progress_dock.rs`,
//! `src/ui/progress_dock/completion.rs`, `src/ui/notice.rs`, `src/ui/browser/progress/*`,
//! `src/ui/jobs.rs`, `src/i18n.rs` and `src/style.css`.
//!
//! The full notes (hosting, rules, what assistive technology hears, coverage, Rune drafts)
//! follow below.

mod countdown;
mod dialog;
pub mod format;
pub mod jobs;
mod model;
mod rate;
mod view;
mod words;

#[cfg(test)]
mod tests;

pub use countdown::{COMPLETE_FOR, Countdown, FADE};
pub use dialog::{EscapeDoes, ProgressDialog, RATE_WIDTH};
pub use model::{
    COMMIT_EVERY, CardLive, Dock, DockIntent, DockNotice, EndKind, EndShown, IMMEDIATE_ITEMS,
    NOTICE_FOR, Notice, Op, OpId, OpSpec, Placement, Progress, REVEAL_DELAY, STALL_AFTER, Sample,
    Shown, Transfer, reveal_after,
};
pub use rate::{RateMeter, SAMPLE_EVERY, SMOOTHING};
pub use view::{
    BAR_RADIUS, CARD_BAR, CARD_MAX_WIDTH, CARD_MIN_WIDTH, CARD_RADIUS, DIALOG_BAR, DOCK_BOTTOM,
    DOCK_MAX_HEIGHT, DockView,
};
pub use words::{Phrase, Phrase2, Words};
