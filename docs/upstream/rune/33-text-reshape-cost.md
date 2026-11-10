# 33. Text shaping: colour changes and single keystrokes reshape the whole text

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C10b-1, U-C10b-5 |
| Affects | C10b (code view and script editor), C10a (highlighting) |
| Retires | C10b's 128-line colour window and its D9 deviation (a 25–27 ms keystroke at 5,000 lines) |
| Checked | By source: `text_hash` hashes each span's start and end (`text/hash.rs:36` at `5d421fd`, `:39–40` at `04b38ae`). Timings from C10b's probe P3 at the pin; not re-run. |

Paste from here.

---

**Title:** Colour-only span changes, and every keystroke in a multi-line field, reshape the whole text

Context: a file manager previews and edits source files of thousands of lines with syntax
colouring.

**What happens** (`5d421fd`; the hash is the same at `04b38ae`):
- A text's colour spans are part of its shaping key: `text_hash` hashes every span's start and
  end (`text/hash.rs:36`). So when highlighting lands, the whole text reshapes: in a 153 KB,
  5,000-line field, about 25 ms with a 200-line span window, 160 ms with every line's 27,780
  spans.
- A keystroke in a multi-line `text_input` reshapes its whole text: about 19 ms a key in the
  same 5,000-line field with no spans.

**Ask.** Key shaping on what changes shaping only (font, weight, size, italic, mono) and repaint
colour spans over the cached layout; and shape a field by paragraph, reusing the unchanged
paragraphs' layouts.
