//! Claiming the app id, and the primary that then serves it.

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use tokio::sync::oneshot;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{Connection, connection};

use super::app_id::AppId;
use super::event::{CommandLine, Event};
use super::forward::{self, Forward};
use super::interfaces::{
    FILE_MANAGER1, FILE_MANAGER1_PATH, FileManager1, FreedesktopApplication, GtkActions,
    GtkApplication,
};
use super::launch::Launch;

/// The bus thread's name.
pub const THREAD_NAME: &str = "single-instance-bus";

/// How many times a launch tries again when the primary it reached went away mid-launch.
const ATTEMPTS: usize = 3;

/// How [`claim`] behaves.
#[derive(Clone, Debug)]
pub struct Options {
    /// The longest the claim waits to connect to the session bus and hear whether it owns the
    /// app id. A bus that never answers leaves the launch [`Unclaimed::Timeout`].
    pub bus_timeout: Duration,
    /// The longest a forwarded launch waits for the primary's answer, which comes once the app
    /// is done with its [`CommandLine`].
    pub forward_timeout: Duration,
    /// A launch the session bus started to provide a service (D-Bus activation): it queues
    /// no [`Event::CommandLine`] of its own, and when another process already is the primary
    /// it ends with `Claim::Forwarded(0)`, forwarding nothing.
    pub service: bool,
    /// Also serve `org.freedesktop.FileManager1` as the primary, delivering its calls as
    /// [`Event::Reveal`]. The name is queued for, never taken: while another file manager
    /// holds it, calls keep reaching that one.
    pub file_manager1: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bus_timeout: Duration::from_secs(5),
            forward_timeout: Duration::from_secs(30),
            service: false,
            file_manager1: false,
        }
    }
}

/// How a launch ended up.
#[derive(Debug)]
pub enum Claim {
    /// This process owns the app id. Its own launch is the first [`Event::CommandLine`].
    Primary(Primary),
    /// Another process is the primary and took this launch. Exit with this status: the
    /// primary's answer, or 1 when it couldn't be reached (said on stderr).
    Forwarded(i32),
    /// No session bus to claim the app id on: this process runs on its own, unclaimed. The
    /// [`Primary`] still delivers its own launch, and nothing else.
    Standalone(Primary, Unclaimed),
}

/// Why a launch couldn't claim the app id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unclaimed {
    /// No session bus answered: none is set, or connecting failed.
    NoBus(String),
    /// The bus took longer than `Options::bus_timeout`.
    Timeout,
    /// The bus answered with an error.
    Error(String),
}

/// Claims `id` on the session bus for this launch.
///
/// Call it first in `main`, before Rune starts. The bus decides between launches at the same
/// moment, so exactly one becomes the primary.
///
/// - [`Claim::Primary`]: attach the app ([`Primary::attach_rune`]) and run it.
/// - [`Claim::Forwarded`]: the primary got the launch (its arguments, working directory and
///   activation token); exit with the status.
/// - [`Claim::Standalone`]: run as the primary would, alone.
///
/// The bus is `DBUS_SESSION_BUS_ADDRESS`, else `$XDG_RUNTIME_DIR/bus`.
pub fn claim(id: &AppId, launch: Launch, options: &Options) -> Claim {
    let mut gone = None;
    for _ in 0..ATTEMPTS {
        match try_primary(id, &launch, options) {
            Attempt::Primary(primary) => return Claim::Primary(primary),
            Attempt::Unclaimed(reason) => {
                return Claim::Standalone(Primary::standalone(launch, options), reason);
            }
            Attempt::Taken if options.service => return Claim::Forwarded(0),
            Attempt::Taken => match forward::forward(id, &launch, options) {
                Forward::Answered(status) => return Claim::Forwarded(status),
                Forward::Gone(why) => gone = Some(why),
                Forward::Failed(why) => return failed(id, &why),
            },
        }
    }
    failed(
        id,
        gone.as_deref()
            .unwrap_or("the running instance kept going away"),
    )
}

fn failed(id: &AppId, why: &str) -> Claim {
    super::event::write_ignoring_errors(
        &mut std::io::stderr().lock(),
        &format!("{id}: could not reach the running instance: {why}\n"),
    );
    Claim::Forwarded(1)
}

enum Attempt {
    Primary(Primary),
    Taken,
    Unclaimed(Unclaimed),
}

/// What the bus thread tells `claim`.
enum Setup {
    Primary(Bus),
    Taken,
    Unclaimed(Unclaimed),
}

