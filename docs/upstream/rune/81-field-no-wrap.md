# 81. A multi-line text field that doesn't wrap

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C10b-4 |
| Affects | C10b (script editor with wrap off) |
| Retires | C10b's field made wider than its widest line inside a sideways viewport |
| Checked | By source at `5d421fd` and `04b38ae`: `text_input` sets `wrap`, and only `single_line` turns it off. No program needed. |

Paste from here.

---

**Title:** A multi-line `text_input` always wraps at its width

A code editor without wrap has to make its multi-line field wider than its widest line inside a
sideways viewport, because `text_input` sets `wrap` and only `single_line` turns it off.

**Ask.** `El::no_wrap` (or `wrap(false)`) for a multi-line field, scrolling sideways to keep the
caret in view.
