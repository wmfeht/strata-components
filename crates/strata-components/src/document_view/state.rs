//! [`DocumentState`]: what the host keeps per document preview, and what the drawer's header
//! and find bar ask of it.

use std::cell::{Cell, OnceCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use rune::{Pixels, TextSelection, Tx};

use super::copy;
use super::find::{Index, Match};
use super::model::{Document, Unit, UnitKind};
use super::shown::{Insert, Shown, math_inserts};
use crate::preview_drawer::{DocumentView, FindStatus, HostedPreview};
use crate::scroll::Bar;

/// How long Copy code shows its check and "Code copied" (Strata's 2 s).
pub const COPIED_FOR: Duration = Duration::from_secs(2);

/// Figure spaces kept at the end of a code block's first line, under the Copy code button, so
/// the button never covers the line's end (`El::uncopied`: they draw, but don't copy). A
/// figure space doesn't break and is a digit wide, about one cell of a mono face and as wide
/// in a proportional one, so five keep room for the button at any text size.
pub(crate) const BUTTON_ROOM: &str = "\u{2007}\u{2007}\u{2007}\u{2007}\u{2007}";

/// An image, diagram or equation's pixels, as the host has them.
#[derive(Clone, Debug, Default)]
pub enum MediaState {
    /// Still decoding or rendering: a placeholder holds its place.
    #[default]
    Loading,
    Ready(Pixels),
    /// Missing or undecodable: its alt text shows instead.
    Failed,
}

/// A selection or scroll the view asks for, applied by [`DocumentState::flush`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Request {
    Select(String, Range<usize>),
    Clear,
    Reveal(String),
}

/// What doesn't change between [`DocumentState::set_document`]s: the document and its shown
/// texts. The view's copy handler holds it, since Rune gives that handler no app state.
#[derive(Debug)]
pub(crate) struct Content {
    pub key: String,
    pub doc: Arc<Document>,
    /// What each unit's text element shows, for the units where that isn't the model's text.
    shown: Vec<Option<Shown>>,
    /// Ctrl+A selected the whole document, and nothing has changed the selection since.
    pub all: Cell<bool>,
}

impl Content {
    fn new(key: String, doc: Arc<Document>) -> Content {
        Content {
            shown: doc.units.iter().map(shown_for).collect(),
            key,
            doc,
            all: Cell::new(false),
        }
    }

    /// The shown text of unit `i`, and its spans.
    pub fn shown(&self, i: usize) -> (&str, &[super::model::Span]) {
        match &self.shown[i] {
            Some(s) => (&s.text, &s.spans),
            None => (&self.doc.units[i].text, &self.doc.units[i].spans),
        }
    }

    pub fn shown_of(&self, i: usize) -> Option<&Shown> {
        self.shown.get(i).and_then(Option::as_ref)
    }

    /// Every unit's copy text: the whole document as Ctrl+A then Ctrl+C copies it.
    pub fn copy_all(&self) -> String {
        self.doc
            .units
            .iter()
            .map(|u| u.copy_text.as_str())
            .collect()
    }

    /// What copying `sel` takes (Strata's rule): units wholly inside it copy their copy text,
    /// and its end units the part selected, with the block's newline where it goes on.
    pub fn copy(&self, sel: &TextSelection) -> String {
        if self.all.get() {
            return self.copy_all();
        }
        copy::copy(self, sel)
    }
}

/// One document preview's state, kept by the host (one per preview) and given to
/// [`DocumentView`](super::DocumentView) by accessor.
#[derive(Debug)]
pub struct DocumentState {
    content: Rc<Content>,
    searchable: bool,
    revision: u64,
    source_available: bool,
    wrap: Option<bool>,
    media: HashMap<usize, MediaState>,
    query: String,
    matches: Vec<Match>,
    current: Option<usize>,
    pending: Vec<Request>,
    pub(crate) visible: Range<usize>,
    /// The code block (or diagram, equation) whose Copy button was pressed, and when.
    pub(crate) copied: Option<(usize, Option<f64>)>,
    pub(crate) bar: Bar,
    colourable: Vec<bool>,
    /// Ctrl+A asked Rune to select everything, and its selection change hasn't come yet.
    all_pending: bool,
    /// The document's searchable text, joined and folded on the first find.
    index: OnceCell<Index>,
}

impl Default for DocumentState {
    fn default() -> Self {
        DocumentState::new(Arc::default())
    }
}

impl DocumentState {
    pub fn new(doc: Arc<Document>) -> Self {
        let mut s = DocumentState {
            content: Rc::new(Content::new("doc".into(), Arc::default())),
            searchable: false,
            revision: 0,
            source_available: false,
            wrap: Some(true),
            media: HashMap::new(),
            query: String::new(),
            matches: Vec::new(),
            current: None,
            pending: Vec::new(),
            visible: 0..0,
            copied: None,
            bar: Bar::new(),
            colourable: Vec::new(),
            all_pending: false,
            index: OnceCell::new(),
        };
        s.set_document(doc);
        s
    }

