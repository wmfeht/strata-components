//! ThumbnailStore (C25): thumbnails as `rune::Pixels` handles under a byte budget, and the
//! requests to load next. Loading and decoding stay in Strata.
//!
//! [`ThumbnailStore`] keeps one entry per thumbnail key ([`ThumbKey`], W1): a [`Thumb`] (an
//! image handle and its placeholder icon's opacity), what the image holds, and the key's
//! [`State`]. Viewers (a list, a grid, a Columns column) tell it what they show
//! ([`ThumbnailStore::sync`]); it ranks the keys into [`Request`]s ([`ThumbnailStore::pop`]),
//! and the host hands each result back ([`ThumbnailStore::complete`]). The store owns no I/O:
//! the host's worker renders, from Strata's disk cache or its sandbox, scaled to the request's
//! [`Step`].
//!
//! ```ignore
//! // The view: build the slot with the step the view draws at.
//! let step = Step::for_tile(look, 48.0, cx.env().scale);
//! Collection::new("files", get, get_mut, look, version)
//!     .icon(icon_slot(|s: &App| &s.thumbs, step, 48.0))
//!     .on_visible(move |s: &mut App, r| {
//!         let view = Viewport::new(r, step).listing(s.version);
//!         s.thumbs.sync(s.viewer, &s.listing, view) // true only after a prune
//!     })
//!     .build(s)
//!
//! // poll: hand results over, take new work. Return false for all of it.
//! for t in s.thumbs.take_cancelled() { s.worker.cancel(t) }
//! for (ticket, outcome) in s.worker.finished() { s.thumbs.complete(ticket, outcome, now); }
//! while s.worker.has_room() { let Some(r) = s.thumbs.pop(now) else { break }; s.worker.start(r) }
//! ```
//!
//! # No view commit per thumbnail
//!
//! A key's handles are made once ([`ThumbnailStore::thumb`], called while the row builds) and
//! filled, resized and emptied in place: `Pixels::set` and `Live::set` wake only the windows
//! that draw them. So:
//!
//! - a part that draws a key's handles is versioned by [`ThumbnailStore::version`] (and its
//!   step), never by the key's state: a version that changed when a thumbnail landed would
//!   build the row again for every thumbnail;
//! - the host's `poll` returns false after [`ThumbnailStore::complete`]: nothing the view reads
//!   changed;
//! - the part draws the icon under the image, with the icon's opacity bound to the handle
//!   ([`picture`]), so an image landing (or a transparent one) needs no rebuild either.
//!
//! The tests hold this in a headless runtime (no view run and no commit beyond the scroll's own
//! while thumbnails land), and the gallery's `c25-work` line counts the view's runs while a
//! round of thumbnails lands. The commit budget was checked by hand:
//!
//! ```text
//! cargo build --profile perf -p strata-gallery
//! STRATA_C25_BUDGET_WARMUP=1 RUNE_COMMIT_BUDGET_MS=1 target/perf/strata-gallery \
//!     --scene c25-hundred-k-light --scale 2 --script <a 4.5 s fling down and back> --tree out.json
//! ```
//!
//! prints the 100k list's first mount (about 29 ms, "committed for dirty") and nothing after
//! the warm-up while 130 thumbnails render and 154 requests are cancelled.
//!
//! # The budget, and what is protected
//!
//! Bytes count `width × height × 4` of each held image. Eviction empties the least recently
//! touched image (touched when its row builds, and on every sync that shows it) that no viewer
//! shows now or showed at its previous sync: Rune reports a reflowed grid's rows a frame late
//! (C1e's draft U-C1f-5), so the previous sync's rows stay protected for one more sync. Rows
//! on screen win over the budget (owner ruling): **bytes stay within the budget unless the
//! protected images alone exceed it**, and then only protected images are held and no margin
//! is asked for. Evicting empties the handle in place, so a row that still draws it shows its
//! icon again, with no commit. The budget doesn't count Rune's own copy of the images drawn
//! this frame (its image atlas) or the GPU's.
//!
//! # Priority
//!
//! A viewer's keys rank in three tiers: the viewport in reading order, then the margin the
//! viewer is scrolling toward, nearest line first, then the margin behind it, nearest first
//! ([`Tier`]). Margins are whole lines, [`Settings`]' `margin` of the viewport per side. Within a
//! tier, keys never loaded go before keys held at another step; every viewer's viewport goes
//! before any margin. Keys past every viewer's margin are never asked for, and requests out
//! for them are cancelled. A result for a cancelled request is dropped, as Strata drops it.
//!
//! # Handles live while a viewer is near
//!
//! Empty handles (nothing held, nothing asked) are pruned once there are more than a few
//! thousand, except near any viewer's viewport. A row still drawing a pruned key's handles would
//! never see its thumbnail, so a sync that pruned returns true: the host's `on_visible`
//! returns it, the view runs once, and such a row builds on the key's new handles (its version
//! carries the handles' epoch).
//!
//! # Decisions
//!
//! - **Keyed by [`ThumbKey`], not `EntryId`** (owner ruling; deviation from the WP's wording).
//!   W1 names C25 as the key's user: it changes when the content does, and it is absent for
//!   entries that never get a thumbnail (folders, unsupported types), so the host's policy
//!   reaches the store as data. Requests carry the [`EntryId`] the key was asked for.
//! - **The size classes are the look's** (`Look::thumbnails`, W2), through [`Step`]. One
//!   handle per key holds one image at a time; a new step replaces it in place, and the old
//!   image draws meanwhile. Two viewers wanting one key at two steps get the larger.
//! - **The on/off switch is the store's** (owner ruling): off, the store asks for nothing and
//!   cancels what is out, and parts draw icons alone ([`ThumbnailStore::set_enabled`] returns
//!   true for the one commit). Held images stay, so on draws them again with no render.
//! - **Strata's numbers are [`Settings::default`]** (owner ruling): 64 MiB, a 25% margin, 30 s
//!   before a failed key is asked again. Strata passes its own.
//! - **A stopped render is its own outcome** ([`Outcome::Stopped`]): not failed, and asked
//!   again after [`ThumbnailStore::requeue_stopped`], as Strata's file-activity hold does.
//! - **Inputs are checked**: an image whose size doesn't match its bytes, is empty or exceeds
//!   its step is recorded as failed ([`Why::Invalid`]), never handed to `Pixels::set` (which
//!   panics on a mismatch).
//!
//! # Deviations from Strata (owner rulings)
//!
//! Strata's in-memory cache is `src/ui/thumbnail/cache.rs` and its scheduling
//! `src/ui/thumbnail/viewport.rs`, at `upstream/main` 35ee677ab.
//!
//! 1. **The margin loads nearest first, in the scroll direction** (the WP asks for it). Strata
//!    loads the margin top to bottom, whichever way the view scrolled.
//! 2. **No entry cap.** Strata also caps the cache at 256 entries; at its 256 px canonical size
//!    that is its 64 MiB, so the byte budget alone keeps the bound.
//! 3. **Images are held per size class**, not as one 256 px texture: Rune draws a streamed
//!    `Pixels` with no mipmaps (draft U-C25-1), so an image is held near the size it draws.
//!    Strata's disk cache stays at 256 px. Until the look has classes below 128 px (W2d), a
//!    16–48 pt list thumbnail is drawn down from 128 px and aliases.
//! 4. **Recency is drawing, not lookups**: a key is touched when its row builds and when a
//!    sync shows it. Strata touches on every cache lookup.
//!
//! # Strata's, not C25's
//!
//! Which entries get a key (folders, `admin://`, custom icons, formats, the STL switch, remote
//! mirrors and cameras), rendering and scaling to the step, the workers and their admission,
//! the disk cache, what counts as a file operation's hold, when a rename keeps the content
//! ([`ThumbnailStore::relocate`] moves it), and the metadata a render reports (the Icons
//! caption's dimensions). Folder recounts after an operation can use the same viewport:
//! `CollectionState::visible_range`.
//!
//! # For C3's review
//!
//! C3 (IconsGrid) hadn't started when this landed; its planner reviews this list:
//!
//! - [`Thumb`] and [`picture`]: a fixed box, the image fitted (letterboxed, so a caption under
//!   it sits under the box, not the image), the icon centred underneath; the caller keys it
//!   (`ICON`, so C23's flights find it).
//! - The step: `Step::for_tile(look, tile points, scale)`; one handle per key, replaced in place
//!   on a zoom, the old image drawing until the new one lands. A change of display scale alone
//!   reaches no handler (C14a's draft), so the step follows on the next commit.
//! - [`Viewport::per_line`] from C3's own count; `sync` from `on_visible` (returning its
//!   result), `resync` after a reflow or a listing change.
//! - The tile's version: `version(key)` and the step, never the state.
//! - The worker in `poll`: `take_cancelled`, `complete`, `pop` to capacity, returning false.
//! - Placeholder icons at large tiles: K3 measured about 0.5 ms to rasterize a 256 px icon the
//!   first time; the icon stays mounted at opacity 0 under a held image, so it isn't
//!   rasterized again when the image goes and comes back.
//! - Memory at 256 px tiles: 256 KiB a held image, and Rune copies each drawn one into its
//!   1024² image pages (16 a page).
//!
//! # Rune drafts (not filed)
//!
//! - **U-C25-1: mipmaps for a streamed `Pixels`.** `Pixels::set` is the streaming path: no mip
//!   levels (`pixels.rs`), so drawing an image well below its size samples it bilinearly and
//!   aliases. `ImageCache` images get mips, but their handle is made at decode time, so a view
//!   would commit to swap one in. Repro: `Pixels::set` a 256 × 256 one-pixel checkerboard and
//!   `--snap` it drawn at 32 × 32 beside `ImageCache::rgba` of the same bytes. Expected (from
//!   the code, not yet run): moiré from the first, an even grey from the second. Ask: a mipmapped `set` (levels built off the main thread),
//!   or `ImageCache::rgba_into(&Pixels, …)` filling an existing handle.
//! - **U-C25-2: whether a `Pixels` is drawn.** An app can't tell whether or when a handle was
//!   last drawn, so a cache's recency has to be approximated (here, from builds and visible
//!   ranges). Repro: none needed; `Pixels` has no such accessor at the pinned rev. Ask:
//!   `Pixels::last_drawn()` (a frame number) or `is_shown()`.
//! - **U-C25-3: scripts can't check a drawn image.** Thumbnails are decorative, hidden from
//!   assistive technology, so a script's tree can't say whether one landed; the gallery reads
//!   it through status lines instead. Repro: a script over a view with an `El::pixels` part;
//!   `--tree`'s `elements` carry no image state. Ask: the image's size (or "empty") in
//!   `--tree`'s elements, for `expect`.
//!
//! The gallery's status lines are built text, not `live_text`: a live label anywhere makes
//! every shift of a virtual window run the view (C1e's draft U-C1f-6).
//!
//! Work package: C25, `docs/work/phase-1-leaves/C25-thumbnail-store.md`.
//!
//! [`ThumbKey`]: crate::contract::ThumbKey
//! [`EntryId`]: crate::contract::EntryId

#![warn(missing_docs)]

mod part;
mod queue;
mod step;
mod store;

#[cfg(test)]
mod tests;

pub use part::{THUMB, icon_slot, picture};
pub use queue::{Direction, Request, Tier, ViewerId, Viewport};
pub use step::Step;
pub use store::{Landed, Outcome, Settings, State, Stats, Thumb, ThumbnailStore, Ticket, Why};
