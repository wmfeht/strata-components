# 93. Draw a run-time image in the element's colour (symbolic icons)

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-P6-2 |
| Affects | P6 (icon theme), the sidebar and toolbar icons loaded from the desktop's theme |
| Retires | P6's deviation "symbolic icons drawn as authored" |
| Checked | By source: `El::icon_one_color` (`el/visuals.rs:98` at `5d421fd`) works on a `rune::Icon`, whose body is `&'static str`; nothing tints `Pixels`. Same at `04b38ae`. |

Paste from here.

---

**Title:** Draw a `Pixels` image's alpha in the element's colour, for symbolic icons loaded at run time

Freedesktop symbolic icons (`*-symbolic.svg`) are drawn in one dark grey (Adwaita's `#2e3436`)
and meant to be recoloured to the text colour, as GTK does. Loaded from the icon theme at run
time, through `ImageCache::file` and drawn with `El::pixels`,
`/usr/share/icons/Adwaita/symbolic/places/folder-symbolic.svg` draws dark grey on a dark
background. `El::icon_one_color` would draw it as a silhouette in the element's colour, but only
for a `rune::Icon`, whose `body` is `&'static str`, so a body read at run time would have to leak.

**Ask.** `El::pixels_tint()` (or `pixels_mask`): draw an image's alpha in the element's colour,
as `icon_one_color` draws an icon's; or a `rune::Icon` with an owned body (`Arc<str>`).

Related: #21.