    /// The view's key, `doc` unless set: its rows are keyed `{key}-u{index}` and each row's
    /// text `t`. Give each document view on screen its own.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.content = Rc::new(Content::new(key.into(), self.content.doc.clone()));
        self
    }

    pub fn view_key(&self) -> &str {
        &self.content.key
    }

    pub(crate) fn content(&self) -> Rc<Content> {
        self.content.clone()
    }

    /// Replaces the document (a reload). Media states are dropped; an open find runs again
    /// on the new text without moving the selection.
    pub fn set_document(&mut self, doc: Arc<Document>) {
        self.searchable = doc
            .units
            .iter()
            .any(|u| searchable(&u.kind) && !u.text.is_empty());
        self.colourable = doc
            .units
            .iter()
            .map(|u| {
                u.first
                    && u.last
                    && matches!(
                        &u.kind,
                        UnitKind::Code {
                            language: Some(_),
                            ..
                        }
                    )
            })
            .collect();
        self.content = Rc::new(Content::new(self.content.key.clone(), doc));
        self.index = OnceCell::new();
        self.revision += 1;
        self.media.clear();
        self.copied = None;
        self.matches = self.run_find();
        self.current = self
            .current
            .filter(|&c| c < self.matches.len())
            .or((!self.matches.is_empty()).then_some(0));
    }

    pub fn document(&self) -> &Arc<Document> {
        &self.content.doc
    }

    /// Bumped by every [`set_document`](Self::set_document).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether the file has a Source view (Markdown, HTML, RTF, CSV, TSV): the header then
    /// offers View source. DOCX, ODT and workbooks have none. Off by default.
    pub fn set_source_available(&mut self, on: bool) {
        self.source_available = on;
    }

    /// The wrap preference, or `None` for a document that always wraps (DOCX, ODT): the
    /// header then hides the toggle. With wrap off, each row scrolls sideways on its own (D7).
    pub fn set_wrap(&mut self, wrap: Option<bool>) {
        self.wrap = wrap;
    }

    pub(crate) fn wraps(&self) -> bool {
        self.wrap.unwrap_or(true)
    }

    /// The pixels for media unit `unit`, as they arrive (or fail).
    pub fn set_media(&mut self, unit: usize, media: MediaState) {
        self.media.insert(unit, media);
    }

    pub(crate) fn media(&self, unit: usize) -> &MediaState {
        const LOADING: MediaState = MediaState::Loading;
        self.media.get(&unit).unwrap_or(&LOADING)
    }

    /// The shown text of unit `i`.
    pub(crate) fn shown(&self, i: usize) -> (&str, &[super::model::Span]) {
        self.content.shown(i)
    }

    pub(crate) fn shown_of(&self, i: usize) -> Option<&Shown> {
        self.content.shown_of(i)
    }

    /// Rune's visible rows: true when a code block that colours came within reach of the
    /// viewport or left it, so the view builds again and asks for its colours.
    pub(crate) fn set_visible(&mut self, rows: Range<usize>) -> bool {
        let old = std::mem::replace(&mut self.visible, rows.clone());
        let reach = |r: &Range<usize>| (r.len() * 2).max(8);
        let from = old
            .start
            .min(rows.start)
            .saturating_sub(reach(&old).max(reach(&rows)));
        let to = (old.end.max(rows.end) + reach(&old).max(reach(&rows))).min(self.colourable.len());
        (from..to).any(|i| {
            self.colourable[i] && super::render::near(&old, i) != super::render::near(&rows, i)
        })
    }

    /// Ctrl+A: the whole document is selected, and copies from the model, from the selection
    /// change Rune's select-all makes until the next one. (Pressed again with everything
    /// selected, Rune's selection doesn't change, and the copy-everything holds.)
    pub(crate) fn select_all(&mut self) {
        if !self.content.all.get() {
            self.all_pending = true;
        }
    }

    pub(crate) fn all_selected(&self) -> bool {
        self.content.all.get()
    }

    /// Lets a test copy a Ctrl+A selection through Rune's own collection.
    #[cfg(test)]
    pub(crate) fn select_all_off_for_test(&mut self) {
        self.content.all.set(false);
    }

    /// The selection changed: the change Ctrl+A's select-all makes starts copy-everything;
    /// any other ends it. (Rune reports a selection over unbuilt rows without all of them,
    /// so the change itself can't say it is the whole document.)
    pub(crate) fn selection_changed(&mut self, sel: Option<&TextSelection>) {
        let all = std::mem::take(&mut self.all_pending) && sel.is_some();
        self.content.all.set(all);
    }

    /// Runs the host's clock: Copy code goes back to its icon [`COPIED_FOR`] after a press.
    /// Call it from the host's poll; true when the view changed.
    pub fn tick(&mut self, now: f64) -> bool {
        match &mut self.copied {
            Some((_, at @ None)) => {
                *at = Some(now);
                false
            }
            Some((_, Some(at))) if now - *at >= COPIED_FOR.as_secs_f64() => {
                self.copied = None;
                true
            }
            _ => false,
        }
    }

    /// Applies the selections and scrolls find has asked for. [`HostedPreview::find`] gets no
    /// `Tx`, so they wait here: call this from the host's poll (which has one). True when it
    /// asked for anything.
    pub fn flush(&mut self, tx: &mut Tx) -> bool {
        let any = !self.pending.is_empty();
        for r in self.pending.drain(..) {
            match r {
                Request::Select(key, range) => tx.select_text(key, range),
                Request::Clear => tx.clear_text_selection(),
                Request::Reveal(key) => tx.scroll_into_view(key),
            }
        }
        any
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> &[Request] {
        &self.pending
    }

    pub(crate) fn row_key(&self, unit: usize) -> String {
        format!("{}-u{unit}", self.content.key)
    }

    pub(crate) fn list_key(&self) -> String {
        format!("{}-rows", self.content.key)
    }

    pub(crate) fn scroll_key(&self) -> String {
        format!("{}-scroll", self.content.key)
    }

    /// The matches of the open find.
    pub fn matches(&self) -> &[Match] {
        &self.matches
    }

    /// The selected match.
    pub fn current_match(&self) -> Option<Match> {
        self.current.map(|c| self.matches[c])
    }

    fn run_find(&self) -> Vec<Match> {
        if self.query.is_empty() {
            return Vec::new();
        }
        let index = self.index.get_or_init(|| {
            Index::new(
                self.content
                    .doc
                    .units
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| searchable(&u.kind))
                    .map(|(i, u)| (i, self.shown(i).0, u.copy_text.ends_with('\n'))),
            )
        });
        index.find(&self.query)
    }

    /// Selects match `c` and scrolls to it. A match that spans two units selects only its
    /// part in the first (Rune selects within one text from code: R1).
    fn select_current(&mut self) {
        let Some(m) = self.current.map(|c| self.matches[c]) else {
            return;
        };
        let len = self.shown(m.start.unit).0.len();
        let end = if m.end.unit == m.start.unit {
            m.end.offset
        } else {
            len
        };
        let row = self.row_key(m.start.unit);
        self.pending.push(Request::Reveal(row.clone()));
        self.pending
            .push(Request::Select(format!("{row}/t"), m.start.offset..end));
    }

    /// Copying the whole document, as Ctrl+A then Ctrl+C takes it: every unit's copy text.
    pub fn copy_all(&self) -> String {
        self.content.copy_all()
    }

    /// What copying `sel` takes, by Strata's rule.
    pub fn copy_selection(&self, sel: &TextSelection) -> String {
        self.content.copy(sel)
    }
}

