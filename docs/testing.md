# Testing

How the gallery, its goldens and its scripts fit together, and how a work package adds to
them without touching another's files. The plan's testing rules are in
[`plan.md` §6](plan.md#6-testing). The repository rules are in [`AGENTS.md`](../AGENTS.md).

## Gallery pages

The gallery (`crates/strata-gallery`) has an index and one page per component, plus a page
for each contract or look work package that shows something (W1, W2, K1–K4, C10a). The
registry in `src/scenes/mod.rs` declares every page. A work package edits only its own
`src/scenes/<id>.rs`, which provides six items:

| Item | What it is |
| --- | --- |
| `Demo` | The page's demo state: a `Default` type, held in `Gallery::<id>` |
| `SCENES` | Golden scenes: each gets an accessibility golden and a PNG golden |
| `REVIEW` | Scenes for review with `--scene` only, kept out of the goldens |
| `view` | The page's view, `fn(&Gallery, &mut Cx<Gallery>) -> El` |
| `on_key` | `AppSpec::on_key` for the page: keys no focused element took. True consumes the key. |
| `poll` | `AppSpec::poll` for the page: runs every frame while it is open. True when state changed. |

Page ids are the component ids (`c1`, `c14a`, `c20b`) or the work package ids (`w2`, `k3`,
`c10a`). The index's id is `gallery`.

Scene rules, which `tests/goldens/registry.rs` enforces:

- A scene name starts with its page id and a dash, and ends in `-light` or `-dark`:
  `c1-ten-rows-light`.
- Every `-light` scene has a `-dark` twin.
- Names are unique across the gallery.
- A scene opens its own page in its own theme: it calls `g.open("c1", dark)`, then sets its
  demo state.

Fake data is generated in the page file or in the component's own test fixtures. Pages never
wait on Strata's workers or on platform shims.

### Keys, polls and async results

The app's hooks (`strata_gallery::spec`) route to the open page, in a window and in headless
`--snap` and `--script` runs alike:

- **Keys.** An element's own `on_key` hears keys only while it has focus. Everything else
  (shortcuts such as Ctrl+L, and keys with nothing focused) reaches the page's `on_key`
  first. Only a key it leaves falls back to the gallery: Escape on a page goes back to the
  index. So a page that uses Escape (cancel a marquee, stop an autoscroll) returns true for
  it.
- **Polls.** The open page's `poll` runs at the start of every frame, with the clock in
  seconds: virtual in tests and headless runs. Use it to deliver fake async results
  (completions, highlighted lines, thumbnails, progress), and return true when state changed.
- **Waker.** `Gallery::waker` wakes the app loop from another thread so the page's `poll` runs.
  In tests it is a no-op.

Drive fake async from `poll` on the virtual clock: a fake worker is a queue of results, each
due at a clock time, that `poll` hands over once the clock reaches it. Goldens and scripts then
see the same results every run. A real thread (with `Gallery::waker`) is for windowed demos
only; its results land at times no golden or script can rely on.

Golden scenes are captured at 1 s on the virtual clock (`AT` in `tests/goldens/support.rs`),
late enough for entrance motion to come to rest. Both goldens run the same frames: the PNG
check passes `--snap --at 1`, and the in-process mount (`strata_gallery::runtime`) runs a
first frame at 0 s, then one every 1/60 s up to 1 s, as `--snap` does. So the page's `poll` runs
61 times at the same clock times for both, and
`png::the_png_render_ends_on_the_mounts_clock` checks the two clocks agree. A golden scene
either pre-fills its state or schedules results that come due before 1 s.

Test a page's behavior in process in `crates/strata-gallery/tests/<id>.rs`, with the tests
inside `mod <id> { … }`, so `mise run test <id>::` runs them with the page's goldens. Mount a
scene with `strata_gallery::runtime`, send keys with `Runtime::key` or pointer input, step
`Runtime::frame`, and read the state; or write a script. `tests/goldens/keys.rs` shows the
key routing itself: a page hook that takes Escape keeps the page open.

### The Look

W2 replaces `Gallery::kit` with the Look contract. Its PR is the one sanctioned edit to
strata-gallery's `src/lib.rs`: it adds a `look: Look` field with the accessor components take
(`fn(&Gallery) -> &Look`), and makes `Gallery::open` rebuild it for `dark`. From then on, K1, K2
and K4 change `g.look` (a theme, density, text size or override) only from their own
`scenes/<id>.rs`.

Open the gallery with `mise run gallery`. A row on the index opens its page, and Escape
returns to the index. `mise run gallery -- --scene c1-ten-rows-dark` opens one scene, and
`-- --list-scenes` lists them all.

## Goldens

```
tests/golden/a11y/<id>/<scene>.json      the accessibility tree, stripped (below)
tests/golden/png/<id>/<scene>.png        the scene rendered headless at --scale 1, 960×640
tests/golden/scripts/<id>/<script>.json  the accessibility tree a script ends on, stripped
tests/scripts/<id>/<script>.json         the scripts
```

Stripped trees leave out bounds, scroll offsets, text selection and shortcuts: layout and
per-platform spelling, not semantics.

Each page gets three tests in `crates/strata-gallery/tests/goldens/`: `<id>::a11y`,
`<id>::png` and `<id>::scripts`. A page with no scenes or scripts passes them trivially.

- A golden with no scene or script behind it fails the test. Rename or remove a scene, then
  bless the page, and the stale golden is deleted. A directory inside a page's golden folder
  also fails.
- A failure lists every differing item with the golden's path and a fresh copy under
  `target/tmp/goldens/<kind>/<id>/`. For a PNG it also writes `<scene>-diff.png`: each pixel's
  largest channel difference, times 4, in red.

### PNG tolerance and the renderer

A pixel is off when its largest channel differs by more than 24/255. That is the "visible"
threshold of rune-kit's gallery diff (`scripts/kit-diff.py` at the pinned Rune rev). No off
pixel is allowed (`MAX_OFF_PIXELS = 0` in `tests/goldens/png.rs`). Blessing keeps a PNG golden
that still matches within the tolerance, so a render on another machine doesn't churn it.

PNG goldens and scripts render through the gallery binary headless. They need a Vulkan
adapter, never the desktop display: the binary runs with `DISPLAY` unset and `WAYLAND_DISPLAY`
set to a socket that doesn't exist (`mise run test` and `bless` do the same), so libwayland
can't fall back to `wayland-0`. The harness picks the adapter in this order:

1. the ICD file named by `STRATA_VK_ICD`;
2. lavapipe for this architecture (`/usr/share/vulkan/icd.d/lvp_icd.x86_64.json`, say), then
   `lvp_icd.json`, then every other `lvp_icd*.json` found, joined with `:`;
3. the default adapter.

CI renders on lavapipe (`mesa-vulkan-drivers`). A golden blessed on a hardware GPU (NVIDIA,
say) can differ from lavapipe's render by more than the tolerance at anti-aliased edges, and
then fails in CI even though it passes locally. Install lavapipe (`vulkan-swrast` on Arch,
`mesa-vulkan-drivers` on Debian and Ubuntu) to bless on CI's renderer. A failure message names
the adapter Rune used.

## Scripts

A script is Rune's headless input script (see Rune's README, "Agent tooling") in object form,
with a header the runner checks:

