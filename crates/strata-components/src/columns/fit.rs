//! What the strip tells C9's drawer about itself in Columns ([`ColumnsFit`]).

use strata_kit::look::Look;

use super::state::ColumnsState;
use super::width;
use crate::preview_drawer::{ColumnsFit, DrawerRules};

/// The strip's fit for the drawer's rules: the columns from the root through the active one,
/// the width that keeps the active one whole (at most one standard column for the
/// narrow-window test), and the columns after it, which borrow the empty slot's width; one
/// standard column at the text size.
pub(super) fn columns_fit(st: &ColumnsState, look: &Look, rules: &DrawerRules) -> ColumnsFit {
    let spans = st.spans(look);
    let standard = width::drawn(width::STANDARD, width::scale(look));
    let Some(&(left, right)) = spans.get(st.active.min(spans.len().saturating_sub(1))) else {
        return ColumnsFit::new(rules, 0.0, 0.0).standard_column(standard);
    };
    let total = spans.last().map_or(0.0, |s| s.1);
    let active = right - left;
    ColumnsFit::new(rules, right, active)
        .keep_focused_standard(active.min(standard))
        .trailing(total - right)
        .standard_column(standard)
}
