# 69. rune-kit `Popover` has no Escape and no initial focus

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C20b-1 |
| Affects | C20b (colour button) |
| Retires | C20b's `El::modal` on the popover's body |
| Checked | By source: `Popover::build` makes a non-modal `role("dialog")` with a scrim for outside presses (`rune-kit/src/overlay.rs` at `04b38ae`); no modal option at either rev. Not run here. |

Paste from here.

---

**Title:** rune-kit `Popover`: Escape does nothing and nothing inside takes focus

`Popover`'s content is a non-modal `dialog` with a scrim for presses outside: Escape does
nothing, nothing in it takes focus, and Tab reaches the page behind it (rune-kit's README:
"popover/select opening autofocus is still separate").

**Repro** (headless): a `Popover` with a focusable child, opened by a button press; Escape
leaves it open, Tab reaches the page behind.

**Ask.** `Popover::modal(true)`: Escape closes, Tab stays in, the first stop is focused, and
focus returns to the trigger on close.
