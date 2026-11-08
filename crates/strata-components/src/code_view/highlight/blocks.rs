//! Code blocks: the adapter for rune-kit's `Markdown` highlight hook, and for C11's own
//! blocks.
//!
//! rune-kit calls its hook while building a view, on the UI thread, and caches the answer per
//! highlighter, language and code. A block's first call misses: it asks the thread and
//! answers plain. When the colours land, [`CodeBlocks::poll`] bumps an epoch that is part of
//! the hook's cache key, so the next build asks again and gets them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rune::Color;
use rune_kit::syntax::{Highlight, Paint};

use super::engine::{Reply, Token};
use super::spans;
use super::worker::Request;

/// Highlighted blocks kept, in bytes of code, before the least recently asked go. Blocks asked
/// for since the last poll (the current build's) are never evicted: the cap stretches to fit
/// one build, so a large document can't evict what it is drawing and ask again.
const CACHE_BYTES: usize = 4 << 20;

/// A pool of code blocks on the highlighter thread, from [`Highlighter::blocks`]. Clones
/// share the pool; it lives on the UI thread.
///
/// [`Highlighter::blocks`]: super::Highlighter::blocks
#[derive(Clone)]
pub struct CodeBlocks {
    state: Rc<RefCell<Blocks>>,
}

struct Entry {
    lang: Box<str>,
    code: Box<str>,
    /// `None` while the thread works on it.
    tokens: Option<Rc<[Token]>>,
    /// When it was last asked for, in [`Blocks::tick`]s.
    used: u64,
}

struct Blocks {
    id: u64,
    tx: Option<Sender<Request>>,
    rx: Receiver<Reply>,
    entries: HashMap<u64, Entry>,
    bytes: usize,
    /// Counts `highlight` calls.
    tick: u64,
    /// The tick of the last poll: entries used after it belong to the current build.
    polled: u64,
    palette: Vec<Option<Color>>,
    epoch: u64,
    wait: Option<Duration>,
}

fn key_of(lang: &str, code: &str) -> u64 {
    let mut h = DefaultHasher::new();
    (lang, code).hash(&mut h);
    h.finish()
}

impl CodeBlocks {
    pub(crate) fn new(tx: Option<Sender<Request>>) -> CodeBlocks {
        let id = super::next_id();
        let (reply, rx) = mpsc::channel();
        if let Some(tx) = &tx {
            let _ = tx.send(Request::OpenPool { id, reply });
        }
        CodeBlocks {
            state: Rc::new(RefCell::new(Blocks {
                id,
                tx,
                rx,
                entries: HashMap::new(),
                bytes: 0,
                tick: 0,
                polled: 0,
                palette: Vec::new(),
                epoch: 0,
                wait: None,
            })),
        }
    }

    /// A highlighter for `Markdown::highlighter`. Build a fresh one each time the view
    /// builds: its cache key changes when colours land or the theme changes.
    pub fn markdown(&self) -> rune_kit::syntax::Highlighter {
        let (id, epoch) = {
            let s = self.state.borrow();
            (s.id, s.epoch)
        };
        let state = self.state.clone();
        rune_kit::syntax::Highlighter::new(move |lang, code| {
            state.borrow_mut().highlight(lang, code).unwrap_or_default()
        })
        .key((id, epoch))
    }

    /// `code`'s colours as rune-kit highlights (byte ranges into `code`), or `None` while
    /// the thread works on it: draw it plain meanwhile. A block with no language, or one no
    /// grammar knows, is plain: `Some` of nothing.
    pub fn highlight(&self, lang: Option<&str>, code: &str) -> Option<Vec<Highlight>> {
        self.state.borrow_mut().highlight(lang, code)
    }

    /// Takes the blocks that have landed. True when colours changed, so the host rebuilds.
    pub fn poll(&mut self) -> bool {
        let mut s = self.state.borrow_mut();
        s.polled = s.tick;
        let mut changed = false;
        while let Ok(reply) = s.rx.try_recv() {
            changed |= s.apply(reply);
        }
        if changed {
            s.epoch += 1;
        }
        changed
    }

