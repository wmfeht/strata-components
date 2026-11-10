//! Lookups on a worker thread, results in shared handles, decoding in Rune's `ImageCache`.

#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant};

use rune::{Image, ImageCache, ImageError, Pixels, Waker};

use super::request::{IconRequest, IconSource};
use super::resolver::{FALLBACK_THEME, IconResolver, SearchPath};

/// The name of the lookup thread [`IconLoader::start`] runs.
pub const THREAD_NAME: &str = "icon-theme";

/// How an [`IconLoader`] looks up, decodes and keeps icons.
#[derive(Clone)]
pub struct Options {
    /// Where themes and unthemed icons are. Default: [`SearchPath::from_env`].
    pub search_path: SearchPath,
    /// The theme lookups start in, until [`IconLoader::set_theme`]. Default: hicolor.
    pub theme: String,
    /// The `ImageCache` that decodes found files. Pass the app's own to share its decoder
    /// threads; by default the loader makes one with the waker it starts with.
    pub cache: Option<Arc<ImageCache>>,
    /// How many recent icons stay loaded once no handle holds them. Default: 128 (GTK keeps
    /// 100).
    pub lru_entries: usize,
    /// Only icons drawn at up to this many device pixels stay loaded that way. Default: 256.
    pub lru_max_px: u32,
    /// A `load` at least this long after the last check has the thread check the themes read
    /// so far for changes on disk. Default: 5 s, as GTK.
    pub revalidate_after: Duration,
    /// Files larger than this fail instead of decoding. Default: 16 MiB, as Rune's
    /// `ImageCache::app_icon`.
    pub max_file_bytes: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            search_path: SearchPath::from_env(),
            theme: FALLBACK_THEME.to_owned(),
            cache: None,
            lru_entries: 128,
            lru_max_px: 256,
            revalidate_after: Duration::from_secs(5),
            max_file_bytes: 16 << 20,
        }
    }
}

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field("search_path", &self.search_path)
            .field("theme", &self.theme)
            .field("cache", &self.cache.as_ref().map(|_| "shared"))
            .field("lru_entries", &self.lru_entries)
            .field("lru_max_px", &self.lru_max_px)
            .field("revalidate_after", &self.revalidate_after)
            .field("max_file_bytes", &self.max_file_bytes)
            .finish()
    }
}

/// Where a [`LoadedIcon`] is.
#[derive(Clone, Debug, PartialEq)]
pub enum IconState {
    /// The lookup hasn't run yet.
    Pending,
    /// Found; the file is decoding. [`LoadedIcon::pixels`] can be drawn already, and shows the
    /// icon once it lands.
    Loading,
    /// Decoded.
    Ready,
    /// No theme in the chain, no unthemed icon and no file has it: show your own fallback.
    NotFound,
    /// Found, but the file can't be read or decoded (or is larger than
    /// [`Options::max_file_bytes`]). The lookup doesn't go on to another file, as GTK's doesn't.
    Failed(ImageError),
}

/// Looks icons up on its own thread ([`THREAD_NAME`]) and hands them over as `rune::Pixels`.
///
/// Cheap to clone; every clone shares the thread, the theme and the handles. The thread stops
/// once the last clone drops, after the lookup it is running.
#[derive(Clone)]
pub struct IconLoader {
    inner: Arc<Inner>,
}

struct Inner {
    jobs: mpsc::Sender<Job>,
    theme: Mutex<String>,
    common: Arc<Common>,
}

/// What the loader and its thread share. It holds no sender, so the thread's channel closes
/// when the last `IconLoader` drops.
struct Common {
    table: Mutex<Table>,
    settled: AtomicBool,
    /// When `load` next asks the thread to check the themes on disk.
    next_check: Mutex<Instant>,
    revalidate_after: Duration,
    cache: Arc<ImageCache>,
    waker: Waker,
    lru_entries: usize,
    lru_max_px: u32,
    max_file_bytes: u64,
}

#[derive(Default)]
struct Table {
    live: HashMap<Key, Weak<Slot>>,
    /// Most recent first.
    lru: VecDeque<Arc<Slot>>,
    /// Found icons whose decode [`IconLoader::poll`] hasn't seen land yet.
    loading: Vec<Weak<Slot>>,
}

enum Job {
    Lookup(Weak<Slot>),
    Revalidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    theme: String,
    source: IconSource,
    size: u32,
    scale: u32,
}

