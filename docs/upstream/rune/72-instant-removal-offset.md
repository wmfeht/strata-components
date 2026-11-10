# 72. A scroll offset survives an instant removal, but not a faded one

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C20c-4 |
| Affects | C20c (rebuilt pages) |
| Retires | C20c's per-visit key for rebuilt pages |
| Checked | C20c's probe at the pin; not re-run here. |

Paste from here.

---

**Title:** A scroll viewport removed in an `instant_changes` commit comes back at its old offset; one removed with a fade comes back at 0

**Repro** (headless, `5d421fd`): a row with `instant_changes` holding one of two keyed scroll
columns by a flag; wheel the first down, flip the flag, run 30 frames, flip it back: its
content's offset is the old one. Without `instant_changes`: 0.

**Ask.** One rule: a removed viewport forgets its offset (or an explicit keep).
