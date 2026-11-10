# 90. Arrow keys between a dialog's controls by position

| | |
| --- | --- |
| Kind | New issue (low priority) |
| Drafts | C15's "Up and Down between body controls" gap |
| Affects | C15 (dialogs) |
| Retires | C15's partial bullet "With focus outside a text field, an unmodified arrow key…" |
| Checked | From C15's notes at the pin; not re-run here. |

Paste from here.

---

**Title:** Up and Down don't move focus between a dialog's controls by position

In a GTK dialog an unmodified arrow key, with focus outside a text field, moves focus to the
nearest control in that direction. In Rune, Left and Right move along a `focus_group` (a
dialog's footer), and Down opens a focused select, but Up and Down don't move between the body's
controls.

**Ask.** An opt-in spatial arrow navigation for a subtree (`El::arrow_focus(Spatial)`), or a
`focus_group` mode that moves by position in two dimensions.
