# 18. Exact or per-index heights for `virtual_rows`

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C1f-1, U-C12-2 |
| Affects | C1e/C1f (group headers forced to the row height), C12 (PDF pages of mixed sizes) |
| Retires | C1f's "header lines are the row height" deviation; C12's zoom that lands up to 0.4 pages off for a frame |
| Checked | By source: one `estimated_row_height` per list (`el/virtual_rows.rs:77`) at `5d421fd` and `04b38ae`. My headless repro didn't isolate the offset, so the numbers below are the components' own probes, not re-run here. |

Paste from here.

---

**Title:** `virtual_rows` has one height estimate, so rows of known, mixed heights land where the estimate puts them

Context: a file manager's grouped list has section headers of one height and rows of another;
its PDF view has pages of mixed sizes that are known before they are drawn.

**What's missing** (`5d421fd`, unchanged at `04b38ae`). A virtual list has one height
estimate (`virtual_rows(count, estimated_row_height, ..)`), and rows it hasn't laid out take it.
Rows of two heights then sit where the app's own index geometry doesn't put them: a press, a
marquee or a reveal computed from the real heights misses. The scroll extent drifts until rows
are visited, and a zoom far down a document can't land in one frame.

**Repro** (from our probes at the pin; not re-run here): `virtual_rows` of 1,000 rows
alternating 26 and 30 pt with an estimate of 26, scrolled to 20,000 pt; a mounted row's drawn
content y differs from the sum of the heights above it by the unvisited rows' 4 pt each. With
500 PDF pages of mixed sizes, the scroll extent changes as pages are visited.

**Ask.** A height per index that layout takes instead of the estimate, either exact
("rows are exactly this tall": `virtual_rows_heights(|s, i| h)`) or as a hint used until a row
is measured.

Related: #364 (a running glide and the anchor), #203.
