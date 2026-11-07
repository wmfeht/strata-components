# strata-platform

Linux platform shims for [Rune](https://github.com/anomalyco/rune) apps: the Wayland and X11
pieces Rune doesn't ship yet. Any Rune app on Linux can use it. Of Rune's crates it depends on
`rune` only, and each shim adds optional dependencies of its own. It has no app-specific names in
its API.

Each shim is a module behind its own cargo feature, so an app builds only what it uses. Most are
asked of Rune upstream, and a shim is deprecated once Rune ships its feature (see
[Shim lifecycle](#shim-lifecycle)). Two aren't: icon theme lookup has no upstream ask proposed,
and the GLib bridge is outside Rune's scope.

The crate's name is still under review (`docs/decisions/S5-upstream.md`, owner questions). A
rename, if any, happens before the first release tag.

## Features

None is on by default. `glib-bridge` is implemented; the other modules are placeholders until
their work packages land. "Issue" is the upstream ask, drafted in
[`docs/decisions/S5-upstream.md`](../../docs/decisions/S5-upstream.md#draft-index), with its link
once filed.

| Feature | Module | What Rune lacks on Linux | Issue |
| --- | --- | --- | --- |
| `dnd` | `dnd` | System drag and drop: `drag_out` is macOS-only, and Wayland drops reach no handler. Neither takes MIME types beyond file paths. | [U1](../../docs/decisions/S5-upstream.md#u1), [U2](../../docs/decisions/S5-upstream.md#u2), [U9](../../docs/decisions/S5-upstream.md#u9), [U10](../../docs/decisions/S5-upstream.md#u10); upstream [#171](https://github.com/anomalyco/rune/issues/171) |
| `clipboard` | `clipboard` | Custom MIME types, several forms in one write, typed reads, and a notice when another app takes the clipboard | [U5](../../docs/decisions/S5-upstream.md#u5), [U9](../../docs/decisions/S5-upstream.md#u9) |
| `csd` | `csd` | Interactive resize, minimise and the window menu for a borderless window | [U6](../../docs/decisions/S5-upstream.md#u6), [U7](../../docs/decisions/S5-upstream.md#u7), [U8](../../docs/decisions/S5-upstream.md#u8), [U9](../../docs/decisions/S5-upstream.md#u9); upstream [#145](https://github.com/anomalyco/rune/issues/145) |
| `foreign-parent` | `foreign_parent` | Making a window transient for another app's window | [U11](../../docs/decisions/S5-upstream.md#u11), [U9](../../docs/decisions/S5-upstream.md#u9) |
| `settings` | `settings` | Settings portal keys beyond `color-scheme`, and the desktop's double-click interval | [U12](../../docs/decisions/S5-upstream.md#u12), [U13](../../docs/decisions/S5-upstream.md#u13) |
| `icon-theme` | `icon_theme` | Freedesktop icon theme lookup | None proposed |
| `media-audio` | `media` | An audio sink with a playback clock | Related: upstream [#30](https://github.com/anomalyco/rune/issues/30) |
| `single-instance` | `single_instance` | Single instance and activation over D-Bus | Upstream [#22](https://github.com/anomalyco/rune/issues/22) |
| `glib-bridge` | `glib_bridge` | A GLib main loop: Rune never iterates a `GMainContext`, so GIO's async calls, signals and monitors never dispatch. Implemented: `GlibThread` runs one on its own thread, and answers land in app state for `poll`. Needs GLib and GIO (`libglib2.0-dev` to build). | None: outside Rune's scope |

Each module's own docs say what Rune lacks in detail, and the limits of the shim.

## Supported Rune revs

Each release tag supports exactly one Rune rev. Pin that rev, in full, beside the tag.

| Tag | Rune rev | Rust | Released |
| --- | --- | --- | --- |
| Unreleased (`main`) | [`78d22ed8eef60d76ac3f322ffc5883f3fcbc57f5`](https://github.com/anomalyco/rune/commit/78d22ed8eef60d76ac3f322ffc5883f3fcbc57f5) | 1.99.1 | — |

## Pinning a release

Rune isn't on crates.io, so neither is this crate: crates.io takes no git dependencies. Releases
are git tags named `strata-platform-v<version>+rune.<rev>`, where `<rev>` is the first seven
digits of the Rune rev the release supports. Pin the tag, and pin Rune at that same rev:

```toml
[dependencies]
rune = { git = "https://github.com/anomalyco/rune", rev = "78d22ed8eef60d76ac3f322ffc5883f3fcbc57f5" }
strata-platform = { git = "https://github.com/wmfeht/strata-components", tag = "strata-platform-v0.1.0+rune.78d22ed", features = ["glib-bridge"] }
```

The tag shown is an example; take a real one from [Supported Rune revs](#supported-rune-revs).

- **Copy the rev exactly, all 40 digits.** Cargo treats `rev = "78d22ed"` and the full rev as two
  different sources. It then builds Rune twice, and this crate's Rune types don't match your
  app's. `cargo tree -e normal --prefix none --format '{p}' --color never | grep '^rune v' | sort -u`
  must print one Rune, ignoring the ` (*)` cargo adds to repeats.
- Your features of `rune` are your own: cargo merges them with this crate's.
- Write the tag with a plain `+` in `Cargo.toml`. Only web links write it as `%2B`.
- To move to a new Rune rev, move both pins together, to a tag that names the new rev.

## Versioning

The crate follows semver. Moving to a new Rune rev is a minor release, because Rune's types
appear in the API. The [changelog](../../CHANGELOG.md) lists every release, with the Rune rev and
Rust version it supports.

## Shim lifecycle

When Rune ships a feature that a module shims:

1. The release that moves to that Rune rev deprecates the replaced items, naming the Rune API to
   use instead. The module docs and the changelog say the same.
2. The next minor release removes them, or the whole module and its feature if Rune replaces
   all of it.

If Rune ships only part of a feature, only the replaced items go. The full checklist is in
[`docs/decisions/S5-upstream.md`](../../docs/decisions/S5-upstream.md#shim-deprecation-checklist).

## License

MIT, like Rune.
