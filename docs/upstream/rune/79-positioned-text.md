# 79. Selectable text laid over glyph boxes the app places

| | |
| --- | --- |
| Kind | New issue (unlike #353, which is hit-testing shaped text) |
| Drafts | U-C12-1 |
| Affects | C12 (PDF text layer) |
| Retires | C12's port of a PDF text selection (caret, word and line, copy) and its silent accessible text |
| Checked | By source at `5d421fd` and `04b38ae`: `text()` shapes its own glyphs; nothing takes per-character rects. No program needed. |

Paste from here.

---

**Title:** Positioned text: selectable, copyable, accessible text over glyph boxes the app places

A PDF's text layer comes with a box per character from the renderer. Rune's selection runs on
text it shapes itself, so the app needs its own selection, copy and accessible text for it, and
assistive technology hears none of the selection.

**Ask.** `El::positioned_text(text, rects)` that takes part in selection, copying and the
accessible text.

Related: #353.
