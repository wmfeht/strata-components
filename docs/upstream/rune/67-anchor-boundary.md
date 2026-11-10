# 67. Anchored overlays are bounded by the window only

| | |
| --- | --- |
| Kind | New issue (related #362) |
| Drafts | U-C22-3 |
| Affects | C22 (folder peek beside a drawer) |
| Retires | C22's `FolderPeek::bounds` clip computed by the host |
| Checked | By source: `anchor_position(a, t, size, vp)` takes the viewport only (`runtime/anchor.rs:21` at `5d421fd`); at head anchoring gained right-to-left mirroring only. C22's probe at the pin; not re-run. |

Paste from here.

---

**Title:** Anchored overlays flip and clamp against the window only; add a boundary element

`anchor_position` (`runtime/anchor.rs:21` at `5d421fd`) flips and clamps against the window, so
a card beside a list next to a drawer opens over the drawer.

**Repro** (headless): a 400 pt list at x 100 and a 300 pt drawer at x 500 in an 800 pt window; a
256 pt card anchored `RightStart` to a row lands at x 508, over the drawer.

**Ask.** A boundary element for flip and clamp, as Floating UI's `boundary`.

Related: #362 (report the side a placement flipped to).
