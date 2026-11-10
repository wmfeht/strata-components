# 75. Capture an element's drawn pixels

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C23-3 |
| Affects | C23 (dissolve), P1 (drag icons rendered in a second `Runtime`) |
| Retires | C23's dissolve from a host-built ghost instead of the real pixels |
| Checked | By source at `5d421fd` and `04b38ae`: no subtree capture; `drag_out::snapshot` is `pub(crate)`. No program needed. |

Paste from here.

---

**Title:** Render a subtree into `Pixels` (or a texture) from the live frame

A dissolve effect (an item breaking into fragments as it's deleted) and a drag icon need an
element's drawn pixels. Only an `effect_backdrop` slot copies the target, the whole of it up to
a point, and only inside a compositor; macOS `drag_out` has a live-frame snapshot, but it is
`pub(crate)`.

**Ask.** `Tx::capture(key) -> Pixels` (or a capture request answered next frame) for a subtree
from the live frame.

Related: (link to 05).
