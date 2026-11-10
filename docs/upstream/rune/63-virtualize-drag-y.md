# 63. `on_drag` on a `.virtualize` stack reports y from the mounted rows

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C1e-1 |
| Affects | C1e (all pointer input on the container) |
| Retires | C1e's content coordinates rebuilt from the window point, the viewport's recorded origin and the scroll offset |
| Checked | From C1e's probe at the pin; not re-run here. |

Paste from here.

---

**Title:** A `.virtualize` stack's `on_drag` reports y rebased to the first mounted row, not the stack's top

An `on_drag` on a virtual list (`.virtualize` or `virtual_rows`) reports `Drag::y` relative to
the first mounted row, so the same content point gives different y as rows mount and unmount
while scrolling. A list that hit-tests presses against its own index geometry can't use it.

**Ask.** Report `x` and `y` in the list's content coordinates (or say in the docs which origin
they use, and give the content offset beside them).
