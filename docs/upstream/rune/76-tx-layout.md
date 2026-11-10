# 76. A handler can't read a layout

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C23-4 |
| Affects | C23 (effects started from a handler) |
| Retires | C23's frame spent recording where an effect's sources are |
| Checked | By source at `5d421fd` and `04b38ae`: no `Tx::layout` or `Tx::presented`. No program needed. |

Paste from here.

---

**Title:** `Tx` can't read where an element is laid out or drawn

An effect started from a key or press handler (fly these items to the Trash) needs where its
sources are now. `Tx` has no `layout(key)` or `presented(key)`, so the app records rects from
`on_layout` beforehand or spends a frame finding them.

**Ask.** `Tx::layout(key)` and `Tx::presented(key)` (the rect as last drawn).
