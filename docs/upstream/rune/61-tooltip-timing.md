# 61. Per-element tooltip timing

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C3-2 |
| Affects | C3 (icon names wait 900 ms; buttons the toolkit's) |
| Retires | C3's deviation "tooltips via Rune's `.help` at the host's delay" |
| Checked | By source: `look.delay.unwrap_or(self.help_delay)`, a window-wide delay (`runtime/input/help.rs:179` at `5d421fd`). Help was rewritten as a state machine past the pin (`0cde17ec`) and keeps through a click; no per-element delay at `04b38ae`. Not run here. |

Paste from here.

---

**Title:** `El::help` has one delay per window, and a move inside the element never restarts the wait

A file manager's icon names show their full name after 900 ms, while its buttons use the
toolkit's delay; GTK restarts the wait after 4 px of movement. `El::help`'s delay is the
window's (`HelpLook::delay`, `runtime/input/help.rs:179` at `5d421fd`), and a move inside the
element never restarts it.

**Ask.** `El::help_delay(secs)`, and a restart distance.
