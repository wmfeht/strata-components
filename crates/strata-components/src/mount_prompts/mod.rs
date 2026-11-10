//! MountPrompts (C26): the dialogs a mount operation asks for, kept apart from GIO so they are
//! testable without it. Strata's replacement for `gtk::MountOperation`'s dialogs.
//!
//! Work package: C26, `docs/work/phase-2-composites/C26-mount-prompts.md`.
//!
//! Three C15 dialogs, each driven by a plain [`MountRequest`] and answered with a plain
//! [`MountResponse`]:
//!
//! - **ask-password** ([`PasswordRequest`]): Username, Domain, the password, "Connect as"
//!   Registered user or Anonymous, and "Password storage" (Don't remember, Until logout,
//!   Forever), each only when the request allows it. [`Secret::Passphrase`] is the variant for
//!   an SSH key's or a LUKS volume's passphrase: "Passphrase required", a Passphrase field, and
//!   the same storage choice.
//! - **ask-question** ([`QuestionRequest`]): the backend's message and its choices. With
//!   `host_key` it is Strata's "Verify server identity" dialog for an SSH host key.
//! - **show-processes** ([`ProcessesRequest`]): the processes keeping a volume busy, each with
//!   an End process button when the host takes [`MountPrompts::on_end_process`], and the
//!   choices. The backend sends it again as the list changes; the prompt updates in place.
//!
//! The host keeps a [`MountPromptState`] (one prompt at a time; queueing is the host's) and
//! builds [`MountPrompts`] over it. The component owns no I/O and knows no GIO.
//!
//! # The GIO binding lives in Strata
//!
//! **Decided by the owner (C26 planning): the `gio::MountOperation` binding lives in Strata,
//! not here.** strata-components depends only on rune, rune-kit and strata-kit: never on
//! strata-platform (`docs/work/README.md`, rule 1) or GIO. The binding is tangled with
//! Strata's policy (credentials typed in a URI answer the first ask unprompted, a second
//! ask-password is a retry, a declined host key changes the error, which questions are trust
//! questions), and `show-processes` needs `unsafe` to read its `GArray` of pids and `/proc` to
//! name them, neither of which belongs here.
//!
//! The binding, on P9's GLib bridge (`strata_platform::glib_bridge`):
//!
//! - **On the GLib thread**, each handler builds the request and awaits
//!   `Asker::ask(request)` in a `spawn_local` future, keeping its `LocalTask`:
//!   `connect_ask_password` (flags to the `needs_*`, `anonymous_allowed` and `saving_allowed`
//!   switches), `connect_local("ask-question", ..)` (gio-rs binds no typed handler; choices
//!   from the `StrV`; none at all: reply `Unhandled`), and `show-processes` (pids from the
//!   `GArray`, named by the host).
//! - **The answer:** [`MountResponse::Password`] sets `username`, `domain`, `password` (each
//!   when `Some`) and `password-save` from [`Remember`] (`Never`, `ForSession`,
//!   `Permanently`), then replies `Handled`; [`MountResponse::Anonymous`] sets `anonymous`;
//!   [`MountResponse::Choice`] sets `choice`; [`MountResponse::Abort`], or the request dropped
//!   unanswered (`ask` resolving to `None`), replies `Aborted`.
//! - **`aborted`**, and a newer `show-processes`, abort the pending `LocalTask`s: the bridge
//!   then marks their requests withdrawn and wakes the app, whose `poll` calls
//!   [`MountPromptState::withdraw`], so the prompt closes without a reply (GTK's
//!   `GtkMountOperation` does the same).
//! - **On Rune's loop**, the app's `poll` drains `Requests`, opens each with
//!   `state.open(request.question().clone(), request)` (answering any ticket it replaces with
//!   `respond(MountResponse::Abort)`), and `on_answer` calls `request.respond(answer)`.
//! - `show-unmount-progress` isn't a prompt: it goes to Strata's release overlay through the
//!   bridge's `listen`.
//!
//! `crates/strata-gallery/tests/c26.rs` holds a test-only reference binding that round-trips a
//! fake `MountOperation`'s `ask-password`, `ask-question` and `aborted` through the real bridge
//! into these prompts. `show-processes` can't be emitted from safe Rust (its `GArray`), so its
//! mapping is covered as plain data only.
//!
//! # Keyboard and focus
//!
//! C15's rules hold (Escape and the close button answer [`MountResponse::Abort`]; Enter in a
//! field presses Connect). Past them:
//!
//! - The password prompt opens on Username when it shows empty, else on the secret field, else
//!   on Connect (Strata's `focus_first`). Backdrop clicks are ignored once a field holds text,
//!   a default username included, as in Strata.
//! - Choosing Anonymous disables the fields, the show/hide toggle and Password storage.
//! - A question or a process list ignores backdrop clicks, has **no default button**
//!   (`StrataDialog::default_button(false)`), so Return with nothing focused never accepts a
//!   host key or ends processes, and opens on its first declining choice (the secondary).
//!   Choices sit as Strata orders them: declining ones first; each answers with its index in
//!   the request. GIO doesn't say which choice declines, so the host marks it
//!   ([`Choice::rejects`]; [`looks_like_rejection`] is Strata's English heuristic).
//! - The show/hide toggle is no Tab stop of its own, as GTK's peek icon isn't.
//!
//! # What assistive technology hears
//!
//! The dialog, named by its title and described by its subtitle; the message; a refusal, as a
//! polite live region (each one a new node, so a second is heard too); Username and Domain as
//! text fields and the secret as a `password-input` whose value is never exposed, each named by
//! its visible label; "Connect as" and "Password storage" as radio groups; the process list as
//! a list of items named by program, described by pid and command line; the buttons, named by
//! their choices.
//!
//! # Deviations
//!
//! - **D2** No TCRYPT options (PIM, hidden or system volume), as Strata: GTK's own dialog had
//!   them.
//! - **D3** The password prompt's icon is the lock: strata-kit has no key icon yet (an
//!   `IconName::Key` addition is a K3/W2 follow-up).
//! - **D5** Every question and process list uses these dialogs; Strata left non-SFTP questions
//!   and the busy dialog to GTK's.
//! - **D6** The host marks declining choices.
//! - **D7** Credentials carry only the fields the request asked for; Strata always set the
//!   username.
//! - **D8** Process names come from the host; End process is an intent the host acts on.
//! - **D9** The fields are the core `text_input` in C18's frame, not rune-kit's `TextInput`,
//!   which takes no right-click handler (Rune draft U-C21-2).
//!
//! Rune dev 5d421fd: **partly**. It covers secure single-line fields (role `password-input`,
//! the value never exposed, no copy, cut, undo or input method; `El::revealed` to show it),
//! modal dialogs and radio groups. It lacks focusing a button from code (#173; C15's
//! `tab_index` order stands in; `Tx::focus` lands past the pin), a rune-kit text field that
//! takes a right-click menu (U-C21-2), and a GLib loop (P9).

mod labels;
mod request;
mod state;
mod view;

#[cfg(test)]
mod tests;

pub use labels::MountLabels;
pub use request::{
    BlockingProcess, Choice, Credentials, MountRequest, MountResponse, PasswordRequest,
    ProcessesRequest, PromptField, QuestionRequest, Remember, Secret, looks_like_rejection,
};
pub use state::{MountPromptState, PasswordForm};
pub use view::{MountPrompts, PASSWORD_WIDTH};
