# 71. An offstage subtree: kept mounted, out of layout, input and the tree

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C20c-2 |
| Affects | C20c (tabbed form's kept pages) |
| Retires | C20c's kept pages as opacity 0, `inert` and `a11y_hidden` in a shared grid cell, still laid out every frame |
| Checked | By source at `5d421fd` and `04b38ae`: no offstage builder. No program needed. |

Paste from here.

---

**Title:** `El::offstage(bool)`: keep a subtree's state while taking it out of layout, hit testing, Tab order and the tree

A tabbed settings window keeps each page's scroll offsets, focus memory and measured rows while
another page shows. Nothing does that: today a hidden page is opacity 0, `inert` and
`a11y_hidden` in a shared grid cell, which still lays it out every frame.

**Ask.** `El::offstage(bool)`, as SwiftUI's hidden `TabView` pages or a web `display: none` that
keeps state.
