# 43. `Tx::scroll_by`, a viewport's offset, and arrow keys that scroll a focused viewport

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C19-1, U-C7-2 (its offset and `scroll_by` half) |
| Affects | C19 (shortcut reference), C7 (tab strip wheel) |
| Retires | C19's row-step reveals for arrow scrolling; C7's offset read back from where its content drew |
| Checked | By source at `5d421fd` and `04b38ae`: no `Tx::scroll_by` or `Cx::scroll_offset`; key routing gives a focused scroll view Page Up, Page Down, Home and End only. C19's `--tree` probe at the pin; not re-run. |

Paste from here.

---

**Title:** Up and Down don't scroll a focused scroll view, and there's no `Tx::scroll_by` or way to read a viewport's offset

**What's missing** (`5d421fd`, unchanged at `04b38ae`):
- Up and Down don't scroll a focused scroll view (the key routing takes only Page Up, Page Down,
  Home and End for it). Repro: a focusable `.scroll(..)` column of 100 rows of 20 pt; focus it,
  press Down; `--tree` shows its offset still 0.
- `Tx` has no `scroll_by(key, dx, dy)`, and no API reads a viewport's offset, so a list that
  scrolls by arrows without a row cursor reveals rows itself, and an app mapping a wheel by hand
  reads where its content drew.

**Ask.** Arrows that scroll the focused viewport by a line when it overflows, as Page Down does
by a page; `Tx::scroll_by(key, dx, dy)`; and `Cx::scroll_offset(key)`.

Related: (link to 10).
