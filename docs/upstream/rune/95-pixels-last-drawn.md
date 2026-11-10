# 95. Whether a `Pixels` is drawn

| | |
| --- | --- |
| Kind | New issue (low priority) |
| Drafts | U-C25-2 |
| Affects | C25 (thumbnail store's recency) |
| Retires | C25's recency approximated from builds and visible ranges |
| Checked | By source: `Pixels` has no draw accessor at `5d421fd` or `04b38ae`. No program needed. |

Paste from here.

---

**Title:** `Pixels` can't say whether or when it was last drawn

A thumbnail cache evicts what hasn't been seen for longest, but an app can't tell whether a
`Pixels` handle was drawn, so recency is approximated from builds and visible ranges.

**Ask.** `Pixels::last_drawn()` (a frame number) or `is_shown()`.