    /// Makes a miss wait up to `wait` for the thread instead of answering plain. **For
    /// headless runs and goldens only**, so a scene's first build is already coloured: in a
    /// window it would stall the frame.
    pub fn wait_on_miss(&mut self, wait: Option<Duration>) {
        self.state.borrow_mut().wait = wait;
    }

    /// Blocks until every block asked for has landed, or `timeout` passes. True when they
    /// all did. For tests and headless snapshots only.
    pub fn settle(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut s = self.state.borrow_mut();
        let mut changed = false;
        while s.pending() {
            let left = deadline.saturating_duration_since(Instant::now());
            match s.rx.recv_timeout(left) {
                Ok(reply) => changed |= s.apply(reply),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        if changed {
            s.epoch += 1;
        }
        !s.pending()
    }
}

impl Blocks {
    fn pending(&self) -> bool {
        self.tx.is_some() && self.entries.values().any(|e| e.tokens.is_none())
    }

    fn apply(&mut self, reply: Reply) -> bool {
        match reply {
            Reply::Palette { from, colors } => {
                self.palette.truncate(from);
                self.palette.extend(colors);
                true
            }
            Reply::Block { key, tokens } => match self.entries.get_mut(&key) {
                Some(e) => {
                    e.tokens = Some(tokens.into());
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    fn highlight(&mut self, lang: Option<&str>, code: &str) -> Option<Vec<Highlight>> {
        // A fence with no language is as often output or prose as code (rune-kit's rule).
        let Some(lang) = lang.map(str::trim).filter(|l| !l.is_empty()) else {
            return Some(Vec::new());
        };
        // No highlighter thread (it couldn't start, or it stopped): plain, as documented.
        if self.tx.is_none() {
            return Some(Vec::new());
        }
        self.tick += 1;
        let key = key_of(lang, code);
        match self.entries.get_mut(&key) {
            Some(e) if *e.lang == *lang && *e.code == *code => e.used = self.tick,
            _ => self.request(key, lang, code),
        }
        if self.entries.get(&key).is_some_and(|e| e.tokens.is_none())
            && let Some(wait) = self.wait
        {
            self.wait_for(key, wait);
        }
        let tokens = self.entries.get(&key)?.tokens.clone()?;
        let mut runs: Vec<(Range<usize>, Color)> = Vec::new();
        spans::colored(&tokens, &self.palette, 0, &mut runs);
        Some(
            runs.into_iter()
                .map(|(r, c)| (r, Paint::Color(c)))
                .collect(),
        )
    }

    fn request(&mut self, key: u64, lang: &str, code: &str) {
        let Some(tx) = &self.tx else {
            return;
        };
        let sent = tx.send(Request::Block {
            pool: self.id,
            key,
            lang: lang.into(),
            code: Arc::from(code),
        });
        if sent.is_err() {
            // The thread is gone: every block is plain from now on.
            self.tx = None;
            return;
        }
        let entry = Entry {
            lang: lang.into(),
            code: code.into(),
            tokens: None,
            used: self.tick,
        };
        // A hash collision replaces the other block: count only the one kept.
        if let Some(old) = self.entries.insert(key, entry) {
            self.bytes = self.bytes.saturating_sub(old.code.len());
        }
        self.bytes += code.len();
        self.evict();
    }

    /// Drops the least recently asked blocks until the cache fits, sparing the current
    /// build's.
    fn evict(&mut self) {
        while self.bytes > CACHE_BYTES {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, e)| e.used <= self.polled)
                .min_by_key(|(_, e)| e.used)
                .map(|(&k, _)| k);
            let Some(key) = oldest else { break };
            if let Some(e) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(e.code.len());
            }
        }
    }

    fn wait_for(&mut self, key: u64, wait: Duration) {
        let deadline = Instant::now() + wait;
        while self.entries.get(&key).is_some_and(|e| e.tokens.is_none()) {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(reply) => {
                    self.apply(reply);
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}

impl Drop for Blocks {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Request::Close { id: self.id });
        }
    }
}
