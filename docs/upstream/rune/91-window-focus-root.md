# 91. `on_window_focus` is heard only on the root

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C6-6 |
| Affects | C6 (location bar), X1 (header's active state), gallery pages |
| Retires | Hosts forwarding window activation to each component |
| Checked | C6's probe P2b at the pin; not re-run here. |

Paste from here.

---

**Title:** `on_window_focus` runs only on the app's root element

A component can't hear its window activate or deactivate: `on_window_focus` is heard only on the
root, so each host forwards it. (Scripts can deactivate the window: `{"t": .., "focus": false}`
reaches the root's handler.)

**Ask.** Call `on_window_focus` on every element that has one, or add `Cx::window_focused()`
that re-runs the view on change.
