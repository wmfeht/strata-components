# Agent Instructions

strata-components is the component library that moves Strata (`lgse/strata`) off GTK 4 and
onto [Rune](https://github.com/anomalyco/rune). Read `docs/plan.md` before you start: it maps
every Strata area to a component (P1–P9, C1–C26, K1–K6) and sets the phases. `docs/work/`
splits the components into work packages (S1–S5, W0–W4, X1–X3 and the component parts, such as
C1a–C1f) and `docs/work/STATUS.md` tracks them.

## Explicit owner consent

Every rule here is a default. The repository owner can override any of them with an explicit
instruction for the current task. A direct request is enough, so act on it without asking again.
Record each override, and every check you skipped, in the handoff. Never report a skipped check
as passed.

## Layout

| Path | What lives there |
| --- | --- |
| `crates/strata-platform` | Tier 1: Linux shims Rune lacks (Wayland and X11). A published, generic crate. Its `examples/` are small Rune apps and the session tests' probes. |
| `crates/strata-platform/tests/session/`, `tests/support/` | The compositor tests (W3): each in a private headless sway, Xwayland and session bus |
| `crates/strata-platform/tests/support/strata-test-peer` | The compositor tests' peer process: test tooling, never published |
| `crates/strata-kit` | Tier 3: theme mapping onto `rune_kit::Kit`, icon catalogs, density, text size, overrides, the bundled JetBrains Mono |
| `crates/strata-components` | Tier 2 and kit-extension widgets, on rune + rune-kit + strata-kit |
| `crates/strata-gallery` | One headless scene per component state, plus the golden tests; `examples/` holds the X1–X3 demos (`shell`, `dnd_wiring`, `clipboard_wiring`) and `media_playback` |
| `tests/golden/` | PNG and accessibility-tree goldens per gallery page, and the tree each script ends on |
| `tests/scripts/` | Rune headless input scripts, one per behavior group |
| `tests/fixtures/` | Shared test inputs (S4's code fixtures) |
| `docs/` | The plan and lasting design and testing notes (`testing.md`) |
| `docs/decisions/` | The spike records S1–S5, with the owner's rulings |
| `docs/work/` | The work packages and their status |
| `.github/workflows/` | CI (`ci.yml`), the compositor tests (`platform.yml`) and `strata-platform`'s release (`release.yml`) |

## Crate rules

- `strata-platform` depends on `rune` only: never on rune-kit, strata-kit or Strata. Its API
  carries no Strata names. Each shim is a module behind its own cargo feature, and its docs link
  the Rune issue and say what Rune lacks. It follows semver, and each Rune rev bump is a minor
  release.
- Components take state by accessor (`fn(&S) -> &X`, `fn(&mut S) -> &mut X`), as rune-kit's
  Table and Tree do. They own no I/O: listings, thumbnails and search results arrive as data.
- Components read only `Kit` tokens and strata-kit extensions, never a theme by name, and never
  hardcoded colours.
- Strata's file-manager behavior stays in Strata. A component holds the interaction model and
  placement, not the policy.

## Rune

- Rune is pre-release and takes no pull requests. When Rune core lacks something, file an issue
  upstream with a headless repro, or add a thin shim in `strata-platform` that names the issue.
  Never vendor or patch Rune.
- Every Rune crate pins the same `dev` rev in the root `Cargo.toml`. Move the rev on purpose in
  its own change, and update `docs/plan.md` §0 and the Rust version in `mise.toml` (Rune's
  `rust-toolchain.toml` at that rev) with it.
- Rune's API reference is `crates/rune/README.md` and `crates/rune-kit/README.md` at the pinned
  rev. Read them there, not from memory.

## Tooling

mise pins the toolchain (`mise.toml`, `mise.lock`). Run `mise install` once, then use the tasks:

| Task | Runs |
| --- | --- |
| `mise run fmt` | `cargo fmt --all --check` |
| `mise run compile` | `cargo check` on every target and feature |
| `mise run compile-features` | `cargo check` of `strata-platform` with no features, then with each feature alone |
| `mise run clippy` | Clippy with `-D warnings` |
| `mise run test [filter]` | The tests, with `DISPLAY` and `WAYLAND_SOCKET` unset and `WAYLAND_DISPLAY` naming a socket that doesn't exist |
| `mise run test-platform [args]` | The compositor tests (`#[ignore]`d in `cargo test`): strata-platform's session tests and the gallery's `x2` and `x3`, each in a private headless sway session. Args go to libtest, e.g. `p4::` |
| `mise run bless [filter]` | The tests with `RUNE_BLESS=1`, which rewrites the goldens: all, or one page (`c1`), scene (`c1-empty-folder-dark`) or script (`c5/stem-selection`) |
| `mise run deny` | cargo-deny: advisories, licenses, bans, sources |
| `mise run typos` | Spelling |
| `mise run check` | `fmt`, `compile`, `compile-features`, `clippy`, `test`, `deny` and `typos`; run it before you push |
| `mise run gallery [args]` | The gallery app, e.g. `-- --snap out.png --scene gallery-index-dark`. Clear the display first, as `mise run test` does |

Add a dependency only if its license is in `deny.toml`'s allow list. Don't widen the list or add
an advisory ignore without a stated reason.

## Testing

- Tests run headless on Rune's virtual clock (`rune::Runtime`). They never use the desktop
  display. Rune's `--snap`, `--tree` and `--script` tools work the same way. The compositor
  tests (`mise run test-platform`) run in a private headless sway, never the desktop's
  (`docs/testing.md`).
- Each component ships with a gallery scene for each state (light and dark), an accessibility
  golden, and a headless script for each feature-map Behavior bullet it implements. Scripts name
  the bullets they check (`featuremap.py qa --node <id>` lists them).
- Goldens change only on purpose: run `mise run bless`, then review every golden diff before you
  commit it.
- Test observable behavior: input routing, selection, focus, lifecycle and what assistive
  technology hears. Don't test layout geometry, spacing or pixel placement with assertions; PNG
  goldens and screenshots cover the look.
- Don't add tests that echo constants or setters, or that repeat an existing case with an axis
  that changes nothing.
- Put unit tests in an adjacent `tests.rs` module, declared with `#[cfg(test)] mod tests;`, not
  inline. Integration tests go in the crate's `tests/` directory.
- Validation is risk-based. For a bounded change, run the affected tests (`mise run test
  <filter>`) and confirm the filter matched at least one test. Run `mise run check` before
  pushing, and the full suite when the change touches shared code, dependencies or the Rune rev.

## Git workflow

- Never commit directly to `main`. Work from an issue on a branch named
  `<type>/<issue-number>-<short-kebab-description>`.
- Commits and PR titles use Conventional Commits: `<type>(optional-scope): <imperative
  description>`, with types such as `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `build`,
  `ci` and `chore`. Use a scope for the crate or component where it helps (`feat(c1): …`).
- One conversation, one PR, unless the owner asks for more.
- Keep PR descriptions short: what changed and why, how to exercise it, and the linked issue.
  Never commit screenshots, logs or review notes. Goldens are the exception.

## Comments

- Prefer clear names and structure. Don't narrate obvious code.
- Comment non-obvious intent, invariants, Rune workarounds (with the issue link) and external
  constraints.
