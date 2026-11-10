//! Tabbed form (C20c): the custom-action editor's notebook. A tablist over pages, each page
//! kept mounted or rebuilt by a flag, with a "has errors" mark per page.
//!
//! Work package: C20c, `docs/work/phase-1-leaves/C20c-tabbed-form-settings-search.md`.
//! Strata: `src/ui/settings/actions/editor.rs` (a `GtkNotebook` with General, Script and
//! Behavior) and `.settings-action-tabs` in `src/style.css`, at upstream `fb7eb0c4e`.
//!
//! # Use
//!
//! ```ignore
//! let form = TabbedForm::new("editor", |s: &App| &s.tabs, |s: &mut App| &mut s.tabs)
//!     .labels(FormLabels { tabs: "Action editor sections".into(), ..FormLabels::default() });
//! let pages = [
//!     FormPage::new("general", "General").errors(s.name_invalid),
//!     FormPage::new("script", "Script"),
//!     FormPage::new("behavior", "Behavior"),
//! ];
//! col()
//!     .child(form.build(s, look, &pages, |s, i| page_body(s, i)))
//!     .child(footer)                 // Cancel and Save
//!     .children(form.hop(s))         // in an element holding the form and the footer
//! ```
//!
//! - **Save with an invalid field:** the host validates, sets [`FormPage::has_errors`], and
//!   calls [`TabbedFormState::show_error`] with the page and the field's key: the page shows
//!   and the keyboard moves to the field (Strata's `invalid_field`). Every edit stays in the
//!   host's state, so nothing is lost and nothing is written.
//! - **Poll:** the host calls [`TabbedFormState::tick`] every frame; it runs the focus hop.
//! - **Keys:** the host's `on_key` calls [`TabbedForm::window_key`] for Ctrl+Page Up and
//!   Ctrl+Page Down.
//! - The page bodies are the host's, padding and scrolling included (Strata pads 22 px and
//!   scrolls each page). With [`Mounting::Kept`] each page keeps its scroll offset.
//!
//! # What assistive technology hears
//!
//! A tablist named by [`FormLabels::tabs`]; each tab by its title, selected when chosen, with
//! its place in the set. A page with errors: its tab is described "Has errors" and reports
//! itself invalid. The chosen panel is a `tabpanel` named by its title; hidden panels are out
//! of the tree, and nothing in them takes focus or the pointer.
//!
//! # Deviations
//!
//! - **D2 (new against Strata):** the error mark. Strata's notebook marks no tab; it switches
//!   to the invalid field's tab and shows one error line, as the host still does here.
//! - **D3 (OWNER: Strata's look on own elements):** the plan builds this from rune-kit
//!   `SegmentedControl`, which at Rune 5d421fd is a radiogroup of radios that take a text
//!   label only, so it can neither be a tablist nor carry a mark (Rune draft U-C20c-1). The
//!   tabs are our own elements with tab roles, in Strata's underline look (bullets 43, 44 of
//!   `integration/custom-actions`).
//! - **D5:** the keyboard moves to the invalid field by a one-frame focus hop ([`hop`]), as
//!   Rune 5d421fd has no `Tx::focus` (upstream #173). Past the pin Rune adds `Tx::focus` for
//!   web focus (#384) and `Tx::focused` in every handler (#441); not adopted.
//! - **D6:** a kept page is hidden by opacity 0, `inert` and `a11y_hidden` in a shared grid
//!   cell, so it still costs layout every frame: Rune has no subtree that stays mounted out of
//!   layout (Rune draft U-C20c-2).
//! - The panels' changes land at once (`instant_changes`): Strata's notebook switches pages
//!   with no transition.

pub(crate) mod hop;
mod state;
mod view;

#[cfg(test)]
mod tests;

pub use state::{FormPage, Mounting, TabbedFormState};
pub use view::{
    FormLabels, LABEL_REST_OPACITY, LABEL_WEIGHT, MARK, RING_ALPHA, RULE_ALPHA, STRIP_PAD_X,
    TAB_GAP, TAB_HEIGHT, TAB_PAD_X, TabbedForm, UNDERLINE,
};
