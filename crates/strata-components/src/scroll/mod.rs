//! Scroll extras (C24): Strata's overlay scroll bar and middle-click autoscroll, on any Rune
//! scroll viewport, and on C1e's collection through its `edge` and `on_scroll` hooks.
//!
//! Work package: C24, `docs/work/phase-1-leaves/C24-scroll-extras.md`. Behaviour follows the
//! feature map's `browser/scrolling` (and the scroll-bar bullets of
//! `browser/selection/marquee`), against Strata's `src/ui/scrolling.rs` and `src/style.css`
//! at `upstream/main` 35ee677ab.
//!
//! # The pieces
//!
//! - [`ScrollFeed`]: a viewport's latest `ScrollPos`, where handlers without the app's state
//!   read it.
//! - [`Bar`] and [`BarLook`]: the overlay scroll bar. Fed by the viewport's position, drawn
//!   over the viewport, revealed by the pointer over the listing.
//! - [`Autoscroll`]: the window's one autoscroll, its state machine and its per-frame
//!   [`Autoscroll::drive`]; [`AutoscrollPointer`] starts it from a collection's middle press;
//!   [`Shield`] is what the window shows while it runs.
//!
//! # On a collection
//!
//! ```ignore
//! // State: listing, coll: CollectionState, feed: ScrollFeed, bar: Bar, auto: Autoscroll.
//! // Once: coll.controllers.pointer = Some(Box::new(AutoscrollPointer::new(
//! //     Some(Box::new(PointerIntent::new())), auto.clone(), SCOPE, feed.clone())));
//! let (feed, bar) = (s.feed.clone(), s.bar.clone());
//! let list = Collection::new(SCOPE, ..)
//!     .settings(autoscroll_settings) // AutoscrollSettings, margin from slots::collection::row_inset
//!     .on_scroll(move |p| { feed.set(p); bar.follow(p); })
//!     .edge(|s: &S| Some(s.bar.build("files-bar", &viewport_path(SCOPE), BarLook::list(s.look()))))
//!     .build(s);
//! let list = s.bar.area(list);
//! // poll:  s.auto.drive(now, look.animations, tx) | collection(..).tick(s, tx)
//! // root:  the window's root gets Shield::new(|s| &s.auto, |s| &s.look).build(s) last.
//! ```
//!
//! Hosts that hide or close a listing call [`Autoscroll::stop_gone`] with its key; a folder
//! change (`CollectionState::enter`) stops it through the decorator's reset.
//!
//! - **C4** draws its vertical bar outside its sideways viewport, pinned to the visible right
//!   edge below its heading strip (its vertical viewport is as wide as its columns), fed by the
//!   same `on_scroll`; and passes its sideways viewport to
//!   [`AutoscrollPointer::sideways`] so autoscroll moves both axes.
//! - **C2** gives each column [`BarLook::column`] and its strip a [`BarLook::strip`] bar
//!   ([`overlay`]), with one [`Autoscroll`] for the window that every column's decorator
//!   shares.
//! - **A plain viewport** (a sidebar, a preview): `.on_scroll_offset(bar.watch())`, the bar in
//!   [`overlay`], and for autoscroll its own middle-press handler calling
//!   [`Autoscroll::start`] with a [`Target`].
//!
//! # Decisions
//!
//! - **Autoscroll glides.** Each frame retargets a `Tx::scroll_to_with` glide a lead ahead of
//!   the offset (`collection::marquee::glide_lead`, C1d's probe-checked lead), rather than
//!   stepping instant scrolls; the frames commit nothing, so Rune's perf warnings stay silent.
//!   With animations off (`Look::animations`, which hosts also pass to
//!   `Cx::reduced_motion`), Rune lands a glide at once, so each frame jumps one frame's travel
//!   instead. It asks for frames only while a viewport moves: in the dead zone, or at an end, it
//!   sleeps until the pointer's next move out of it.
//! - **The stop is dead** (Strata parity): the viewports stay where the last frame drew them.
//! - **The shield is a modal.** Rune has no capture phase; a modal layer over the window is how
//!   every press anywhere, with any button, and Escape whatever has focus, reach the autoscroll
//!   first and go no further (probe P5). It is labelled "Autoscroll" for assistive technology.
//! - **Background is Strata's.** A middle press starts autoscroll where it lands on no entry, or
//!   in the blank strip along either side of a row ([`AutoscrollSettings::margin`]), which lies
//!   outside the row in Strata's lists; anywhere else the controller inside hears it (C1b's
//!   open-in-new-tab).
//! - **The bar is revealed by the pointer over its listing** as GTK's overlay indicators are,
//!   held while the pointer is over the bar or holds its thumb, and fades [`bar::HOLD`] after.
//!   A faded bar isn't a hit target (Rune skips elements at no opacity), so a press there
//!   reaches the rows; the pointer has to move there first, which shows it.
//! - **A press on the bar's trough jumps** the thumb there and drags on from it (GTK 4's
//!   primary-button warp), rather than paging.
//!
//! # Deviations
//!
//! - **From plan §4 and the work package: C24's own bar, not rune-kit's `ScrollBar`.** At the
//!   pinned rev rune-kit keys a bar's state and its drag target by the viewport's own key, so
//!   two bars over viewports keyed alike (C1e keys every collection's viewport `viewport`:
//!   Columns, two panes) share one state, and dragging one scrolls the first (probe P2,
//!   R-C24-1). Its wrapper also moves the viewport under a new keyed parent, which breaks C1e's
//!   key paths (`{scope}/viewport`, P4), and it must wrap the viewport it scrolls, which would
//!   put C4's vertical bar at its content's right edge, off screen when the table is wider
//!   than its pane. And it draws no trough, has no end-gap or minimum-thumb setting (R-C24-2).
//!   Coordinator-approved; listed for the owner's plan edit.
//! - **While autoscroll runs the wheel does nothing, and keys other than Escape don't reach
//!   the listing**: the modal makes the window behind it inert (probe P5). Strata's capture
//!   gestures take only presses, so its wheel and keys keep working during an autoscroll.
//!   Page and window shortcuts still run (the app's `on_key` hears them, probe A1). Assistive
//!   technology hears only the dialog, "Autoscroll", while it runs (a modal hides the window
//!   behind it); Strata's autoscroll leaves its tree alone.
//! - **The scroll cursor is `Cursor::Move`**: Rune has no all-scroll cursor (U-C24-5).
//! - **The bar's fade hold is 1 s**, not GTK's own indicator timing, which isn't confirmed.
//!
//! # Left to others
//!
//! - X1 (host chrome): the popover-panel wheel bullets of `browser/scrolling` (a wheel tick
//!   outside an open Sort by, Appearance or Thumbnail size panel closes it and scrolls the
//!   listing under the pointer), and mounting the [`Shield`] at the window's root.
//! - X1 follow-ups: the bar in views that merged before C24 (C8's sidebar, C9's drawer, C14a's
//!   pixels viewport, the c1 gallery page).
//! - C18/C19: the armed 10xer chord Escape cancels; C24 tells the host the stop's reason
//!   ([`Stop::Escape`], through [`Shield::on_stop`]).
//! - Open item for a C1b/C1e follow-up: C1e counts a row's inset as the row, so a middle press
//!   on a row's margin in a listing that fits still opens the entry in a new tab (C1b), where
//!   Strata's margin is background and the press does nothing.
//!
//! # Rune and rune-kit drafts (not filed)
//!
//! Written up for the owner to file; each repro runs headless (`rune::Runtime`). Probes:
//! `strata-spikes/c24-probes`.
//!
//! - **R-C24-1 (rune-kit): scroll bars over viewports keyed alike share state and drag the
//!   first.** `ScrollBar::scroll_bar_styled` keys its kept state by `"{key}-scroll-area"`
//!   (`scroll_bar.rs`, `kept(area)`) and drags `"{area}/{key}"`, which resolves to the first
//!   match in tree order. Repro: two panes keyed `a` and `b`, each holding a 1,000-row virtual
//!   viewport keyed `viewport` with `.scroll_bar_styled(ScrollBarStyle::scroll_view(&kit))`;
//!   wheel `b` to 2,000, then drag `b`'s thumb down 120 pt: `a` scrolls to 7,956 and `b` stays
//!   at 2,000. Ask: key the state and the target by the viewport's path, or take an explicit
//!   key.
//! - **R-C24-2 (rune-kit): a scroll bar with a trough, end gaps and a minimum thumb, apart from
//!   its viewport.** `ScrollBarStyle` sets the thumb's colour and widths only: no track fill (or
//!   its hover fill), no gap from the track's ends and no least thumb length outside the
//!   presets, and the bar must wrap the viewport it scrolls, so it can't sit at the visible edge
//!   of a vertical viewport inside a wider sideways one. Repro (from the API, not run): a
//!   `scroll_x` viewport 600 wide holding a 900-wide `.scroll()` viewport with
//!   `.scroll_bar(&kit)`; `attach` places the strip at the inner viewport's right edge, past
//!   the 600-wide view. Ask: track colours, end inset and minimum length on the style, and a
//!   bar fed a `ScrollPos` and a target that the app places.
//! - **U-C24-3: a vertical wheel gesture moves to whichever viewport the pointer reaches.** The
//!   README says a gesture latches "as AppKit latches a gesture to the view it began on", but
//!   only the axis latches: `Runtime::wheel` picks `scroll_at(x, y)` for every delta, and only a
//!   horizontal gesture keeps `wheel_lock.scroller`. Repro (P1c): a `scroll_x` strip of three
//!   400-wide vertical viewports; a phased gesture (`Began`, three `Moved`, `Ended`) of `dy -40`
//!   at x 300, 390, 450, 500: columns 0 and 1 each scroll 80. Ask: keep a vertical gesture's
//!   viewport to its end, as a horizontal one keeps its scroller.
//! - **U-C24-4: a modal mounted mid-drag drops the captured drag without a release.** Repro
//!   (P6): an element with `pointer_buttons([Middle])` whose `on_drag` mounts a full-window
//!   `.modal(..)` on a middle Down; move 120 pt with the button held and release: the element's
//!   `on_drag` hears the Down and nothing more, not even a cancelled Up. Ask: end the capture
//!   with a cancelled `Up` (as Escape does mid-drag).
//! - **U-C24-5: no all-scroll cursor.** `Cursor` has `Move` but not CSS `all-scroll` (GTK's
//!   `all-scroll`, the autoscroll cursor on Linux and Windows). Repro: none needed; the enum has
//!   no variant. Ask: `Cursor::AllScroll`.
//! - **U-C24-6: a modal's dismiss has no transaction.** `El::modal(click_outside, dismiss)`
//!   takes `Fn(&mut S)`, so an Escape that dismisses can't make a scroll request (here: stop a
//!   glide) in the same frame; it waits for the next poll. Ask: `Fn(&mut S, &mut Tx)`, as
//!   `on_press_tx` is.
//! - **U-C24-7: headless Shift + wheel doesn't scroll sideways.** The winit host turns a
//!   Shift + mouse-wheel delta horizontal (`scroll/x.rs`, `wheel_delta`); `Runtime::wheel_event`
//!   with `mods.shift` and a script `wheel` with `"shift": true` scroll vertically, so the
//!   Columns bullet "Shift+wheel over a listing scrolls the column strip" can't be tested
//!   headless. Repro (P1g): `wheel_event(WheelEvent::new(0, -120).mods(shift).precise(false))`
//!   over a column: the column scrolls, the strip doesn't. Ask: apply the turn in
//!   `wheel_event`, where scripts reach it.
//! - **U-C24-8: a scroll bar's thumb drag over virtual rows trips the "stepping its own glide"
//!   warning.** A thumb drag asks for an instant scroll on each move (with `live_only`), and
//!   over a long virtual list every frame mounts new rows, a virtual-only commit (no view
//!   runs); the warning counts those commits and fires after 8 frames. rune-kit's own
//!   `ScrollBar` trips it the same way. Repro (probe K1): 1,000 virtual rows with
//!   `.scroll_bar_styled(..)`; press the thumb and move it 25 pt a frame for 12 frames: "an
//!   instant scroll … moved "viewport-scroll-area/viewport" on 8 frames in a row, each with a
//!   commit". Ask: don't count virtual-only remounts, or a scroll the pointer drives
//!   directly, as a stepped glide.

pub mod autoscroll;
pub mod bar;
mod feed;

pub use autoscroll::{
    Autoscroll, AutoscrollPointer, AutoscrollSettings, Shield, Start, Stop, Target,
};
pub use bar::{Bar, BarLook, BarMode, overlay};
pub use feed::ScrollFeed;
