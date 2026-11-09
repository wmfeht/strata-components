//! [`PaletteState`]: the open palette, held by the host and reached by accessor.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use rune::{TextEdit, Tx};

use super::data::{Answer, PaletteItem};
use super::intent::{By, PaletteIntent, Via};
use super::mode::PaletteMode;
use super::model::{Model, Phase};
use crate::contract::Point;
use crate::scroll::Bar;

/// What a call into the palette changed, and the intents it raised for the host to route as
/// it routes the palette's own.
#[must_use = "route the intents as the palette's own"]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Polled {
    /// The palette's look changed: the host's poll returns true.
    pub changed: bool,
    pub intents: Vec<PaletteIntent>,
}

impl Polled {
    fn changed() -> Self {
        Self {
            changed: true,
            intents: Vec::new(),
        }
    }

    fn push(&mut self, intent: PaletteIntent) {
        self.changed = true;
        self.intents.push(intent);
    }
}

/// The palette's intent handler as the latest view built it. Rows are memoised, so a row
/// kept since an earlier answer would otherwise call the handler of the view that built it.
/// Type-erased, as the state isn't generic over the app's.
#[derive(Clone, Default)]
pub(crate) struct HandlerSlot(Rc<RefCell<Option<Rc<dyn Any>>>>);

impl HandlerSlot {
    pub fn set<T: 'static>(&self, handler: T) {
        *self.0.borrow_mut() = Some(Rc::new(handler));
    }

    pub fn get<T: Clone + 'static>(&self) -> Option<T> {
        self.0.borrow().as_ref()?.downcast_ref::<T>().cloned()
    }
}

impl std::fmt::Debug for HandlerSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HandlerSlot")
    }
}

/// The palette closing after a result opened: the panel is built once more, so its
/// `focus_return` can name where focus goes (Rune reads it from the panel's last build), and
/// the next [`PaletteState::tick`] after that build removes it.
#[derive(Debug, Default)]
struct Leaving {
    /// The mode it was open in: the leaving frame draws the panel as it was, so the exit
    /// motion fades what was on screen.
    mode: Option<PaletteMode>,
    built: Cell<bool>,
}

/// The palette's state: the mode it is open in, the field's text, the results and the
/// highlight. The host keeps one per window and hands the palette an accessor to it.
///
/// The host calls [`PaletteState::tick`] from its poll every frame, and
/// [`PaletteState::deliver`] with each [`Answer`] to a [`PaletteIntent::Query`].
#[derive(Debug, Default)]
pub struct PaletteState {
    mode: Option<PaletteMode>,
    /// The field's text, caret and focus. The field binds it; a host reads it, and changes it
    /// only through the methods here.
    pub edit: TextEdit,
    model: Model,
    leaving: Leaving,
    /// Intents raised where no `Tx` reaches (Rune's modal dismissal), for the next tick.
    queued: Vec<PaletteIntent>,
    pub(crate) bar: Bar,
    pub(crate) handler: HandlerSlot,
}

