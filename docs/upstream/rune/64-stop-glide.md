# 64. Stop a glide in place

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C1e-7 |
| Affects | C1e (keyboard and marquee scrolls), C2 (column strip glides) |
| Retires | C1e's stop by a jump to a computed target, or a glide on a stiff spring |
| Checked | By source at `5d421fd` and `04b38ae`: `Tx` has scroll requests to a target, none that stops a running glide where it is. No program needed. |

Paste from here.

---

**Title:** No way to stop a running scroll glide where it is

A glide started with `scroll_to` (or Rune's reveal) can only be replaced by another request to a
target. An app that stops its own glide (a key that moves the cursor back, a marquee that
takes over) computes where the glide is and jumps there, or glides there on a stiff spring so it
doesn't jerk.

**Ask.** `Tx::stop_scroll(key)`: stop a viewport's glide or momentum at its drawn offset.
