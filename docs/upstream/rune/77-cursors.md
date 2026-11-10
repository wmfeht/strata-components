# 77. Cursors: all-scroll, wait and progress

| | |
| --- | --- |
| Kind | New issue (beside #145, #233, #292 for resize shapes) |
| Drafts | U-C24-5, R-C14b-2 |
| Affects | C24 (middle-click autoscroll), C14b (model view rendering) |
| Retires | C24's `Move` cursor for autoscroll; C14b's default arrow while rendering |
| Checked | By source: `Cursor` at `5d421fd` (`cursor.rs:15`) and `04b38ae` (which adds `ZoomIn` and `ZoomOut` in `776952b0`) has no all-scroll, wait or progress. No program needed. |

Paste from here.

---

**Title:** `Cursor` has no all-scroll, wait or progress shape

`Cursor` (`cursor.rs:15` at `5d421fd`; `ZoomIn` and `ZoomOut` added at `04b38ae`) has `Move`
but not CSS `all-scroll` (the middle-click autoscroll cursor on Linux and Windows), and no
`wait` or `progress`, which a view shows while it renders. winit has all three
(`CursorIcon::AllScroll`, `Wait`, `Progress`).

**Ask.** `Cursor::AllScroll`, `Cursor::Wait` and `Cursor::Progress`.

Related: #145, #292 (resize shapes).