impl PaletteState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the palette is open.
    pub fn is_open(&self) -> bool {
        self.mode.is_some()
    }

    /// Whether the panel is built: open, or in its leaving frame.
    pub(crate) fn shell_open(&self) -> bool {
        self.mode.is_some() || self.leaving.mode.is_some()
    }

    pub(crate) fn leaving(&self) -> bool {
        self.leaving.mode.is_some()
    }

    /// The mode the panel draws: the open one, or the one it is leaving.
    pub(crate) fn drawn_mode(&self) -> Option<&PaletteMode> {
        self.mode.as_ref().or(self.leaving.mode.as_ref())
    }

    /// What the panel's body draws: the phase, also in the leaving frame.
    pub(crate) fn drawn_phase(&self) -> Option<Phase> {
        Some(self.model.phase(self.drawn_mode()?))
    }

    /// The view built the leaving frame.
    pub(crate) fn mark_built(&self) {
        if self.leaving.mode.is_some() {
            self.leaving.built.set(true);
        }
    }

    /// The mode it is open in.
    pub fn mode(&self) -> Option<&PaletteMode> {
        self.mode.as_ref()
    }

    /// The current query's generation.
    pub fn generation(&self) -> u64 {
        self.model.generation()
    }

    /// What the body shows, or `None` while closed.
    pub fn phase(&self) -> Option<Phase> {
        Some(self.model.phase(self.mode.as_ref()?))
    }

    /// The results shown.
    pub fn items(&self) -> &[PaletteItem] {
        self.model.shown()
    }

    /// The rows' version: changes whenever the shown results do.
    pub(crate) fn revision(&self) -> u64 {
        self.model.revision()
    }

    /// The highlighted result.
    pub fn highlighted(&self) -> Option<usize> {
        self.model.highlighted()
    }

    /// The highlighted result's item.
    pub fn highlighted_item(&self) -> Option<&PaletteItem> {
        self.highlighted().and_then(|i| self.items().get(i))
    }

    /// Highlights result `i` (a scene, or a host restoring a palette).
    pub fn set_highlighted(&mut self, i: Option<usize>) {
        self.model.set_highlighted(i);
    }

    /// Whether the current query still runs: the spinner shows.
    pub fn busy(&self) -> bool {
        self.is_open() && self.model.busy()
    }

    /// Why the shown results are partial, if they are.
    pub fn partial(&self) -> Option<&str> {
        self.model.partial()
    }

    /// The result count assistive technology was last told.
    pub fn announced(&self) -> Option<usize> {
        self.model.announced()
    }

    /// Opens in `mode` with an empty query, and asks for it. On an open palette it switches
    /// mode and starts over.
    pub fn open(&mut self, mode: PaletteMode) -> Polled {
        self.model.reset();
        self.leaving = Leaving::default();
        self.edit = TextEdit::new("");
        let query = self.model.request("", &mode);
        self.mode = Some(mode);
        let mut out = Polled::changed();
        out.push(PaletteIntent::Query(query));
        out
    }

    /// Ctrl+K and Ctrl+Shift+K: closes an open palette, whatever its mode (Strata), else opens
    /// it in `mode`.
    pub fn toggle(&mut self, mode: PaletteMode) -> Polled {
        if self.is_open() {
            self.close_by(By::Toggle)
        } else {
            self.open(mode)
        }
    }

    /// Closes it ([`By::Host`]).
    pub fn close(&mut self) -> Polled {
        self.close_by(By::Host)
    }

    fn close_by(&mut self, by: By) -> Polled {
        if self.mode.take().is_none() {
            return Polled::default();
        }
        self.model.reset();
        self.edit = TextEdit::new("");
        let mut out = Polled::changed();
        out.push(PaletteIntent::Closed { by });
        out
    }

    /// Sets the query to `text`, the caret at its end, and asks for it: a host opening on a
    /// query, or a scene.
    pub fn set_query(&mut self, text: &str) -> Polled {
        if !self.is_open() {
            return Polled::default();
        }
        self.edit.replace_text(text);
        self.edit.set_caret(text.len());
        self.edited()
    }

    /// An answer to a [`PaletteIntent::Query`]. One for another generation, after the palette
    /// closed, or while nothing can be searched, is dropped.
    pub fn deliver(&mut self, answer: Answer) -> Polled {
        let Some(mode) = &self.mode else {
            return Polled::default();
        };
        if self.model.arrive(answer, mode) {
            Polled::changed()
        } else {
            Polled::default()
        }
    }

    /// The query `generation` failed: the body says `message` until the next edit.
    pub fn fail(&mut self, generation: u64, message: impl Into<String>) -> bool {
        self.is_open() && self.model.fail(generation, message.into())
    }

    /// Nothing can be searched (Strata: no local search locations): the body says `message`
    /// and the field takes no text until the palette closes.
    pub fn unavailable(&mut self, message: impl Into<String>) -> bool {
        if !self.is_open() {
            return false;
        }
        self.model.unavailable(message.into());
        true
    }

    /// Opens the result `id`'s containing folder: the host's result menu.
    pub fn reveal(&mut self, id: &str, tx: &mut Tx) -> Polled {
        if !self.is_open() || !self.items().iter().any(|it| it.id == id) {
            return Polled::default();
        }
        self.finish(PaletteIntent::Reveal { id: id.to_owned() }, By::Reveal, tx)
    }

    /// One frame: ends the leaving frame once the view built it, and hands over intents
    /// raised where no `Tx` reached.
    pub fn tick(&mut self) -> Polled {
        let mut out = Polled::default();
        if self.leaving.mode.is_some() && self.leaving.built.get() {
            self.leaving = Leaving::default();
            self.model.reset();
            self.edit = TextEdit::new("");
            out.changed = true;
        }
        for intent in self.queued.drain(..) {
            out.push(intent);
        }
        out
    }

    // The palette's own handlers.

    /// The field's text changed.
    pub(crate) fn edited(&mut self) -> Polled {
        let Some(mode) = &self.mode else {
            return Polled::default();
        };
        let text = self.edit.text.clone();
        let query = self.model.request(&text, mode);
        let mut out = Polled::changed();
        out.push(PaletteIntent::Query(query));
        out
    }

    pub(crate) fn escape(&mut self) -> Polled {
        self.close_by(By::Escape)
    }

    /// Rune's modal dismissed the panel: no `Tx` reaches here, so the intent waits for the
    /// next tick.
    pub(crate) fn dismissed(&mut self) {
        let out = self.close_by(By::Dismiss);
        self.queued.extend(out.intents);
    }

    pub(crate) fn step(&mut self, delta: i32) -> bool {
        self.is_open() && self.model.step(delta)
    }

    /// Opens the highlighted result.
    pub(crate) fn open_highlighted(&mut self, new_tab: bool, tx: &mut Tx) -> Polled {
        match self.highlighted_item().map(|it| it.id.clone()) {
            Some(id) => self.choose(&id, new_tab, Via::Key, tx),
            None => Polled::default(),
        }
    }

    /// Opens the highlighted result's folder.
    pub(crate) fn reveal_highlighted(&mut self, tx: &mut Tx) -> Polled {
        match self.highlighted_item().map(|it| it.id.clone()) {
            Some(id) => self.reveal(&id, tx),
            None => Polled::default(),
        }
    }

    /// Opens result `id`.
    pub(crate) fn choose(&mut self, id: &str, new_tab: bool, via: Via, tx: &mut Tx) -> Polled {
        if !self.is_open() || !self.items().iter().any(|it| it.id == id) {
            return Polled::default();
        }
        let intent = PaletteIntent::Open {
            id: id.to_owned(),
            new_tab,
            via,
        };
        self.finish(intent, By::Open, tx)
    }

    fn finish(&mut self, intent: PaletteIntent, by: By, tx: &mut Tx) -> Polled {
        let mut out = Polled::changed();
        out.push(intent);
        // Closed, but the results and the field stay as they were until the leaving frame
        // has been built.
        self.leaving = Leaving {
            mode: self.mode.take(),
            built: Cell::new(false),
        };
        out.push(PaletteIntent::Closed { by });
        // A window at rest runs no frame by itself after this one: the tick that ends the
        // leaving frame needs one.
        tx.wake_after(Duration::ZERO);
        out
    }

    /// A right-click on result `id`.
    pub(crate) fn menu(&mut self, id: &str, at: Point) -> Polled {
        if !self.is_open() || self.model.highlight_id(id).is_none() {
            return Polled::default();
        }
        let mut out = Polled::changed();
        out.push(PaletteIntent::ResultMenu {
            id: id.to_owned(),
            at,
        });
        out
    }
}
