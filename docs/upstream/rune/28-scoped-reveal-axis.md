# 28. A scoped reveal can't be kept to its own viewport's axis

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C2-4 |
| Affects | C2 (Miller columns), C1e (`set_own_viewport_reveals`) |
| Retires | C1e's own-viewport reveals by an offset computed from geometry, which give up Rune's following of a row layout moves |
| Checked | C2's probe `p7` at the pin; not re-run here. `Tx::scroll_into_view_in` (`el/tx/scroll.rs:24`) unchanged in signature at head. |

Paste from here.

---

**Title:** `scroll_into_view_in(scope, key)` also scrolls a sideways viewport outside its scope, at once

Context: a file manager's Miller columns: vertical lists side by side in a horizontally
scrolled strip that glides to the column being worked in.

**What happens** (`5d421fd`). `Tx::scroll_into_view_in(scope, row)` scrolls the nearest vertical
viewport around the row and the nearest horizontal one too, even when that one is outside
`scope`, and at once in an animated commit. A list inside the strip can't reveal its row
without jumping the strip, and the strip can't glide there itself in the same frame.

**Repro** (headless, from our probe): four 300 pt columns in a 900 pt `scroll_x` strip, each a
vertical viewport of virtual rows; `scroll_into_view_in("c3", "row-3-2")` (a row already in view
vertically) puts column 3's drawn x at 900, 600, 600 over the next frames, where the strip's own
`scroll_to` to the same place glides 900, 887, 858, 823.

**Ask.** A reveal limited to an axis, or to the viewports inside its scope.