```json
{
  "bullets": ["browser/selection: Shift+click extends the selection from the anchor"],
  "scene": "c1-ten-rows-light",
  "events": [
    {"t": 0.1, "type": "tap", "key": "row-3"},
    {"t": 0.3, "type": "tap", "key": "row-6", "shift": true}
  ],
  "expect": [
    {"key": "row-5", "states": ["selected"]},
    {"key": "rename-field", "absent": true}
  ]
}
```

| Key | Rule |
| --- | --- |
| `bullets` | Required, not empty. Each entry is `<node-id>: <bullet text or a prefix unique within the node>`, naming a feature-map Behavior bullet the script checks (`featuremap.py qa --node <id>` lists them). Node ids are `/`-separated segments of `a-z`, `0-9` and `-`. |
| `scene` | Required: a scene of the page whose folder holds the script (golden or review). |
| `events` | Required, not empty: Rune script events. Rune exits with an error on an unknown event type or key name, or on a `key` that names no element. |
| `expect` | Optional. Each entry has a `key`, plus any of `role`, `name`, `value` (equal), `states` (each present), `focused` (true or false) and `absent` (`true`: no node with that key). |
| `displays`, `pointer` | Optional: passed to Rune as they are. |

Any other key is an error, so typos fail. The runner starts the gallery headless with
`--scene <scene> --script <file> --tree <out>`. It fails when:

- Rune exits with an error;
- Rune reports key warnings (duplicate keys break scripts that target keys);
- an `expect` entry doesn't hold, in bless mode too;
- the accessibility tree the script ends on differs from its golden.

Run one by hand with
`mise run gallery -- --scene <scene> --script tests/scripts/<id>/<name>.json`, adding
`--snap out.png` or `--tree tree.json` to see where it ends.

Name scripts after the behavior group they check, in lowercase kebab case
(`stem-selection.json`, `commit-and-cancel.json`).

