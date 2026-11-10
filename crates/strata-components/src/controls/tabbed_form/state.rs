//! The form's state: the chosen page and a pending focus hop.

use rune::Tx;

use super::hop::Hop;

/// A page of the form, as data: its key, its tab's title and whether it holds a field the
/// host found invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormPage {
    /// Unique among the form's pages; keys the tab and the panel.
    pub key: String,
    pub title: String,
    /// Draws the error mark on the tab and describes the tab as having errors.
    pub has_errors: bool,
}

impl FormPage {
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            has_errors: false,
        }
    }

    /// Mark the page as holding an invalid field, or clear the mark.
    pub fn errors(mut self, on: bool) -> Self {
        self.has_errors = on;
        self
    }
}

/// Whether a page the user isn't looking at stays built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mounting {
    /// Every page is built every frame; the hidden ones are invisible, inert and out of the
    /// accessibility tree. Each keeps what Rune holds per element across a switch: its scroll
    /// offset, the focus memory of its groups, measured virtual-row heights. The form is as
    /// tall as its tallest page, as GtkNotebook is.
    #[default]
    Kept,
    /// Only the chosen page is built; a switch starts the page afresh (scrolled to its top).
    /// The form is as tall as the chosen page.
    Rebuilt,
}

/// The form's state, held by the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabbedFormState {
    selected: usize,
    /// Counts page changes: a rebuilt page's body is keyed by it, so each visit mounts afresh.
    pub(super) visits: u64,
    pub(super) hop: Hop,
}

impl TabbedFormState {
    /// The first page chosen.
    pub fn new() -> Self {
        Self::default()
    }

    /// The chosen page's index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Choose page `i`. The view clamps an index past the last page.
    pub fn select(&mut self, i: usize) {
        if self.selected != i {
            self.selected = i;
            self.visits += 1;
        }
    }

    /// After a failed save (Strata's `invalid_field`): choose page `i` and move the keyboard to
    /// the element keyed `field` on it, once [`super::TabbedForm::hop`]'s leaf has been drawn
    /// and dropped. The host also sets the page's [`FormPage::has_errors`] and keeps every edit:
    /// the form holds none of them.
    pub fn show_error(&mut self, i: usize, field: impl Into<String>, tx: &mut Tx) {
        self.select(i);
        self.hop.request(field.into(), tx);
    }

    /// From the host's poll, every frame: runs a pending focus hop. True when the view changes.
    pub fn tick(&mut self, tx: &mut Tx) -> bool {
        self.hop.tick(tx)
    }
}
