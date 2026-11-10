# 62. `on_visible_rows` a frame late after a rekeying commit

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C1f-5 |
| Affects | C1f, C3 (grid reflow), C25 (thumbnail requests) |
| Retires | A frame of thumbnail requests for the old range after every reflow |
| Checked | C1f's probe at the pin; not re-run here. |

Paste from here.

---

**Title:** After a commit that changes the rows, `on_visible_rows` hears the new range a frame late

After a commit that rekeys a virtual list's rows (a grid reflowed to a new width), the handler
hears the new range only on the next frame, so for one frame the app's visible range is the
old one.

**Repro** (headless, `5d421fd`): a grid of 4,000 cells in lines keyed by their first cell,
scrolled to line 76 at 5 a line; narrow it to 2 a line: on the first frame the cells drawn are
380..413 and the last report still 380..465; it updates on the second.

**Ask.** Report the new range from the commit that changed the rows.
