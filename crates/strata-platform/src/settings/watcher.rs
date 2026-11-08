//! The watcher thread: a tokio current-thread runtime with one zbus session connection, as
//! Rune's own `color-scheme` reader runs (`platform/portal_appearance.rs`).

use std::io;
use std::pin::pin;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use futures_util::future::select;
use futures_util::stream::{self, BoxStream, StreamExt};
use tokio::sync::oneshot;
use zbus::OwnedVariant;
use zbus::proxy::{Builder, CacheProperties};
use zbus::{Connection, Message, Proxy};

use super::fallback::{Fallback, Reported};
use super::keys::{NAMESPACES, Setting};
use super::parse::{self, Applied, Reply};
use super::snapshot::DesktopSettings;

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Settings";

/// The watcher thread's name.
pub const THREAD_NAME: &str = "desktop-settings";

/// How long dropping a [`Watcher`] waits for its thread to stop before leaving it to finish
/// alone, so a drop never holds the UI thread for long.
const STOP_WAIT: Duration = Duration::from_millis(200);

/// What the watcher reports, in order: one [`Event::Snapshot`] first, then changes and
/// fallbacks as they happen.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// The first answer: the portal's values, or the defaults once the portal or the bus is
    /// known to be missing (a [`Event::Fallback`] then says why). Sent exactly once.
    Snapshot(DesktopSettings),
    /// One key changed, or became known: its new value, and every setting just after it.
    Changed {
        setting: Setting,
        settings: DesktopSettings,
    },
    /// A fallback, the first time its kind (and key) happens.
    Fallback(Fallback),
}

/// How the watcher waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    /// How long [`Watcher::start`] waits for the first answer, so the app's first frame can use
    /// it: 100 ms by default, as Rune waits for `color-scheme`. A later answer arrives as
    /// changes. Zero never waits.
    pub first_answer: Duration,
    /// The bound on connecting, on activating the portal, and on each read: 10 s by default.
    pub timeout: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            first_answer: Duration::from_millis(100),
            timeout: Duration::from_secs(10),
        }
    }
}

impl Options {
    /// The defaults with another first-answer wait.
    pub fn first_answer(self, first_answer: Duration) -> Self {
        Options {
            first_answer,
            ..self
        }
    }

    /// The defaults with another bound on connecting and reading.
    pub fn timeout(self, timeout: Duration) -> Self {
        Options { timeout, ..self }
    }
}

/// Reads the desktop's settings and follows their changes on a thread of its own
/// ([`THREAD_NAME`]). Dropping it stops the thread.
pub struct Watcher {
    latest: Arc<Mutex<DesktopSettings>>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
    /// Disconnects when the thread ends.
    ended: mpsc::Receiver<()>,
}