## Running and blessing

| Command | Runs |
| --- | --- |
| `mise run test` | Every test, headless |
| `mise run test c1::` | Page c1's three golden tests. The trailing `::` keeps `c1` from matching `c10`. |
| `mise run test c1::png` | One page's PNG goldens |
| `mise run bless c1` | Rewrites only `tests/golden/*/c1/` |
| `mise run bless c1-ten-rows-dark` | Rewrites one scene's a11y and PNG goldens |
| `mise run bless c5/stem-selection` | Rewrites one script's golden |
| `mise run bless` | Rewrites every golden. Avoid it on a branch: it touches other pages. |

Confirm a filtered run matched at least one test.

Give each worktree its own target directory; don't point several at one `CARGO_TARGET_DIR`.
Cargo can then reuse another worktree's build of a workspace crate, because its dep-info names
that worktree's files, and the tests run stale code. To save a full build, copy a target
directory with `cp -a --reflink=auto`, then run
`cargo clean -p strata-gallery -p strata-components -p strata-kit -p strata-platform` in the new
worktree.

`RUNE_BLESS=1` turns blessing on, as in Rune. `STRATA_BLESS` narrows it to a page id, a scene
name or `<id>/<script>`. While blessing, the harness skips every other item: it neither compares
nor writes it. `mise run bless <filter>` sets both and passes the cargo filters `<id>::` and
`registry::bless_filter_names_a_page_scene_or_script`.

- The filter must be lowercase (`a-z`, `0-9`, `-`, `_`, `/`); the task rejects anything else.
- A filter that names no page, no golden scene of its page or no script of its page fails the
  run, so a typo never passes by blessing nothing. Review scenes have no goldens, so their
  names fail too.
- A script whose `expect` entries fail is never blessed.

After blessing, run `mise run test` and review every golden diff (look at each PNG) before you
commit.

## strata-platform features

`mise run compile-features` checks `strata-platform` with no features, then with each feature
alone, so no shim leans on another's dependencies. `mise run check` and CI run it.

- Each shim's feature and its optional dependencies have their own block in
  `crates/strata-platform/Cargo.toml`. Fill in only your own.
- A crate appears once in `[dependencies]`. If another shim's block already declares a
  dependency you need (zbus for P5 and P8, say), move it to the shared block at the top of
  `[dependencies]`, keep it `optional = true`, and enable it with `dep:<name>` from each feature
  that uses it. Two blocks that each declare it merge cleanly in git but make the manifest
  invalid.
- `_transfer` is internal: P0's substrate, enabled by `dnd`, `clipboard` and `foreign-parent`.
- An example gates its own body on its feature, so it needs no `[[example]]` entry and builds
  in every feature combination:

  ```rust
  #[cfg(feature = "csd")]
  fn main() { /* the demo */ }

  #[cfg(not(feature = "csd"))]
  fn main() {
      eprintln!("run with --features csd");
  }
  ```

## Dependencies

Every crate's `[dependencies]` has a comment-separated block per work package that adds
third-party crates, plus a shared block:

| Crate | Blocks |
| --- | --- |
| strata-platform | one per feature (P0–P9) |
| strata-kit | K1, K2, K3, K4; `[build-dependencies]`: K3 |
| strata-components | C10a, C11, C12, C13, C14b, C25 |
| strata-gallery (dev) | the X1, X2 and X3 demos |

- Add a crate to `[workspace.dependencies]` (one line, in alphabetical order; check its licence
  against `deny.toml`), then `<name>.workspace = true` in your block.
- In strata-kit, strata-components and the gallery, a crate declared in any block is usable
  from every module: use it as it is, and don't declare it again.
- C10a adds `syntect` and `two-face` to `[workspace.dependencies]` and to its strata-components
  block. K1 needs syntect's `Theme` type and reuses the workspace entry: `syntect.workspace =
  true` in its own strata-kit block, with the features it needs. C10b and C11 use C10a's
  declaration as it is.

## Shared lists and merge conflicts

Git conflicts when two branches change neighbouring lines. W0 separates the lines that
different work packages edit with a blank line or a comment: the strata-platform features and
dependency blocks, and the gallery's demo dev-dependency blocks (X1, X2, X3, plus a shared
block for a crate two demos need, such as strata-platform with their features), are laid out
this way. Keep those separators. Lists without them, such as
`[workspace.dependencies]` and a crate's `[dependencies]`, can still conflict when two branches
add neighbouring entries. Resolve those on rebase. For `Cargo.lock`, take `main`'s copy and run
`cargo check` to regenerate it ([shared files](work/README.md#shared-files)).
