//! Code blocks: the adapter for rune-kit's `Markdown` highlight hook, and for C11's own
//! blocks.
//!
//! rune-kit calls its hook while building a view, on the UI thread, and caches the answer per
//! highlighter, language and code. A block's first call misses: it asks the thread and
//! answers plain. When the colours land, [`CodeBlocks::poll`] bumps an epoch that is part of
//! the hook's cache key, so the next build asks again and gets them.
//!
//! An epoch bump is also where the cache trims: after one, the host builds again and every
//! block on screen is asked for, through any number of hooks. So a block asked for in the
//! last two builds (between bumps) is on screen, or was a moment ago, and is never evicted;
//! the others go, least recently asked first, down to [`CACHE_BYTES`]. A miss never evicts
//! a block the current build may still ask for.

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

/// The cache trims to this many bytes of code at each epoch bump, sparing the blocks asked for
/// in the last two builds: the cap stretches to fit what is on screen, so a large document
/// (or two views of one pool) can't evict what it is drawing and ask for it again.
const CACHE_BYTES: usize = 4 << 20;

/// How far the cap stretches (512 blocks at the 32 KiB cut). A miss past it first evicts
/// blocks not asked for since the last bump; a block that would still take the cache past it
/// answers plain and is never asked for, so nothing lands, nothing wakes the host, and memory
/// stays bounded. A host that never polls (one that waits on misses) never trims, so its
/// cache grows to here.
///
/// Both bounds count code bytes. Each landed block also keeps its runs, 12 bytes each: about
/// one run per 5 to 8 bytes of typical code, so 2 to 3 times the code, and at worst (a run per
/// byte) 13 times. rune-kit's hook keeps its own cache besides, within its own budget.
const CACHE_CEILING: usize = 4 * CACHE_BYTES;

/// A block colours up to its last line end within this many bytes, rune-kit's cut for an
/// app's highlighter (`rune_kit::syntax`). `Markdown` already hands over no more; C11's own
/// blocks get the same cut, so a block colours alike either way, and one huge block can't
/// hold the thread or the cache.
const BLOCK_CUT: usize = 32 * 1024;

/// How much of `code` gets colour: all of it, or up to its last line end within
/// [`BLOCK_CUT`] (none when the first line runs past it), as rune-kit cuts.
fn coloured_len(code: &str) -> usize {
    if code.len() <= BLOCK_CUT {
        return code.len();
    }
    code.as_bytes()[..BLOCK_CUT]
        .iter()
        .rposition(|&b| b == b'\n')
        .unwrap_or(0)
}

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
    /// The code as sent to the thread: its reply carries the same `Arc`, so a reply for a
    /// block since replaced under the same key is told apart.
    code: Arc<str>,
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
    /// The tick at the last epoch bump: blocks asked for after it belong to the current build.
    build_start: u64,
    /// The tick at the bump before: blocks asked for after it were in the last two builds.
    prev_start: u64,
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
                build_start: 0,
                prev_start: 0,
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
    /// grammar knows, is plain: `Some` of nothing. Colour stops at the last line end within
    /// 32 KiB, as rune-kit's `Markdown` cuts, and a block past the cache's ceiling for one
    /// build is plain.
    pub fn highlight(&self, lang: Option<&str>, code: &str) -> Option<Vec<Highlight>> {
        self.state.borrow_mut().highlight(lang, code)
    }

    #[cfg(test)]
    pub(crate) fn cached_bytes(&self) -> usize {
        self.state.borrow().bytes()
    }

    /// Whether `code`'s colours (cut as `highlight` cuts) are cached and landed.
    #[cfg(test)]
    pub(crate) fn has_landed(&self, lang: &str, code: &str) -> bool {
        let s = self.state.borrow();
        let code = &code[..coloured_len(code)];
        s.entries
            .get(&key_of(lang, code))
            .is_some_and(|e| *e.code == *code && e.tokens.is_some())
    }

    /// The keys of the blocks asked for that haven't landed.
    #[cfg(test)]
    pub(crate) fn pending_keys(&self) -> Vec<u64> {
        let s = self.state.borrow();
        s.entries
            .iter()
            .filter(|(_, e)| e.tokens.is_none())
            .map(|(&k, _)| k)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn pool_id(&self) -> u64 {
        self.state.borrow().id
    }

    /// Takes the blocks that have landed. True when colours changed: the host builds again,
    /// asking for every block on screen (the next `markdown()` hook has a new cache key).
    pub fn poll(&mut self) -> bool {
        let mut s = self.state.borrow_mut();
        let mut changed = false;
        while let Ok(reply) = s.rx.try_recv() {
            changed |= s.apply(reply);
        }
        if changed {
            s.bump();
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
            s.bump();
        }
        !s.pending()
    }
}