impl IconLoader {
    /// Starts the lookup thread. `waker` is the app's setup waker (`AppSpec::setup`): it is
    /// called each time a lookup settles, and by the default `ImageCache` when a decode lands.
    pub fn start(waker: Waker, options: Options) -> io::Result<Self> {
        let cache = options
            .cache
            .unwrap_or_else(|| Arc::new(ImageCache::new(waker.clone())));
        let common = Arc::new(Common {
            table: Mutex::new(Table::default()),
            settled: AtomicBool::new(false),
            next_check: Mutex::new(Instant::now() + options.revalidate_after),
            revalidate_after: options.revalidate_after,
            cache,
            waker,
            lru_entries: options.lru_entries,
            lru_max_px: options.lru_max_px,
            max_file_bytes: options.max_file_bytes,
        });
        let (jobs, receive) = mpsc::channel();
        let resolver = IconResolver::new(options.search_path);
        let worker = common.clone();
        thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || run(&receive, &worker, resolver))?;
        Ok(Self {
            inner: Arc::new(Inner {
                jobs,
                theme: Mutex::new(options.theme),
                common,
            }),
        })
    }

    /// The theme later lookups start in, such as the desktop's icon theme from the app's
    /// settings. Handles from before keep their icon; ask again for the new theme's.
    pub fn set_theme(&self, name: &str) {
        *lock(&self.inner.theme) = name.to_owned();
    }

    pub fn theme(&self) -> String {
        lock(&self.inner.theme).clone()
    }

    /// The icon for `request` in the current theme. It never blocks on the file system, so a
    /// view can call it each time it runs: a request asked before, while a handle to it lives
    /// or it is among the recent ones, gets the same handle back.
    pub fn load(&self, request: &IconRequest) -> LoadedIcon {
        let theme = self.theme();
        let key = Key {
            theme,
            source: request.source.clone(),
            size: request.size,
            scale: request.scale.to_bits(),
        };
        let common = &self.inner.common;
        self.check_due();
        let mut table = lock(&common.table);
        if let Some(slot) = table.live.get(&key).and_then(Weak::upgrade) {
            table.touch(&slot, common);
            return LoadedIcon(slot);
        }
        table.live.retain(|_, slot| slot.strong_count() > 0);
        let slot = Arc::new(Slot::new(request.clone(), key.theme.clone()));
        table.live.insert(key, Arc::downgrade(&slot));
        table.touch(&slot, common);
        drop(table);
        if self
            .inner
            .jobs
            .send(Job::Lookup(Arc::downgrade(&slot)))
            .is_err()
        {
            slot.settle(Outcome::Refused(ImageError(
                "the icon lookup thread stopped".into(),
            )));
        }
        LoadedIcon(slot)
    }

    /// Whether a lookup settled, or a found icon finished decoding, since the last call. Call
    /// it from `AppSpec::poll` and run the view again when it is true.
    pub fn poll(&self) -> bool {
        let common = &self.inner.common;
        let mut changed = common.settled.swap(false, Ordering::AcqRel);
        lock(&common.table)
            .loading
            .retain(|slot| match slot.upgrade() {
                Some(slot) if slot.decoding() => true,
                Some(_) => {
                    changed = true;
                    false
                }
                None => false,
            });
        changed
    }

    /// Checks the themes on disk now, on the thread, without waiting for
    /// [`Options::revalidate_after`]. Lookups asked after this call run after the check. When
    /// something changed, the loader forgets its handles (later requests get new ones), and
    /// [`poll`](Self::poll) turns true so the view asks again.
    pub fn rescan(&self) {
        *lock(&self.inner.common.next_check) = Instant::now() + self.inner.common.revalidate_after;
        let _ = self.inner.jobs.send(Job::Revalidate);
    }

    /// Asks for a check when one is due. A request that hits a shared handle never reaches the
    /// thread, so `load` itself has to ask, as GTK's lookups check every 5 s.
    fn check_due(&self) {
        let now = Instant::now();
        let mut next = lock(&self.inner.common.next_check);
        if now >= *next {
            *next = now + self.inner.common.revalidate_after;
            drop(next);
            let _ = self.inner.jobs.send(Job::Revalidate);
        }
    }
}

impl std::fmt::Debug for IconLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IconLoader")
            .field("theme", &self.theme())
            .finish_non_exhaustive()
    }
}