/// Whether find searches a unit of this kind.
pub(crate) fn searchable(kind: &UnitKind) -> bool {
    matches!(
        kind,
        UnitKind::Heading(_)
            | UnitKind::Paragraph
            | UnitKind::ListItem { .. }
            | UnitKind::ListChild { .. }
            | UnitKind::Quote
            | UnitKind::Code { .. }
    )
}

/// Whether a unit is a code block's first line of its block (where Copy code goes).
pub(crate) fn has_copy_button(u: &Unit) -> bool {
    u.first && matches!(u.kind, UnitKind::Code { .. })
}

fn shown_for(u: &Unit) -> Option<Shown> {
    let mut inserts = math_inserts(&u.text, &u.spans);
    if has_copy_button(u) {
        let at = u.text.find('\n').unwrap_or(u.text.len());
        inserts.push(Insert {
            old: at..at,
            new: BUTTON_ROOM.into(),
            copy: String::new(),
        });
    }
    (!inserts.is_empty()).then(|| Shown::with(&u.text, &u.spans, inserts))
}

impl HostedPreview for DocumentState {
    fn document_view(&self) -> Option<DocumentView> {
        self.source_available.then_some(DocumentView::Rendered)
    }

    fn wrap(&self) -> Option<bool> {
        self.wrap
    }

    fn searchable(&self) -> bool {
        self.searchable
    }

    fn find(&mut self, query: &str) {
        self.query = query.to_owned();
        self.matches = self.run_find();
        self.current = (!self.matches.is_empty()).then_some(0);
        // Each edit to the query clears the selection; the first match then takes it.
        self.pending.push(Request::Clear);
        self.content.all.set(false);
        self.select_current();
    }

    fn find_step(&mut self, forward: bool) {
        let n = self.matches.len();
        if n == 0 {
            return;
        }
        self.current = Some(match self.current {
            Some(c) if forward => (c + 1) % n,
            Some(c) => (c + n - 1) % n,
            None => 0,
        });
        self.select_current();
    }

    fn find_status(&self) -> Option<FindStatus> {
        Some(FindStatus {
            current: self.current.map(|c| c + 1),
            total: self.matches.len(),
        })
    }

    fn find_end(&mut self) {
        // The selected match stays selected for Ctrl+C.
        self.query.clear();
        self.matches.clear();
        self.current = None;
    }
}