impl Watcher {
    /// Starts the watcher. `sink` hears every [`Event`] on the watcher's thread, in order;
    /// [`follow`](super::follow) is the sink that moves them onto Rune's loop. Waits up to
    /// [`Options::first_answer`] for the first answer, and fails only if the thread can't be
    /// spawned.
    pub fn start(
        options: Options,
        sink: impl Fn(Event) + Send + Sync + 'static,
    ) -> io::Result<Watcher> {
        let latest = Arc::new(Mutex::new(DesktopSettings::default()));
        let (stop, stopped) = oneshot::channel();
        let (answered, first) = mpsc::sync_channel(1);
        let (ended_tx, ended) = mpsc::channel::<()>();
        let mut feed = Feed {
            latest: Arc::clone(&latest),
            sink: Box::new(sink),
            settings: DesktopSettings::default(),
            reported: Reported::default(),
            answered: Some(answered),
        };
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || {
                let _ended = ended_tx;
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(async {
                        // Stopping drops the watch wherever it waits.
                        let _ = select(pin!(watch(options, &mut feed)), stopped).await;
                    }),
                    Err(error) => feed.missing(Fallback::NoBus(format!(
                        "no runtime for the connection: {error}"
                    ))),
                }
            })?;
        if !options.first_answer.is_zero() {
            let _ = first.recv_timeout(options.first_answer);
        }
        Ok(Watcher {
            latest,
            stop: Some(stop),
            thread: Some(thread),
            ended,
        })
    }

    /// The latest settings: the defaults until the first answer. An app can seed its state
    /// with it right after [`Watcher::start`].
    pub fn snapshot(&self) -> DesktopSettings {
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        // Join only a thread that has ended. One still busy after STOP_WAIT is detached: it
        // stops at its next await, and dropping never hangs the app on it.
        if let Err(RecvTimeoutError::Disconnected) = self.ended.recv_timeout(STOP_WAIT)
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

/// The watcher's side: the settings as it last reported them, and where reports go.
struct Feed {
    latest: Arc<Mutex<DesktopSettings>>,
    sink: Box<dyn Fn(Event) + Send + Sync>,
    settings: DesktopSettings,
    reported: Reported,
    /// Taken by the snapshot, which ends `start`'s wait.
    answered: Option<mpsc::SyncSender<()>>,
}

impl Feed {
    fn store(&self) {
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = self.settings;
    }

    fn snapshot(&mut self) {
        self.store();
        (self.sink)(Event::Snapshot(self.settings));
        if let Some(answered) = self.answered.take() {
            let _ = answered.try_send(());
        }
    }

    fn fallback(&mut self, fallback: Fallback) {
        if self.reported.first(&fallback) {
            (self.sink)(Event::Fallback(fallback));
        }
    }

    /// The bus or the portal is missing: the snapshot, if none went out yet, then why.
    fn missing(&mut self, why: Fallback) {
        if self.answered.is_some() {
            self.snapshot();
        }
        self.fallback(why);
    }

    /// Reports what a reply or a change did. Before the snapshot, the changes are folded into
    /// it; after, each is its own event.
    fn report(&mut self, applied: Applied) {
        if self.answered.is_some() {
            self.snapshot();
        } else if !applied.changes.is_empty() {
            self.store();
            for (setting, settings) in applied.changes {
                (self.sink)(Event::Changed { setting, settings });
            }
        }
        for fallback in applied.fallbacks {
            self.fallback(fallback);
        }
    }
}

enum Input {
    Changed(Message),
    Owner(Option<String>),
}

async fn watch(options: Options, feed: &mut Feed) {
    let connection = match tokio::time::timeout(options.timeout, Connection::session()).await {
        Ok(Ok(connection)) => connection,
        Ok(Err(error)) => return feed.missing(Fallback::NoBus(error.to_string())),
        Err(_) => {
            return feed.missing(Fallback::NoBus(
                "timed out connecting to the session bus".into(),
            ));
        }
    };
    let subscribed = tokio::time::timeout(options.timeout, subscribe(&connection)).await;
    let (portal, inputs) = match subscribed {
        Ok(Ok(subscribed)) => subscribed,
        Ok(Err(error)) => return feed.missing(Fallback::NoPortal(error.to_string())),
        Err(_) => {
            return feed.missing(Fallback::NoPortal(
                "timed out subscribing to the Settings portal".into(),
            ));
        }
    };
    // Subscribed before the first read, so no change between the two is lost.
    let mut owner = read(&portal, options.timeout, feed).await;
    let mut inputs = inputs;
    while let Some(input) = inputs.next().await {
        match input {
            Input::Changed(message) => {
                let Ok((namespace, name, value)) = message
                    .body()
                    .deserialize::<(String, String, OwnedVariant)>()
                else {
                    continue;
                };
                let applied = parse::apply_change(&mut feed.settings, &namespace, &name, &value);
                feed.report(applied);
            }
            // A new portal (a restart, or one that appeared late): read it again. The owner of
            // a portal the first read activated is the one that answered, so it isn't read twice.
            Input::Owner(Some(name)) if owner.as_deref() != Some(name.as_str()) => {
                owner = read(&portal, options.timeout, feed).await;
            }
            Input::Owner(Some(_)) => {}
            Input::Owner(None) => {
                if owner.take().is_some() {
                    feed.fallback(Fallback::PortalLost);
                }
            }
        }
    }
}

/// The portal proxy, and one stream of its changes and its owner changes. All of
/// `SettingChanged` comes through one match rule: a rule per namespace would let the merged
/// streams reorder changes across namespaces, which the bus delivers in order. Other
/// namespaces' changes wake the thread and are dropped by key.
async fn subscribe(
    connection: &Connection,
) -> zbus::Result<(Proxy<'static>, BoxStream<'static, Input>)> {
    let portal: Proxy<'static> = Builder::new(connection)
        .destination(DESTINATION)
        .path(PATH)
        .interface(INTERFACE)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let changes = portal
        .receive_signal("SettingChanged")
        .await?
        .map(Input::Changed);
    let owners = portal
        .receive_owner_changed()
        .await?
        .map(|owner| Input::Owner(owner.map(|name| name.to_string())));
    Ok((portal, stream::select(changes, owners).boxed()))
}

/// Reads every namespace at once and reports it. Returns the unique name that answered, or
/// None when the read failed (reported as [`Fallback::NoPortal`]).
async fn read(portal: &Proxy<'static>, timeout: Duration, feed: &mut Feed) -> Option<String> {
    let namespaces = (&NAMESPACES[..],);
    let call = portal.call_method("ReadAll", &namespaces);
    let reply = match tokio::time::timeout(timeout, call).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => {
            feed.missing(Fallback::NoPortal(error.to_string()));
            return None;
        }
        Err(_) => {
            feed.missing(Fallback::NoPortal(
                "timed out reading the Settings portal".into(),
            ));
            return None;
        }
    };
    let Ok(values) = reply.body().deserialize::<Reply>() else {
        feed.missing(Fallback::NoPortal(
            "the Settings portal's ReadAll answer isn't a{sa{sv}}".into(),
        ));
        return None;
    };
    let applied = parse::apply_reply(&mut feed.settings, &values);
    feed.report(applied);
    reply.header().sender().map(|name| name.to_string())
}