impl Table {
    /// Moves `slot` to the front of the recent icons, if its size lets it stay.
    fn touch(&mut self, slot: &Arc<Slot>, common: &Common) {
        if let Some(at) = self.lru.iter().position(|s| Arc::ptr_eq(s, slot)) {
            self.lru.remove(at);
        } else if slot.request.pixels() > common.lru_max_px {
            return;
        }
        self.lru.push_front(slot.clone());
        self.lru.truncate(common.lru_entries);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn run(receive: &mpsc::Receiver<Job>, common: &Common, mut resolver: IconResolver) {
    while let Ok(job) = receive.recv() {
        let slot = match job {
            Job::Revalidate => {
                if resolver.revalidate() {
                    let mut table = lock(&common.table);
                    table.live.clear();
                    table.lru.clear();
                    drop(table);
                    common.settled.store(true, Ordering::Release);
                    (common.waker)();
                }
                continue;
            }
            // Every handle dropped before its turn: nothing to look up.
            Job::Lookup(slot) => match slot.upgrade() {
                Some(slot) => slot,
                None => continue,
            },
        };
        let outcome = resolve(&mut resolver, common, &slot);
        let found = matches!(outcome, Outcome::Found { .. });
        slot.settle(outcome);
        if found {
            lock(&common.table).loading.push(Arc::downgrade(&slot));
        }
        drop(slot);
        common.settled.store(true, Ordering::Release);
        (common.waker)();
    }
}

fn resolve(resolver: &mut IconResolver, common: &Common, slot: &Slot) -> Outcome {
    let request = &slot.request;
    match &request.source {
        IconSource::Names(names) => {
            match resolver.resolve(&slot.theme, names, request.size, request.theme_scale()) {
                Some(found) => {
                    let symbolic = found.is_symbolic();
                    open(common, found.path, request, symbolic)
                }
                None => Outcome::NotFound,
            }
        }
        IconSource::File(path) => {
            let symbolic = path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.ends_with("-symbolic"));
            open(common, path.clone(), request, symbolic)
        }
    }
}

fn open(common: &Common, path: PathBuf, request: &IconRequest, symbolic: bool) -> Outcome {
    match std::fs::metadata(&path) {
        Err(e) => Outcome::Refused(ImageError(format!("{}: {e}", path.display()))),
        Ok(meta) if meta.len() > common.max_file_bytes => Outcome::Refused(ImageError(format!(
            "{} is too large for an icon",
            path.display()
        ))),
        Ok(_) => {
            let image = common.cache.file(&path, request.size as f32, request.scale);
            Outcome::Found {
                image,
                path,
                symbolic,
            }
        }
    }
}

enum Outcome {
    Found {
        image: Image,
        path: PathBuf,
        symbolic: bool,
    },
    NotFound,
    Refused(ImageError),
}

struct Slot {
    request: IconRequest,
    theme: String,
    outcome: OnceLock<Outcome>,
    lock: Mutex<()>,
    settled: Condvar,
}

impl Slot {
    fn new(request: IconRequest, theme: String) -> Self {
        Self {
            request,
            theme,
            outcome: OnceLock::new(),
            lock: Mutex::new(()),
            settled: Condvar::new(),
        }
    }

    fn settle(&self, outcome: Outcome) {
        let _ = self.outcome.set(outcome);
        let _guard = lock(&self.lock);
        self.settled.notify_all();
    }

    fn decoding(&self) -> bool {
        matches!(self.outcome.get(), Some(Outcome::Found { image, .. }) if !image.is_ready())
    }
}

/// A shared handle on one lookup. Keep it while the icon shows: it, or the loader's recent
/// icons, keep the decoded pixels alive.
#[derive(Clone)]
pub struct LoadedIcon(Arc<Slot>);

impl LoadedIcon {
    pub fn state(&self) -> IconState {
        match self.0.outcome.get() {
            None => IconState::Pending,
            Some(Outcome::NotFound) => IconState::NotFound,
            Some(Outcome::Refused(e)) => IconState::Failed(e.clone()),
            Some(Outcome::Found { image, .. }) => match image.error() {
                Some(e) => IconState::Failed(e.clone()),
                None if image.is_ready() => IconState::Ready,
                None => IconState::Loading,
            },
        }
    }

    /// The icon to draw with `El::pixels`, once found. It paints nothing while it decodes, and
    /// Rune redraws it when it lands, without the view running again.
    pub fn pixels(&self) -> Option<&Pixels> {
        self.image().map(Image::pixels)
    }

    /// The decoding image, once found: its size, or why it failed.
    pub fn image(&self) -> Option<&Image> {
        match self.0.outcome.get()? {
            Outcome::Found { image, .. } => Some(image),
            _ => None,
        }
    }

    /// The file found.
    pub fn path(&self) -> Option<&Path> {
        match self.0.outcome.get()? {
            Outcome::Found { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Whether the icon was found under a `-symbolic` name. It draws as its file paints it,
    /// usually grey, not in the foreground colour GTK gave it.
    pub fn is_symbolic(&self) -> bool {
        matches!(
            self.0.outcome.get(),
            Some(Outcome::Found { symbolic: true, .. })
        )
    }

    pub fn request(&self) -> &IconRequest {
        &self.0.request
    }

    /// The theme the lookup started in.
    pub fn theme(&self) -> &str {
        &self.0.theme
    }

    /// Blocks up to `timeout` for the lookup, then (if it found a file) until the file has
    /// decoded, and returns the state. For headless fixtures and tools, like `Image::wait`;
    /// an app uses its waker and [`IconLoader::poll`] instead.
    pub fn wait(&self, timeout: Duration) -> IconState {
        let deadline = Instant::now() + timeout;
        let mut guard = lock(&self.0.lock);
        while self.0.outcome.get().is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return IconState::Pending;
            }
            guard = self
                .0
                .settled
                .wait_timeout(guard, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        drop(guard);
        if let Some(image) = self.image() {
            let _ = image.wait();
        }
        self.state()
    }
}

impl std::fmt::Debug for LoadedIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedIcon")
            .field("request", &self.0.request)
            .field("theme", &self.0.theme)
            .field("state", &self.state())
            .field("path", &self.path())
            .finish()
    }
}
