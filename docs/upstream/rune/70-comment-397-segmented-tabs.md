# 70. Comment on #397: a tab mode for `SegmentedControl`

| | |
| --- | --- |
| Kind | Comment on [#397](https://github.com/anomalyco/rune/issues/397) (SegmentedControl segments can't carry an icon, trailing element or tooltip) |
| Drafts | U-C20c-1 |
| Affects | C20c (settings tabs), C19, C20d (category tabs) |
| Retires | C20c's, C19's and C20d's own tablists |
| Checked | By source: `row().role("radiogroup")` (`rune-kit/src/segmented.rs:76`) at `5d421fd` and `04b38ae`. |

Paste from here.

---

Adding a related need: a tab mode. `SegmentedControl` builds a `radiogroup` of `radio` segments
from text labels (`rune-kit/src/segmented.rs:76`), so it can't be a settings window's or a
picker's page switcher: that needs `tablist`, `tab` with `selected`, and a `tabpanel` it names.
A per-segment trailing slot, as asked here, would also carry an error dot whose meaning the
segment's accessible description names.
