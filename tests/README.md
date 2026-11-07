# Test data

Per-page folders, so a branch adds and blesses only its own files. `<id>` is a gallery page id
(`c1`, `c14a`, `w2`, `gallery` for the index).

- `golden/a11y/<id>/<scene>.json`: each golden scene's accessibility tree, without bounds,
  scroll offsets, text selection or shortcuts.
- `golden/png/<id>/<scene>.png`: each golden scene rendered headless at scale 1.
- `golden/scripts/<id>/<script>.json`: the accessibility tree each script ends on.
- `scripts/<id>/<script>.json`: Rune headless input scripts, one per behavior group. Each
  names the feature-map Behavior bullets it checks in `bullets`.

`crates/strata-gallery/tests/goldens/` checks all of them. `mise run bless <id>` rewrites one
page's goldens; review every diff before you commit it. [`docs/testing.md`](../docs/testing.md)
covers the script format, the PNG tolerance and the bless filters.
