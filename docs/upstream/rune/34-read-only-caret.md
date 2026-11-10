# 34. A caret and keyboard selection in read-only text

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C10b-2, R2 (C11) |
| Affects | C10b (code view), C11 (document view), C12 (PDF text, which ports its own caret) |
| Retires | C10b's D1 and C11's D3 deviations (no caret or keyboard selection in read-only views) |
| Checked | By source at `5d421fd` and `04b38ae`: `El::read_only` at head is the ARIA state only (`el/a11y.rs:123`); no read-only mode on `text_input`, no caret on `selectable` text. No program needed. |

Paste from here.

---

**Title:** Read-only text has no caret: no Shift+arrow selection, Home and End by line, or word steps

Context: a file manager's source and document previews are read-only, but users select from the
keyboard there, as GTK's `editable(false)` with `cursor_visible(true)` allows.

**What's missing** (`5d421fd`, unchanged at `04b38ae`):
- `text_input` has no read-only mode, so a read-only view can't have a caret, Shift+arrow
  selection, or Home and End by line.
- `El::selectable` text has no caret: no Shift+arrow, Shift+Home/End or word-step extension, no
  Right collapsing the selection, no grapheme steps.

**Ask.** `text_input(..).read_only()` that keeps navigation, selection and copy and refuses
typing, paste, cut and IME; or a keyboard caret for `selectable` text.
