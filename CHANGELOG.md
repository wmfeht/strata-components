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
  Apart from `glib-bridge`, the modules are placeholders until their work packages land.
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
