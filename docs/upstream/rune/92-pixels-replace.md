# 92. `Pixels::set` drops the image it replaces

| | |
| --- | --- |
| Kind | New issue (related #148) |
| Drafts | U-P7-2 |
| Affects | P7 (frame pacer) |
| Retires | A 3.7 MB allocation and free per 720p frame |
| Checked | By source: `Pixels::set(w, h, rgba: Vec<u8>)` (`pixels.rs:90` at `5d421fd`); no replace or in-place fill at `04b38ae`. No program needed. |

Paste from here.

---

**Title:** `Pixels::set` drops the image it replaces, so a video stream allocates every frame

Stream 1280×720 RGBA at 30 fps into one `Pixels` with `set(w, h, rgba)`: each call takes a new
`Vec<u8>` and drops the previous one under the lock, so a producer allocates and frees 3.7 MB a
frame and can't reuse a buffer.

**Ask.** A way to get the replaced buffer back (`Pixels::replace(w, h, rgba) -> Vec<u8>`) or to
fill a buffer in place (`Pixels::update(w, h, |rgba| …)`), so a pacer recycles its frames.

Related: #148 (read `Pixels` back).
