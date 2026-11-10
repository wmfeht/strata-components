# 31. Live values for more than rects, opacity and text

| | |
| --- | --- |
| Kind | New issue (related #420, #421) |
| Drafts | U-C13-1, U-C13-3, U-C16-2, U-C16-3, R-C4-4 |
| Affects | C13 (spectrum, waveform), C16 (progress bars), C4 (column resize) |
| Retires | C13's 96 `live_rect` bars in place of a shape; C16's track width read back through `on_layout_tx` and its four-a-second accessibility commits; C4's view commit on every resize move |
| Checked | By source at `5d421fd` and `04b38ae`: no `live_*` binding for shape points, rotation of non-icons, fractional width, a column width or `range_value`. No program needed beyond the shapes below. |

Paste from here.

---

**Title:** Live values: bind shape points, rotation, a fractional width and an accessibility value without a view commit

Context: a file manager's media preview (spectrum, waveform), progress bars, and a list table
whose columns resize by dragging. `Live` and `LiveText` move rects, opacity and text without
running the view; these can't be moved that way at `5d421fd` (unchanged at `04b38ae`):

- **Shape geometry.** A shape's points are set at build, so a waveform that grows in, or a
  spectrum drawn as shapes, rebuilds the view each frame (96 bars need 96 `live_rect` elements
  instead).
- **Rotation.** `LoopSpec::rotation` turns icon quads; any other element turns only through
  `El::rotate`, a commit per frame (#420 notes rotation can't be bound live).
- **A fractional width.** A progress fill can't follow a `Live<f32>` fraction of its track:
  `live_rect` is in points, so the track's width comes back through `on_layout_tx`, and
  rune-kit's `Progress` takes no live value.
- **An accessibility value.** A progress bar's `range_value` is set at build, so assistive
  technology hears it only on commits. `LiveText` does reach the tree; nothing binds a range.
- **A column width.** Resizing a table column commits the view on every move to give each
  mounted row's cell its width: 30 rows of 5 cells, a 100-frame drag, 100 view runs.

**Ask.** Live bindings for these: shape points (or a polygon's scale), rotation on any element,
`El::live_w_frac(&Live<f32>)` (or `Progress::live`), `El::live_range_value`, and a live width
that cells follow (as `live_offset` is a live position).

Related: #420, #421, #301 (a small live rect re-rasters the window).
