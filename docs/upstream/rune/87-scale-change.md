# 87. No notification on a scale-only change

| | |
| --- | --- |
| Kind | New issue |
| Drafts | C14a's "No notification on a scale-only change" draft |
| Affects | C14a, C14b (image and model viewports), C12 |
| Retires | C14a's one-off rescale child mounted to resync after a scale change |
| Checked | From C14a's report at the pin; not re-run here. |

Paste from here.

---

**Title:** A display scale change alone fires no layout handler

When only the display scale changes (a window moved to a monitor with another scale, with the
same size in points), no `on_layout` or `on_layout_tx` fires, so an element that renders pixels
for its size in device pixels (an image or a PDF page at 1:1) doesn't hear it.

**Ask.** Fire layout handlers on a scale change (with the scale), or an `on_scale_change` hook.

Related: #316, #500 (a window moved to another scale).
