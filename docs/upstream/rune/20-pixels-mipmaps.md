# 20. Mipmaps for a streamed `Pixels`

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C25-1 (and C14a's "`Pixels::set` builds no mips" note) |
| Affects | C25 (thumbnail store), W2d (thumbnail size classes), C12 (zoomed-out pages), C14a (images shrunk to fit) |
| Retires | W2d's 32/64/128/256 px thumbnail classes held at most twice their drawn size; hosts' use of `ImageCache` for stills only |
| Checked | By source: `Pixels::set` (`pixels.rs:90`) builds no mip levels at `5d421fd`; `pixels.rs` changed past the pin (39 lines) without adding any. C25 ran the repro at the pin; not re-run here. |

Paste from here.

---

**Title:** `Pixels::set` builds no mipmaps, so a streamed image drawn well below its size aliases

Context: a file manager streams thumbnails into one `Pixels` handle per file (the handle stays,
the image arrives later) and draws them at whatever size the grid uses.

**What's missing** (`5d421fd`). `Pixels::set` is the streaming path and builds no mip levels
(`pixels.rs:90`), so a 256 px thumbnail drawn at 37 pt samples bilinearly and aliases.
`ImageCache` images get mips, but `ImageCache::rgba` makes a new handle for each call (keyed by
the bytes) and fills its mips later, so a store that keeps one handle per file can't use it:
each landed image would be a new handle, and the view would commit to swap it in.

**Repro** (headless `--snap` at scale 1; run by us at the pin): a 256 × 256 one-pixel
checkerboard drawn at 37 × 37 pt, once streamed through `Pixels::set` and once through
`ImageCache::rgba` of the same bytes. The streamed square shows a beat pattern of near-black and
white blocks (grey levels 7 to 255, standard deviation 21 of 255); the cached one is an even grey
(188, no deviation). A 256 → 32 draw (an exact eighth) hides it.

**Ask.** A mipmapped `set` (levels built off the main thread), or
`ImageCache::rgba_into(&Pixels, …)` that fills an existing handle.

Related: #163 (atlas pages keep a CPU copy), #148 (reading `Pixels` back).
