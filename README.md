# strata-components

The component library that moves [Strata](https://github.com/lgse/strata), a Linux file
manager, off GTK 4 and onto [Rune](https://github.com/anomalyco/rune).

> **Status: early, pre-release.** The workspace scaffold, the component and look contracts,
> and the phase 0 spike records have landed. Components and platform shims are being built
> now. Progress is tracked in [`docs/work/STATUS.md`](docs/work/STATUS.md) on the
> `integration` branch.

## What's in it

| Crate | Tier | What it holds |
| --- | --- | --- |
| [`strata-platform`](crates/strata-platform) | 1 | Linux shims that Rune lacks today: drag and drop, clipboard MIME types, client-side decorations, foreign parenting, desktop settings, icon themes, single instance, a GLib bridge. Generic to any Rune app. Each shim sits behind its own cargo feature. |
| [`strata-kit`](crates/strata-kit) | 3 | The look: Strata's themes mapped onto `rune_kit::Kit`, density and type tokens, icon catalogs, and user overrides. |
| [`strata-components`](crates/strata-components) | 2 | File-manager components built on rune, rune-kit and strata-kit: the entry collection and its Columns, Icons and List views, location bar, tabs, sidebar, preview surfaces, dialogs, progress dock and settings controls. |
| [`strata-gallery`](crates/strata-gallery) | — | One headless scene per component state, and the golden tests. |

Components take their state by accessor and own no I/O: listings, thumbnails and search
results arrive as data. They read only `Kit` tokens and strata-kit extensions, never a theme
by name.

## Getting started

The toolchain is pinned with [mise](https://mise.jdx.dev). Rune is pre-release and not on
crates.io; every Rune crate is pinned to one `dev` rev in the root `Cargo.toml`.

```sh
mise install            # the pinned Rust, cargo-deny and typos
mise run check          # fmt, compile, clippy, tests, deny and typos
mise run gallery        # open the component gallery
mise run gallery -- --snap out.png --scene gallery-index-dark   # render a scene headless
```

On Linux, building Rune needs the Wayland and xkbcommon development headers
(`libwayland-dev` and `libxkbcommon-dev` on Debian and Ubuntu).

## Testing

Tests run headless on Rune's virtual clock and never touch the desktop display. Each
component ships with light and dark gallery scenes, accessibility and PNG goldens, and
scripted input checks that name the feature-map behaviours they cover.

```sh
mise run test [filter]  # e.g. mise run test c1::
mise run bless <filter> # rewrite one page's goldens, then review the diff
```

See [`docs/testing.md`](docs/testing.md) for the gallery pages, golden folders, script
format and bless filters.

## Documents

- [`docs/plan.md`](docs/plan.md): the plan. It maps every Strata area to a component and
  sets the phases.
- [`docs/work/`](docs/work/README.md): the plan split into work packages, with their
  dependencies and milestones.
- [`docs/decisions/`](docs/decisions): spike records on Linux windowing, data transfer
  beside winit, and foreign parenting.
- [`AGENTS.md`](AGENTS.md): the repository rules for contributors and coding agents.

## Contributing

Rune takes no pull requests. When Rune lacks something, the fix is an upstream issue with a
headless repro, or a thin shim in `strata-platform` that names that issue and goes away once
Rune ships the feature. Read [`AGENTS.md`](AGENTS.md) before opening a change.

## License

[MIT](LICENSE).
