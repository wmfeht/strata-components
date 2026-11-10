# 83. Comment on #102: the outline route isn't DejaVu's alone

| | |
| --- | --- |
| Kind | Comment on [#102](https://github.com/anomalyco/rune/issues/102) (default-presentation emoji render as outlines when DejaVu covers them) |
| Drafts | C20d's #102 comment draft |
| Affects | C20d (`drawn_text` adds U+FE0F) |
| Retires | C20d's `drawn_text` workaround |
| Checked | C20d's probe at the pin; not re-run here. `text/fallback.rs` still sends a grapheme to the emoji stack on U+FE0F at both revs (by source). |

Paste from here.

---

The outline route isn't DejaVu's alone. With no DejaVu installed (Noto Color Emoji 2.057 CBDT,
Noto Sans Symbols and Symbols 2), 23 of the 1,898 emoji up to Emoji 15.1 still draw as Noto Sans
Symbols outlines. Probe: one runtime, one emoji a frame, counted as colour when an image-atlas
page turns dirty. Every one draws in colour with U+FE0F after it, so an emoji picker adds it as it
draws; a fix in `fallback::routes` for `Emoji_Presentation` characters would cover both fonts.