/// One try at the app id, on a new bus thread. The thread stays as the primary's, or ends.
fn try_primary(id: &AppId, launch: &Launch, options: &Options) -> Attempt {
    let inbox = Arc::new(Inbox::default());
    let status = Arc::new(AtomicI32::new(0));
    // Queued before the name is, so this launch comes before any forwarded one.
    if !options.service {
        inbox.deliver(Event::CommandLine(CommandLine::local(
            launch.clone(),
            status.clone(),
        )));
    }
    let (tx, rx) = mpsc::channel();
    // Set when `claim` stopped waiting: a late connection must not become a primary nobody
    // serves.
    let abandoned = Arc::new(AtomicBool::new(false));
    let thread = {
        let (id, inbox, abandoned) = (id.clone(), inbox.clone(), abandoned.clone());
        let options = options.clone();
        std::thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || serve(id, inbox, options, abandoned, tx))
    };
    if let Err(e) = thread {
        return Attempt::Unclaimed(Unclaimed::Error(format!("starting {THREAD_NAME}: {e}")));
    }
    // The thread times itself out; the margin covers a thread that never got to run.
    let setup = rx.recv_timeout(options.bus_timeout + Duration::from_secs(1));
    match setup {
        Ok(Setup::Primary(bus)) => Attempt::Primary(Primary {
            inbox,
            status,
            bus: Some(bus),
        }),
        Ok(Setup::Taken) => Attempt::Taken,
        Ok(Setup::Unclaimed(reason)) => Attempt::Unclaimed(reason),
        Err(_) => {
            abandoned.store(true, Ordering::SeqCst);
            Attempt::Unclaimed(Unclaimed::Timeout)
        }
    }
}

/// The bus thread: claims the name, then serves until the primary is released or dropped.
fn serve(
    id: AppId,
    inbox: Arc<Inbox>,
    options: Options,
    abandoned: Arc<AtomicBool>,
    tx: mpsc::Sender<Setup>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(Setup::Unclaimed(Unclaimed::Error(format!(
                "starting the bus runtime: {e}"
            ))));
            return;
        }
    };
    runtime.block_on(async move {
        let setup = tokio::time::timeout(
            options.bus_timeout,
            become_primary(&id, &inbox, options.file_manager1),
        )
        .await;
        let conn = match setup {
            Ok(Ok(Some(conn))) => conn,
            other => {
                let _ = tx.send(match other {
                    Ok(Ok(_)) => Setup::Taken,
                    Ok(Err(reason)) => Setup::Unclaimed(reason),
                    Err(_) => Setup::Unclaimed(Unclaimed::Timeout),
                });
                return;
            }
        };
        let (stop, stopped) = oneshot::channel::<mpsc::Sender<()>>();
        let bus = Bus {
            conn: Mutex::new(Some(conn.clone())),
            runtime: tokio::runtime::Handle::current(),
            stop: Mutex::new(Some(stop)),
            bus_timeout: options.bus_timeout,
        };
        if abandoned.load(Ordering::SeqCst) || tx.send(Setup::Primary(bus)).is_err() {
            release(&conn, &id, options.file_manager1).await;
            return;
        }
        // Released (`Primary::release`) or dropped: give the names up, so the next launch
        // claims them, and let calls still waiting fail, so their launches try again.
        let done = stopped.await.ok();
        release(&conn, &id, options.file_manager1).await;
        drop(conn);
        if let Some(done) = done {
            let _ = done.send(());
        }
    });
}

/// Exports the interfaces, then asks for the name. They are exported first so that a launch
/// that finds the name taken always reaches them.
async fn become_primary(
    id: &AppId,
    inbox: &Arc<Inbox>,
    file_manager1: bool,
) -> Result<Option<Connection>, Unclaimed> {
    let conn = connection::Builder::session()
        .build()
        .await
        .map_err(|e| Unclaimed::NoBus(e.to_string()))?;
    let error = |e: zbus::Error| Unclaimed::Error(e.to_string());
    let path = id.object_path();
    let server = conn.object_server();
    let inbox = || inbox.clone();
    server
        .at(path.as_str(), GtkApplication { inbox: inbox() })
        .await
        .map_err(error)?;
    server
        .at(path.as_str(), FreedesktopApplication { inbox: inbox() })
        .await
        .map_err(error)?;
    server
        .at(path.as_str(), GtkActions { inbox: inbox() })
        .await
        .map_err(error)?;
    if file_manager1 {
        server
            .at(FILE_MANAGER1_PATH, FileManager1 { inbox: inbox() })
            .await
            .map_err(error)?;
    }
    match conn
        .request_name_with_flags(id.as_str(), RequestNameFlags::DO_NOT_QUEUE)
        .await
    {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
        Err(zbus::Error::NameTaken) | Ok(RequestNameReply::Exists) => return Ok(None),
        Ok(RequestNameReply::InQueue) => {
            return Err(Unclaimed::Error(format!("{id} was queued, not owned")));
        }
        Err(e) => return Err(error(e)),
    }
    if file_manager1 {
        // No flags: wait in the queue behind another file manager, never replace it.
        conn.request_name_with_flags(FILE_MANAGER1, RequestNameFlags::empty())
            .await
            .map_err(error)?;
    }
    Ok(Some(conn))
}

