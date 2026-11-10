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
    /// The hop's leaves mount in the form (a page switch's) rather than in the host's element
    /// ([`TabbedFormState::show_error`]'s, [`super::TabbedForm::hop`]).
    pub(super) hop_in_form: bool,
    /// The page [`TabbedFormState::show_error`] shows when its hop drops its leaf.
    pending: Option<usize>,
    /// The page shown, with its visit, while a page switch's hop carries the keyboard out of
    /// it: it stays shown until the leaves drop, so the keyboard is still somewhere then
    /// (`hop.rs`). The tabs show the new choice at once.
    held: Option<(usize, u64)>,
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

    /// The page the view shows and its visit: the chosen one, unless a switch holds the old.
    pub(super) fn shown(&self) -> (usize, u64) {
        self.held.unwrap_or((self.selected, self.visits))
    }

    /// Before a switch from the keyboard (Ctrl+Page Up or Down): move the keyboard to the
    /// first of `targets` with a focusable element, holding the shown page until it moves
    /// when `hold` (the keyboard may be in it). Replaces a pending
    /// [`TabbedFormState::show_error`].
    pub(super) fn carry(&mut self, targets: Vec<String>, hold: bool, tx: &mut Tx) {
        if hold {
            self.held = Some(self.shown());
        }
        self.pending = None;
        self.hop_in_form = true;
        self.hop.request_first(targets, tx);
    }

    /// After a failed save (Strata's `invalid_field`): choose page `i` and move the keyboard to
    /// the element keyed `field` on it, two frames on, once [`super::TabbedForm::hop`]'s leaf
    /// has been drawn and dropped. The page changes in the commit that moves the keyboard, so
    /// a keyboard inside the page being hidden (Return in one of its fields pressed Save) still
    /// moves. The host also sets the page's [`FormPage::has_errors`] and keeps every edit: the
    /// form holds none of them.
    ///
    /// `field` must key a focusable element on page `i`: if it keys nothing shown there (a
    /// host bug), the page still changes and the keyboard, if it was on the page being hidden,
    /// is dropped.
    pub fn show_error(&mut self, i: usize, field: impl Into<String>, tx: &mut Tx) {
        self.pending = Some(i);
        self.hop_in_form = false;
        self.hop.request(field.into(), tx);
    }

    /// From the host's poll, every frame: runs a pending focus hop. True when the view changes.
    pub fn tick(&mut self, tx: &mut Tx) -> bool {
        if self.hop.dropping() {
            self.held = None;
            if let Some(i) = self.pending.take() {
                self.select(i);
            }
        }
        self.hop.tick(tx)
    }
}
