# 94. The single-colour icon rasterizer ignores dashes, miter limits and clip paths

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-K3-3 |
| Affects | K3 (icon catalogs drawn under Theme colors) |
| Retires | The three Catppuccin icons listed as drawn differently under Theme colors |
| Checked | From K3's catalogue at the pin (`crates/strata-kit/icons/README.md`); not re-run here. |

Paste from here.

---

**Title:** Icons drawn through the single-colour rasterizer lose `stroke-dasharray`, `stroke-miterlimit` and `clip-path`

Icon bodies drawn in one colour (tinted to the text colour) go through Rune's own rasterizer,
which ignores three SVG attributes that resvg (the colour path) keeps:
- `stroke-dasharray`: a dashed line draws solid;
- `stroke-miterlimit`: Rune's default limit is used;
- `clip-path`: drawn unclipped.

**Ask.** Support those three in the single-colour path, or draw single-colour icons through
resvg with the colour substituted.