async fn release(conn: &Connection, id: &AppId, file_manager1: bool) {
    let _ = conn.release_name(id.as_str()).await;
    if file_manager1 {
        let _ = conn.release_name(FILE_MANAGER1).await;
    }
}

/// The primary's side of the bus.
#[derive(Debug)]
struct Bus {
    /// Taken on release: once every clone is gone the socket closes, and a launch still
    /// waiting for an answer hears at once that the primary left.
    conn: Mutex<Option<Connection>>,
    runtime: tokio::runtime::Handle,
    stop: Mutex<Option<oneshot::Sender<mpsc::Sender<()>>>>,
    bus_timeout: Duration,
}

/// The instance that handles launches: this process's own, then every one forwarded to it.
#[derive(Debug)]
pub struct Primary {
    inbox: Arc<Inbox>,
    status: Arc<AtomicI32>,
    bus: Option<Bus>,
}

impl Primary {
    fn standalone(launch: Launch, options: &Options) -> Primary {
        let inbox = Arc::new(Inbox::default());
        let status = Arc::new(AtomicI32::new(0));
        if !options.service {
            inbox.deliver(Event::CommandLine(CommandLine::local(
                launch,
                status.clone(),
            )));
        }
        Primary {
            inbox,
            status,
            bus: None,
        }
    }

    /// Hands every event to `sink`, from the bus thread: those queued since the claim first,
    /// in order, then each as it comes. A second call replaces the sink.
    pub fn attach(&self, sink: impl Fn(Event) + Send + 'static) {
        self.inbox.attach(Box::new(sink));
    }

    /// Delivers every event on Rune's loop: each runs `on` as an [`rune::AppHandle::update`],
    /// in arrival order, also while no window shows. `on` may open windows there
    /// (`rune::open_window`).
    pub fn attach_rune<S: 'static>(
        &self,
        handle: rune::AppHandle<S>,
        on: fn(&mut S, Event, &mut rune::Tx),
    ) {
        self.attach(move |event| handle.update(move |s, tx| on(s, event, tx)));
    }

    /// Whether this process owns the app id on the bus (false when standalone, or released).
    pub fn is_claimed(&self) -> bool {
        self.bus
            .as_ref()
            .is_some_and(|b| b.stop.lock().is_ok_and(|s| s.is_some()))
    }

    /// The status this process's own [`CommandLine`] set, once it is dropped: exit with it.
    pub fn exit_status(&self) -> i32 {
        self.status.load(Ordering::Relaxed)
    }

    /// The primary's session bus connection, to serve more on it (a portal backend, say).
    /// Use it from futures run with [`Primary::spawn`]: it lives on the bus thread's runtime.
    /// None when standalone or released. Drop the clone with the work it serves: a clone kept
    /// past [`Primary::release`] keeps the connection open.
    pub fn connection(&self) -> Option<Connection> {
        self.bus
            .as_ref()
            .and_then(|b| b.conn.lock().ok().and_then(|c| c.clone()))
    }

    /// Runs `future` on the bus thread. False when standalone, and `future` is dropped.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) -> bool {
        match &self.bus {
            Some(bus) => {
                bus.runtime.spawn(future);
                true
            }
            None => false,
        }
    }

    /// Stops being the primary: gives the app id (and `org.freedesktop.FileManager1`) up,
    /// and returns once they are released, or after `Options::bus_timeout`. Call it when the
    /// app decides to quit, so a launch at that moment starts a new primary instead of
    /// reaching one about to exit. Launches still waiting for an answer fail, and try again.
    pub fn release(&self) {
        let Some(bus) = &self.bus else { return };
        let Some(stop) = bus.stop.lock().ok().and_then(|mut s| s.take()) else {
            return;
        };
        if let Ok(mut conn) = bus.conn.lock() {
            conn.take();
        }
        let (done, released) = mpsc::channel();
        if stop.send(done).is_ok() {
            let _ = released.recv_timeout(bus.bus_timeout);
        }
    }
}

impl Drop for Primary {
    fn drop(&mut self) {
        self.release();
    }
}

pub(super) type Sink = Box<dyn Fn(Event) + Send>;

/// Events on their way to the app: queued until a sink is attached, then handed straight on.
#[derive(Default)]
pub(super) struct Inbox {
    state: Mutex<InboxState>,
}

#[derive(Default)]
struct InboxState {
    queued: Vec<Event>,
    sink: Option<Sink>,
}

impl std::fmt::Debug for Inbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Inbox")
    }
}

impl Inbox {
    pub(super) fn deliver(&self, event: Event) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &state.sink {
            Some(sink) => sink(event),
            None => state.queued.push(event),
        }
    }

    pub(super) fn attach(&self, sink: Sink) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // Under the lock, so an event arriving now waits for the queued ones.
        for event in std::mem::take(&mut state.queued) {
            sink(event);
        }
        state.sink = Some(sink);
    }
}