impl Blocks {
    fn pending(&self) -> bool {
        self.tx.is_some() && self.entries.values().any(|e| e.tokens.is_none())
    }

    /// A new epoch: the next build asks for every block on screen. Trims the cache to
    /// [`CACHE_BYTES`], sparing the blocks asked for in the last two builds.
    fn bump(&mut self) {
        self.epoch += 1;
        let spare_after = self.prev_start;
        self.evict(CACHE_BYTES, 0, spare_after);
        self.prev_start = self.build_start;
        self.build_start = self.tick;
    }

    fn apply(&mut self, reply: Reply) -> bool {
        match reply {
            Reply::Palette { from, colors } => {
                self.palette.truncate(from);
                self.palette.extend(colors);
                true
            }
            Reply::Block { key, code, tokens } => match self.entries.get_mut(&key) {
                Some(e) if Arc::ptr_eq(&e.code, &code) => {
                    e.tokens = Some(tokens.into());
                    true
                }
                _ => false,
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
        let code = &code[..coloured_len(code)];
        if code.is_empty() {
            return Some(Vec::new());
        }
        self.tick += 1;
        let key = key_of(lang, code);
        match self.entries.get_mut(&key) {
            Some(e) if *e.lang == *lang && *e.code == *code => e.used = self.tick,
            _ => {
                if !self.request(key, lang, code) {
                    return Some(Vec::new());
                }
            }
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

    /// Asks the thread for a block and keeps a pending entry. False, and nothing asked, when
    /// the block would take the cache past [`CACHE_CEILING`] even without the blocks not
    /// asked for since the last bump, or there is no thread: it is then plain.
    fn request(&mut self, key: u64, lang: &str, code: &str) -> bool {
        // A hash collision replaces the other block: count only the one kept, and drop its job.
        if let Some(old) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old.code.len());
            if old.tokens.is_none() {
                self.forget(vec![key]);
            }
        }
        let current: usize = self
            .entries
            .values()
            .filter(|e| e.used > self.build_start)
            .map(|e| e.code.len())
            .sum();
        if current + code.len() > CACHE_CEILING {
            return false;
        }
        self.evict(CACHE_CEILING, code.len(), self.build_start);
        let Some(tx) = &self.tx else {
            return false;
        };
        let code: Arc<str> = Arc::from(code);
        let sent = tx.send(Request::Block {
            pool: self.id,
            key,
            lang: lang.into(),
            code: code.clone(),
        });
        if sent.is_err() {
            // The thread is gone: every block is plain from now on.
            self.tx = None;
            return false;
        }
        self.bytes += code.len();
        let entry = Entry {
            lang: lang.into(),
            code,
            tokens: None,
            used: self.tick,
        };
        self.entries.insert(key, entry);
        true
    }

    /// Drops the least recently asked blocks asked for at or before tick `spare_after` until
    /// `extra` more bytes fit under `limit`. The thread drops the jobs of those still pending.
    fn evict(&mut self, limit: usize, extra: usize, spare_after: u64) {
        let mut forgotten = Vec::new();
        while self.bytes + extra > limit {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, e)| e.used <= spare_after)
                .min_by_key(|(_, e)| e.used)
                .map(|(&k, _)| k);
            let Some(key) = oldest else { break };
            if let Some(e) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(e.code.len());
                if e.tokens.is_none() {
                    forgotten.push(key);
                }
            }
        }
        if !forgotten.is_empty() {
            self.forget(forgotten);
        }
    }

    /// Tells the thread to drop the jobs of blocks no longer wanted.
    fn forget(&mut self, keys: Vec<u64>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Request::Forget {
                pool: self.id,
                keys,
            });
        }
    }

    fn wait_for(&mut self, key: u64, wait: Duration) {
        let deadline = Instant::now() + wait;
        while self.entries.get(&key).is_some_and(|e| e.tokens.is_none()) {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(reply) => {
                    // A new theme (a palette replacing colours already given) must reach
                    // rune-kit's cache too: bump the epoch, as `poll` would.
                    let theme =
                        matches!(&reply, Reply::Palette { from, .. } if *from < self.palette.len());
                    self.apply(reply);
                    if theme {
                        self.bump();
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Drop for Blocks {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Request::Close { id: self.id });
        }
    }
}
