# 39. rune-kit `TabStrip`: labels, look hooks, gaps, keys and edge margin

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C7-1, U-C7-3, U-C7-4, U-C7-5, U-C7-7 |
| Affects | C7 (browser tabs), C11b (sheet tabs) |
| Retires | C7's ids prefixed with the strip key, the underline and tint set over each tab, its one-jump reveal clear of the edge fades (`probe.rs`) |
| Checked | By source at `5d421fd`: `.label("Close tab")` (`rune-kit/src/shell/tab_strip/tab.rs:304`), `.key(format!("t-{id}"))` (`:124`), `scroll_margin(fade)` only on the vertical strip (`shell/tab_strip.rs:586`). The label and key are unchanged at `04b38ae` (`tab.rs:307`). |

Paste from here.

---

**Title:** rune-kit `TabStrip`: fixed English close label, no active indicator, gaps only at a point, tab keys that collide, and reveals under the edge fade

**What's missing** (`5d421fd`):
1. The close button is labelled "Close tab" in English (`tab.rs:304`), with no tooltip or
   shortcut hint, and nothing sets them.
2. The look has no hooks past its fill, close glyph and sizes: no active indicator (an
   underline), no title colour, no drop-hover state.
3. `TabDragState::set_incoming` takes only a point, so a strip can't open its gap at a slot
   (after the last tab, from the keyboard), the gap is always a tab wide, and there's no thin
   insertion mark.
4. Each tab is keyed `t-<id>` whatever the strip's key (`tab.rs:124`), so two strips' tabs (or
   any other `t-` key) collide.
5. A horizontal strip's viewport has no scroll margin (the vertical one has
   `scroll_margin(fade)`), so a tab revealed at an end lands half under the edge fade.

**Ask.** `TabStrip::close_label` and `close_help`; an active-indicator option and a title or
drop-hover style; `set_incoming_at(slot)` and an insertion-mark mode; tab keys scoped by the
strip's key; the same scroll margin sideways.

Related: #396 (SidebarToggle's fixed English labels).
