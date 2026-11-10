# 52. Comment on #365: a scroll bar with a trough, apart from its viewport

| | |
| --- | --- |
| Kind | Comment on [#365](https://github.com/anomalyco/rune/issues/365) (always-visible thin scroll bar with track inset and minimum thumb length) |
| Drafts | R-C24-2 |
| Affects | C24 (its own overlay bar) |
| Retires | C24's own bar (with 51) |
| Checked | Re-checked by C24b at the pin and at `dev` 111 commits on: `track_offset` exists, the inset and least thumb are still presets' only, no track fill, and the bar wraps its viewport (`attach`). Read again here at `04b38ae`: only right-to-left changed (`0f12557b`). From the API; not run. |

Paste from here.

---

Two more needs from a file manager's scroll bars, beside the inset and minimum thumb:

- **A track fill.** Strata-style bars draw a trough (and a hover fill for it). No
  `ScrollBarStyle` field sets a track colour.
- **A bar placed apart from its viewport.** The bar wraps the viewport it scrolls (`attach`,
  placed at `viewport[1 - a] - end` across it), so a vertical bar can't sit at the visible edge
  of a vertical viewport inside a wider sideways one: in a 600 pt `scroll_x` viewport holding a
  900 pt `.scroll()` viewport with `.scroll_bar(&kit)`, the strip lands at the inner
  viewport's right edge, 300 pt past the view.

Could a bar take a `ScrollPos` and a target to scroll, and let the app place it? With
`track_inset` and `min_thumb` on any style (as asked here) and track colours, an app could use
rune-kit's bar in both cases.
