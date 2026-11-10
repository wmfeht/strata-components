# 73. A `Sequence` has no reduced pose

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C20c-3 |
| Affects | C20c (settings search's found-row highlight) |
| Retires | C20c's static fill removed by the host's tick when animations are off |
| Checked | By source at `5d421fd` and `04b38ae`: under reduced motion a `Sequence` lands its final pose. No program needed. |

Paste from here.

---

**Title:** Under reduced motion a `Sequence` lands its final pose at once, so a highlight that fades out never shows

A settings search flashes the row it jumps to (fade in, hold, fade out). Under reduced motion,
or a settle, the sequence lands its final pose, which is "faded out", so the user never sees
which row was found.

**Ask.** `Sequence::reduced(pose, hold)`: a pose held for a time instead of the final one.
