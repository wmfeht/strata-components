# Testing

How the gallery, its goldens and its scripts fit together, and how a work package adds to
them without touching another's files. The plan's testing rules are in
[`plan.md` §6](plan.md#6-testing). The repository rules are in [`AGENTS.md`](../AGENTS.md).

## Gallery pages

The gallery (`crates/strata-gallery`) has an index and one page per component, plus a page
for each contract or look work package that shows something (W1, W2, K1–K4, C10a). The
registry in `src/scenes/mod.rs` declares every page. A work package edits only its own
`src/scenes/<id>.rs`, which provides four items:

| Item | What it is |
| --- | --- |
| `Demo` | The page's demo state: a `Default` type, held in `Gallery::<id>` |
| `SCENES` | Golden scenes: each gets an accessibility golden and a PNG golden |
| `REVIEW` | Scenes for review with `--scene` only, kept out of the goldens |
| `view` | The page's view, `fn(&Gallery, &mut Cx<Gallery>) -> El` |

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

Open the gallery with `mise run gallery`. A row on the index opens its page, and Escape
returns to the index. `mise run gallery -- --scene c1-ten-rows-dark` opens one scene, and
`-- --list-scenes` lists them all.

## Goldens

```
tests/golden/a11y/<id>/<scene>.json      accessibility tree, without bounds, scroll, selection and shortcut
tests/golden/png/<id>/<scene>.png        the scene rendered headless at --scale 1, 960×640
tests/golden/scripts/<id>/<script>.json  the accessibility tree a script ends on, stripped the same way
tests/scripts/<id>/<script>.json         the scripts
```

Each page gets three tests in `crates/strata-gallery/tests/goldens/`: `<id>::a11y`,
`<id>::png` and `<id>::scripts`. A page with no scenes or scripts passes them trivially.

- A golden with no scene or script behind it fails the test. Rename or remove a scene, then
  bless the page, and the stale golden is deleted.
- A failure lists every differing item with the golden's path and a fresh copy under
  `target/tmp/goldens/<kind>/<id>/`. For a PNG it also writes `<scene>-diff.png`: each pixel's
  largest channel difference, times 4, in red.

### PNG tolerance and the renderer

A pixel is off when its largest channel differs by more than 24/255. That is the "visible"
threshold of rune-kit's gallery diff (`scripts/kit-diff.py` at the pinned Rune rev). No off
pixel is allowed (`MAX_OFF_PIXELS = 0` in `tests/goldens/png.rs`). Blessing keeps a PNG golden
that still matches within the tolerance, so a render on another machine doesn't churn it.

PNG goldens and scripts render through the gallery binary headless, so they need a Vulkan adapter but
never the desktop display. The harness picks the adapter in this order:

1. the ICD file named by `STRATA_VK_ICD`;
2. lavapipe (`/usr/share/vulkan/icd.d/lvp_icd*.json`), when installed;
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

Run one by hand with `mise run gallery -- --scene <scene> --script tests/scripts/<id>/<name>.json`,
adding `--snap out.png` or `--tree tree.json` to see where it ends.

Name scripts after the behavior group they check (`stem-selection.json`, `commit-and-cancel.json`).

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
nor writes it. `mise run bless <filter>` sets both and passes the cargo filter `<id>::`. After
blessing, run `mise run test` and review every golden diff (look at each PNG) before you commit.

## strata-platform features

`mise run compile-features` checks `strata-platform` with no features, then with each feature
alone, so no shim leans on another's dependencies. `mise run check` and CI run it.

- Each shim's feature and its optional dependencies have their own block in
  `crates/strata-platform/Cargo.toml`. Fill in only your own.
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

## Shared lists and merge conflicts

Git conflicts when two branches change neighbouring lines. W0 separates the lines that
different work packages edit with a blank line or a comment: the strata-platform features and
dependency blocks are laid out this way. Keep those separators. Lists without them, such as
`[workspace.dependencies]` and a crate's `[dependencies]`, can still conflict when two branches
add neighbouring entries. Resolve those on rebase. For `Cargo.lock`, take `main`'s copy and run
`cargo check` to regenerate it ([shared files](work/README.md#shared-files)).
