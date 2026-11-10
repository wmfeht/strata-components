# 88. Closing a modal returns focus to an opener that became disabled

| | |
| --- | --- |
| Kind | New issue |
| Drafts | C15's "modal restore to a disabled opener" gap |
| Affects | C15 (dialogs) |
| Retires | C15's "Rune gap … or Strata policy with `return_focus`" bullet |
| Checked | By source: `sync_modals` drops only an opener that has gone (`runtime/input/focus.rs:440–443` at `5d421fd`). Modal focus became a state machine past the pin (`22aa8cde`, `runtime/input/modal_focus.rs`); not re-checked for this case. |

Paste from here.

---

**Title:** A modal whose action disables its opener returns focus to that disabled opener

A settings page's "Reset all" opens a confirmation; its Reset disables the "Reset all" button.
When the dialog closes, Rune returns focus to that now-disabled button: the modal restore drops
only an opener that has gone (`sync_modals`, `runtime/input/focus.rs:443` at `5d421fd`).

**Ask.** Treat a disabled (or inert) opener as gone, and fall back as for a removed one.
