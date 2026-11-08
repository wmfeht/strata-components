//! A fake `org.freedesktop.portal.Settings`, served from the test process on the session's
//! private bus only: it connects to the address it is given (`Session::bus().address()`), never
//! to a bus from the environment. It answers `ReadAll`, `Read` and `ReadOne` from the values
//! it holds, emits `SettingChanged` when a test changes one, logs every call, and can give up
//! its name and take it back, as a restarted portal does.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use tokio::runtime::Handle;
use tokio::sync::oneshot;
use zbus::message::Header;
use zbus::{Connection, OwnedVariant, Variant, fdo};

const NAME: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Settings";

/// One method call the fake answered.
#[derive(Clone, Debug)]
pub struct Call {
    pub method: String,
    pub sender: String,
}

#[derive(Default)]
struct State {
    values: BTreeMap<(String, String), OwnedVariant>,
    calls: Vec<Call>,
}

struct Settings {
    state: Arc<Mutex<State>>,
}

fn owned(value: &Variant<'_>) -> OwnedVariant {
    value.try_to_owned().expect("a plain value")
}

fn not_found() -> fdo::Error {
    fdo::Error::Failed("no such key".into())
}

impl Settings {
    fn record(&self, method: &str, header: &Header<'_>) {
        let sender = header.sender().map(|s| s.to_string()).unwrap_or_default();
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .calls
            .push(Call {
                method: method.into(),
                sender,
            });
    }

    fn value(&self, namespace: &str, key: &str) -> Option<OwnedVariant> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let value = state.values.get(&(namespace.to_owned(), key.to_owned()))?;
        Some(value.try_clone().expect("a plain value"))
    }
}

#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl Settings {
    async fn read_all(
        &self,
        namespaces: Vec<String>,
        #[zbus(header)] header: Header<'_>,
    ) -> HashMap<String, HashMap<String, OwnedVariant>> {
        self.record("ReadAll", &header);
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut reply: HashMap<String, HashMap<String, OwnedVariant>> = HashMap::new();
        for ((namespace, key), value) in &state.values {
            if namespaces.is_empty() || namespaces.contains(namespace) {
                reply
                    .entry(namespace.clone())
                    .or_default()
                    .insert(key.clone(), value.try_clone().expect("a plain value"));
            }
        }
        reply
    }

    /// Wraps its answer in a second variant, as real portals do.
    async fn read(
        &self,
        namespace: String,
        key: String,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<OwnedVariant> {
        self.record("Read", &header);
        let value = self.value(&namespace, &key).ok_or_else(not_found)?;
        Ok(owned(&Variant::Variant(Box::new(Variant::from(value)))))
    }

    async fn read_one(
        &self,
        namespace: String,
        key: String,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<OwnedVariant> {
        self.record("ReadOne", &header);
        self.value(&namespace, &key).ok_or_else(not_found)
    }

    #[zbus(property)]
    async fn version(&self) -> u32 {
        2
    }
}

pub struct FakePortal {
    runtime: Handle,
    /// Dropped on the runtime's watch, before the runtime stops.
    connection: Option<Connection>,
    state: Arc<Mutex<State>>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl FakePortal {
    /// Serves `values` on the bus at `address`, owning the portal's name unless `own` is false.
    pub fn start(address: &str, values: &[(&str, &str, Variant<'static>)], own: bool) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        for (namespace, key, value) in values {
            state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .values
                .insert(((*namespace).into(), (*key).into()), owned(value));
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("the fake portal's runtime");
        let handle = runtime.handle().clone();
        let (stop, stopped) = oneshot::channel::<()>();
        // The runtime's own thread drives its I/O; the test thread hands it work through
        // `Handle::block_on`.
        let thread = std::thread::Builder::new()
            .name("fake-settings-portal".into())
            .spawn(move || {
                let _ = runtime.block_on(stopped);
            })
            .expect("the fake portal's thread");
        let settings = Settings {
            state: Arc::clone(&state),
        };
        let connection = handle.block_on(async {
            zbus::connection::Builder::address(address)
                .serve_at(PATH, settings)
                .build()
                .await
                .expect("connecting the fake portal to the private bus")
        });
        let fake = FakePortal {
            runtime: handle,
            connection: Some(connection),
            state,
            stop: Some(stop),
            thread: Some(thread),
        };
        if own {
            fake.acquire();
        }
        fake
    }

    /// Takes the portal's name.
    pub fn acquire(&self) {
        self.runtime
            .block_on(self.connection().request_name(NAME))
            .expect("owning the portal's name");
    }

    /// Gives up the portal's name, as a portal that exits does.
    pub fn release(&self) {
        self.runtime
            .block_on(self.connection().release_name(NAME))
            .expect("releasing the portal's name");
    }

    /// Sets a value and announces it with `SettingChanged`.
    pub fn set(&self, namespace: &str, key: &str, value: Variant<'static>) {
        self.set_quietly(namespace, key, value.try_clone().expect("a plain value"));
        self.emit(namespace, key, value);
    }

    /// Sets a value without a signal, as a portal that changed while it was gone.
    pub fn set_quietly(&self, namespace: &str, key: &str, value: Variant<'static>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values
            .insert((namespace.into(), key.into()), owned(&value));
    }

    /// Emits `SettingChanged` without storing the value: noise and bad values.
    pub fn emit(&self, namespace: &str, key: &str, value: Variant<'static>) {
        self.runtime
            .block_on(self.connection().emit_signal(
                None::<&str>,
                PATH,
                INTERFACE,
                "SettingChanged",
                &(namespace, key, value),
            ))
            .expect("emitting SettingChanged");
    }

    fn connection(&self) -> &Connection {
        self.connection
            .as_ref()
            .expect("the fake portal's connection")
    }

    /// Every call answered so far.
    pub fn calls(&self) -> Vec<Call> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .calls
            .clone()
    }
}

impl Drop for FakePortal {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = self.runtime.block_on(connection.close());
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
