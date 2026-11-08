# Changelog

This lists the changes to `strata-platform`, the one crate this repository publishes. The other
crates are internal (`publish = false`) and aren't listed.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Each release is a git tag,
`strata-platform-v<version>+rune.<rev>`, where `<rev>` is the first seven digits of the Rune rev
it supports. Moving to a new Rune rev is a minor release, because Rune's types appear in the
crate's API. How to pin a release is in the crate's
[README](crates/strata-platform/README.md#pinning-a-release), and the release rules are in
[`docs/decisions/S5-upstream.md`](docs/decisions/S5-upstream.md#release-workflow).

Each release section starts with a line naming its Rune rev in full and its Rust version, then
lists its changes under Added, Changed, Deprecated, Removed and Fixed. A Rune rev move goes under
Changed, as "Rune `old` → `new`", with the upstream changes that matter to the shims. A shim
deprecated because Rune ships its feature goes under Deprecated with its Rune replacement, and is
removed in the next minor release.

## [Unreleased]

Rune: [`78d22ed`](https://github.com/anomalyco/rune/commit/78d22ed8eef60d76ac3f322ffc5883f3fcbc57f5)
(`78d22ed8eef60d76ac3f322ffc5883f3fcbc57f5`) · Rust 1.99.1

Nothing is released yet. The first release is planned for milestone M1 with the `csd`,
`settings`, `single-instance` and `glib-bridge` features.

### Added

- One cargo feature per shim, each gating its own module: `dnd`, `clipboard`, `csd`,
  `foreign-parent`, `settings`, `icon-theme`, `media-audio`, `single-instance` and `glib-bridge`.
  Apart from `glib-bridge`, `csd` and `single-instance`, the modules are placeholders until
  their work packages land.
- `glib-bridge`: a GLib main loop for Rune apps. `GlibThread::start` runs a fresh `MainContext`
  on its own thread and wakes the app's `poll` as answers land in its state.
  - `Glib::spawn` runs a GIO future there, and its output lands in a `Task`.
  - `Glib::listen` connects GObject signals and queues their arguments in a `Signal`.
    `Glib::channel` and `Glib::attach` are its two halves, for several sources feeding one queue.
  - `Glib::requests` carries a GLib-side question the app must answer from `poll`, such as a
    mount operation's password prompt (`Asker::ask`, `Request`).
  - `spawn_local` tracks GLib-side futures, so dropping or shutting down the `GlibThread` stops
    them on its own thread.

  It needs GLib and GIO; building it needs `libglib2.0-dev` or your distribution's equivalent.
- `csd`: client-side decorations. `Policy::Auto` opens a borderless window on Wayland and X11;
  `Policy::Native` keeps the native frame. The app keeps a `Csd` in its state.
  - `csd::header` makes the app's header move the window and maximise it on a double press, and
    opens the window menu where the window manager has one.
  - `csd::decorate` adds resize bands over the window's edges where they can work.
  - `Csd::close`, `Csd::minimize` and `Csd::toggle_maximized` are the window buttons' handlers.
    Each request answers an `Outcome`, naming the Rune issue draft behind a gap.
  - `csd::attach`, from `App::on_window`, links the window. On X11 it opens its own connection
    for `_NET_WM_MOVERESIZE`, `WM_CHANGE_STATE` and `_GTK_SHOW_WINDOW_MENU`, and watches the
    window's maximised and full-screen state and the window manager's capabilities.

  On Wayland a borderless window has no edge resize, minimise or window menu until Rune ships
  them (drafts U6, U7 and U8). On X11, resizing may fail where the window manager grabs the
  pointer itself (mutter and KWin are untested). It uses `x11rb`.
- `single-instance`: one primary process per app id, as `GApplication` gives GTK apps.
  `single_instance::claim` claims the id on the session bus first thing in `main`. A later
  launch forwards its arguments (as bytes), working directory and activation token to the
  primary, prints what the primary sends back and exits with its status (`Claim::Forwarded`).
  - The primary gets each launch, activation and call as an `Event` on Rune's loop
    (`Primary::attach_rune`, through `AppHandle::update`), its own launch first.
    `CommandLine::print`, `print_err` and `exit` answer a forwarded launch.
  - It speaks GApplication's wire protocol (`org.gtk.Application`, `org.freedesktop.Application`,
    `org.gtk.Actions`), so a GApplication of the same id and this primary reach each other.
  - `Options::file_manager1` also serves `org.freedesktop.FileManager1`, delivering its calls
    as `Reveal` data; the name is queued for, never taken from another file manager.
  - Without a session bus, or with one that doesn't answer within `Options::bus_timeout`, a
    launch runs alone (`Claim::Standalone`).

  zbus types appear in its API, so a move to another zbus major is a minor release. It uses
  `zbus` and `tokio`, which Rune's `dbus` feature already builds.
