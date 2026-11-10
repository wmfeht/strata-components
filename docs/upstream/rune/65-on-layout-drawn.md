# 65. `on_layout` reports a layout spring's end, not where the frame draws

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C2-3 |
| Affects | C2 (column hit tests during a width spring) |
| Retires | C2's drop targets ahead of what is drawn while a column's width springs |
| Checked | C2's probe `p6` at the pin; not re-run here. |

Paste from here.

---

**Title:** During a layout spring, `on_layout` hears the final rect once while the element is drawn moving

During a width spring, a sibling's `on_layout` hears its final rect once, while
`ElementInfo::rect` moves over several frames, so an app hit-testing by layout is ahead of what
is drawn.

**Repro** (headless, `5d421fd`): two columns in a row; widen the first from 300 to 500 in an
animated commit: the second's `on_layout` hears x 500 at once, while its drawn x goes 310, 381,
440, 473 over the next frames.

**Ask.** The drawn rect in `on_layout` (or a drawn counterpart), or a flag saying a spring is
moving it.
