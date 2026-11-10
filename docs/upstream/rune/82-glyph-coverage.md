# 82. Ask whether a grapheme draws

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C20d-1 |
| Affects | C20d (emoji picker) |
| Retires | C20d's D3 (an emoji version cap instead of hiding what can't draw) |
| Checked | By source at `5d421fd` and `04b38ae`: no coverage query in `TextSystem`. Searched upstream (coverage, missing glyph, tofu, notdef, has_glyph): nothing. |

Paste from here.

---

**Title:** No way to ask whether a grapheme draws (or draws in colour) before showing it

GTK's emoji chooser hides emoji Pango can't draw (`pango_layout_get_unknown_glyphs_count`). A
Rune picker shows a missing-glyph box for emoji newer than the system's emoji font: `text("🫩")`
(Emoji 16.0) with a Noto Color Emoji older than that draws tofu, and nothing in `TextSystem` says
so beforehand.

**Ask.** `TextSystem::draws(&str) -> Option<Presentation>` (colour, outline, or missing).

Related: #102, #103.
