# Test data

- `golden/a11y/<scene>.json`: each gallery scene's accessibility tree, without bounds, scroll
  offsets or shortcuts. `crates/strata-gallery/tests/a11y.rs` compares against them.
- `golden/png/`: rendered scenes, once components land.
- `scripts/<behavior-group>.json`: Rune headless input scripts, run with
  `mise run gallery -- --script tests/scripts/<name>.json`. Each one names the feature-map
  Behavior bullets it checks.

`mise run bless` rewrites the goldens from the current output. Review every diff before you
commit it.
