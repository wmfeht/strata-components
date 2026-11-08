//! The interfaces the primary serves: GApplication's (`org.gtk.Application`,
//! `org.freedesktop.Application`, `org.gtk.Actions`) at the app id's object path, and, opted
//! into, `org.freedesktop.FileManager1`.
//!
//! The signatures are GLib's (`gio/gapplicationimpl-dbus.c`), so a GApplication of the same id
//! forwards to this primary and this crate's forwarder reaches a GApplication primary.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc::unbounded_channel;
use zbus::message::Header;
use zbus::names::OwnedUniqueName;
use zbus::{Connection, OwnedObjectPath, OwnedVariant, Signature, fdo, interface};

use super::event::{CommandLine, Event, Out, Reveal, RevealMethod};
use super::launch::{Activation, Launch, PlatformData};
use super::primary::Inbox;

/// The forwarder's object for a launch's output, and GLib's interface for it.
pub(super) const CALLBACK_PATH: &str = "/org/gtk/Application/CommandLine";
pub(super) const CALLBACK_INTERFACE: &str = "org.gtk.private.CommandLine";
pub(super) const FILE_MANAGER1: &str = "org.freedesktop.FileManager1";
pub(super) const FILE_MANAGER1_PATH: &str = "/org/freedesktop/FileManager1";

pub(super) struct GtkApplication {
    pub inbox: Arc<Inbox>,
}

#[interface(name = "org.gtk.Application")]
impl GtkApplication {
    async fn activate(&self, platform_data: PlatformData) {
        self.inbox
            .deliver(Event::Activate(Activation::from_platform_data(
                &platform_data,
            )));
    }

    async fn open(&self, uris: Vec<String>, hint: String, platform_data: PlatformData) {
        self.inbox.deliver(Event::Open {
            uris,
            hint,
            activation: Activation::from_platform_data(&platform_data),
        });
    }

    /// A forwarded launch. The reply, its exit status, waits until the app is done with the
    /// [`CommandLine`]; what it prints meanwhile goes to the caller's `path` first, in order.
    async fn command_line(
        &self,
        path: OwnedObjectPath,
        arguments: Vec<Vec<u8>>,
        platform_data: PlatformData,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> i32 {
        let caller: Option<OwnedUniqueName> = header.sender().map(|s| s.to_owned().into());
        let (tx, mut rx) = unbounded_channel();
        let launch = Launch::from_wire(arguments, &platform_data);
        self.inbox
            .deliver(Event::CommandLine(CommandLine::remote(launch, tx)));
        while let Some(out) = rx.recv().await {
            let (method, text) = match out {
                Out::Exit(status) => return status,
                Out::Print(text) => ("Print", text),
                Out::PrintErr(text) => ("PrintError", text),
            };
            // Each line is answered before the next goes, so the caller sees them in order
            // and before the exit status. A caller that went away just misses them.
            let _ = conn
                .call_method(
                    caller.as_ref(),
                    &path,
                    Some(CALLBACK_INTERFACE),
                    method,
                    &(text,),
                )
                .await;
        }
        0
    }
}

pub(super) struct FreedesktopApplication {
    pub inbox: Arc<Inbox>,
}

#[interface(name = "org.freedesktop.Application")]
impl FreedesktopApplication {
    async fn activate(&self, platform_data: PlatformData) {
        self.inbox
            .deliver(Event::Activate(Activation::from_platform_data(
                &platform_data,
            )));
    }

    async fn open(&self, uris: Vec<String>, platform_data: PlatformData) {
        self.inbox.deliver(Event::Open {
            uris,
            hint: String::new(),
            activation: Activation::from_platform_data(&platform_data),
        });
    }

    async fn activate_action(
        &self,
        action_name: String,
        parameter: Vec<OwnedVariant>,
        platform_data: PlatformData,
    ) {
        self.inbox
            .deliver(action(action_name, parameter, &platform_data));
    }
}

/// GLib's action group interface. A GApplication forwarder lists the primary's actions first
/// (`DescribeAll`) and gives up if that fails, so the primary serves an empty list.
pub(super) struct GtkActions {
    pub inbox: Arc<Inbox>,
}

#[interface(name = "org.gtk.Actions")]
impl GtkActions {
    fn list(&self) -> Vec<String> {
        Vec::new()
    }

    fn describe(&self, action_name: String) -> fdo::Result<(bool, Signature, Vec<OwnedVariant>)> {
        Err(fdo::Error::InvalidArgs(format!(
            "the app exports no action {action_name:?}"
        )))
    }

    fn describe_all(&self) -> HashMap<String, (bool, Signature, Vec<OwnedVariant>)> {
        HashMap::new()
    }

    async fn activate(
        &self,
        action_name: String,
        parameter: Vec<OwnedVariant>,
        platform_data: PlatformData,
    ) {
        self.inbox
            .deliver(action(action_name, parameter, &platform_data));
    }
}

fn action(name: String, parameter: Vec<OwnedVariant>, platform_data: &PlatformData) -> Event {
    Event::Action {
        name,
        parameter: parameter.into_iter().next(),
        activation: Activation::from_platform_data(platform_data),
    }
}

pub(super) struct FileManager1 {
    pub inbox: Arc<Inbox>,
}

impl FileManager1 {
    fn reveal(&self, method: RevealMethod, uris: Vec<String>, startup_id: String) {
        self.inbox.deliver(Event::Reveal(Reveal {
            method,
            uris,
            startup_id,
        }));
    }
}

#[interface(name = "org.freedesktop.FileManager1")]
impl FileManager1 {
    pub(super) async fn show_folders(&self, uris: Vec<String>, startup_id: String) {
        self.reveal(RevealMethod::Folders, uris, startup_id);
    }

    pub(super) async fn show_items(&self, uris: Vec<String>, startup_id: String) {
        self.reveal(RevealMethod::Items, uris, startup_id);
    }

    pub(super) async fn show_item_properties(&self, uris: Vec<String>, startup_id: String) {
        self.reveal(RevealMethod::ItemProperties, uris, startup_id);
    }
}
