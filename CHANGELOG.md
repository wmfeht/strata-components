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

Rune: [`5d421fd`](https://github.com/anomalyco/rune/commit/5d421fd908ba19b760b93a3dcf821e159ec1d4d7)
(`5d421fd908ba19b760b93a3dcf821e159ec1d4d7`) · Rust 1.99.1

Nothing is released yet. The first release is planned for milestone M1 with the `csd`,
`settings`, `single-instance` and `glib-bridge` features.

### Changed

- Rune `78d22ed` → `5d421fd` (`dev`, 2026-11-08), on the same Rust. What matters to the shims:
  - A covered Wayland window's due `wake_after` timer now runs, at most 250 ms late, instead of
    the loop spinning at full CPU and running no `poll` while it waits for a frame the
    compositor never sends (Rune `39301f6`, #324). A `glib-bridge` answer still runs `poll`
    only when the window draws, so while a Wayland window is fully covered it waits for the
    window to show or for the app's next timer (Rune #139). `settings` and the transfer
    substrate post through `AppHandle` and land while covered, as before.
  - `csd`: a right-click on a text field in the header no longer opens the window menu. Rune
    routes a field's secondary click to its own field menu, which Linux can't show, so without
    the field's own `on_secondary_press` it focuses the field, moves its caret and shows
    nothing.
  - Rune now opens fontconfig at run time for installed font families, and adds
    `yeslogic-fontconfig-sys` (MIT) and a second `roxmltree` (MIT or Apache-2.0) to the build.

### Added

- One cargo feature per shim, each gating its own module: `dnd`, `clipboard`, `csd`,
  `foreign-parent`, `settings`, `icon-theme`, `media-audio`, `single-instance` and `glib-bridge`.
  Apart from `dnd`, `clipboard`, `glib-bridge`, `csd`, `settings` and `single-instance`, the
  modules are placeholders until their work packages land.
- `dnd`: system drag and drop on Wayland (`wl_data_device`) and X11 (XDND 5), mirroring Rune's
  `drag_out` and `on_drop_files` with any MIME types. Each window has a `Dnd`, attached from
  `App::on_window`.
  - `Dnd::drag_source` with a `DragSource`: a `DragItem` of MIME types made lazily, the actions
    it allows (narrowed by Ctrl and Shift), an icon (`DragIcon::image`, or `DragIcon::snapshot`
    of a view, rendered off the UI thread), and the outcome in `on_end`. `Dnd::start_drag` and
    `Dnd::owns` start one from an element's own `on_drag`; `Dnd::cancel_drag` cancels it.
  - `Dnd::drop_target` with a `DropTarget`: the types it reads (the first offered, or all),
    hover, move (in the target's points) and reject hooks, reads while hovering (`peek`), the
    action by GTK's modifier rule unless the target's `action` picks it, and asks answered
    after the drop (`on_ask`).
  - `DragItem::local` and `DropTarget::local`: a typed payload between the app's own windows,
    with nothing serialized.
  - On Wayland, Rune's capture of the press ends once the compositor has taken the pointer
    (a synthetic Escape, a workaround for draft U10), and only while the pressed element is
    still mounted where it was. A drag no proof reaches keeps the capture until the
    window's next press or focus change. Keep a drag's source mounted at the same path,
    with its `on_drag`, until the drag ends, and don't open a modal as it starts: the module
    docs list every path where the capture stays held or an Escape reaches the app. On X11
    a release before the drag's XI2 events were selected still ends the drag.
  - Every Wayland drag lists `application/x-local-drag;id=<counter>.<nonce>` (no process
    id), which other clients see; on X11 only a drag with a local payload does.
  - `Dnd::start_drag` takes the phase and the element's key.
  - Call `dnd::shutdown()` before `Tx::quit` or `AppHandle::quit`, in `App::on_quit`, and in
    the last window's `on_close` (`on_quit` doesn't run in a last window that is closing). A drag from another app takes the compositor's action. X11's
    source shows no cursor feedback. Link is X11 only.
- `clipboard`: copy and paste with any MIME types on Wayland and X11, beside Rune's own
  text clipboard. A `Clipboard` per state type, attached from `App::on_window`.
  - `Clipboard::copy` writes a `Content` of any types at once, each with bytes or a lazy
    writer (run once per copy, on a worker), images too (`Content::png`, `Content::image`),
    and text under every name it goes by (`Content::text`, `TEXT_TYPES`).
  - `Clipboard::offered` lists what a selection offers; `Clipboard::read` reads the first of
    several types off the UI thread with `Limits`, as `Tx::read_clipboard` does; a read of
    the app's own copy is served in process.
  - `on_change` hears another app's copy, the selection emptied, and each copy holding and
    losing it (`Change::Lost` with why: taken, replaced, cleared, refused by the compositor,
    failed). `Clipboard::clear` empties a selection only while the copy still holds it.
  - `Files` writes and reads `text/uri-list`, `x-special/gnome-copied-files` and
    `application/x-kde-cutselection` as GTK does, with the cut flag.
  - Routes: Wayland data control (hears unfocused), else `wl_data_device` on mutter (hears
    another app's copy at the next focus, writes with the triggering event's serial), else X11
    selections through Xwayland when Wayland has no route (libwayland < 1.23 on mutter); X11
    selections on X11. The primary selection needs data control or X11.
  - On X11, `clipboard::shutdown()` hands a copy that still holds the clipboard to a running
    clipboard manager, every type of it, waiting at most 2 s (`Content::keep_after_exit`);
    `Closed::hand_off` says how it went. On Wayland a copy goes with the app.
  - Call `clipboard::shutdown()` (or `dnd::shutdown()`: either does both) in `App::on_quit` and
    in the last window's `on_close`, as drag and drop's.
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
    as `Reveal` data; the name is queued for, never taken from another file manager. Asking for
    it is best effort: `Primary::file_manager1` says whether it was owned, queued or refused.
  - `Options::actions` names the actions `ActivateAction` may run; others are refused with
    `InvalidArgs`, as a GApplication refuses them.
  - Without a session bus, or with one that doesn't answer within `Options::bus_timeout`, a
    launch runs alone (`Claim::Standalone`).
  - `Primary::release` gives the app id up when the app quits. Launches it has answered still
    get their output and status; those it hasn't claim the app id again.

  zbus types appear in its API, so a move to another zbus major is a minor release. It uses
  `zbus` and `tokio`, which Rune's `dbus` feature already builds.
- `settings`: the desktop's settings from the XDG Settings portal. `Watcher::start` reads them
  on a thread of its own and follows `SettingChanged` and the portal's restarts, with no polling;
  `settings::follow` lands each change in the app's state through `AppHandle::update_if`.
  - `DesktopSettings` holds the text scale, double-click interval, drag threshold, caret blink
    (on, time and timeout), GNOME's animations switch, the portal's `reduced-motion` and the
    recent-files switch, each with its default until the desktop gives a value
    (`DesktopSettings::is_known`).
  - Events arrive in order: one `Event::Snapshot`, then an `Event::Changed` per change, and an
    `Event::Fallback` the first time each fallback happens (no bus, no portal, an unset key, an
    unusable value, the portal leaving). The module never prints.
  - A portal slower than `Options::timeout` gets the defaults at the timeout, and its answer as
    changes when it lands. A restarted portal is read again; a key it lacks goes back to its
    default.

  The portal is the only source: without one (an X11 session with only XSETTINGS, say) the
  values are the defaults. Rune's own double press keeps its fixed 0.5 s (draft U13). It uses
  `zbus` and `tokio`, as Rune's `dbus` feature does.
