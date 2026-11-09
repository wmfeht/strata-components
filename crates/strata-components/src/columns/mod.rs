//! ColumnsView (C2): Miller columns, a sideways strip of C1e collections, each its own virtual
//! list and selection.
//!
//! Work package: C2, `docs/work/phase-3-views/C2-columns-view.md`. Feature map nodes
//! `browser/view-modes/columns` (108 bullets) and `operations/drag-and-drop/column-autoscroll`
//! (9). Behaviour follows Strata's `src/ui/browser/columns/` at `upstream/main` 35ee677ab.
//!
//! ```ignore
//! // State: listings: impl ColumnSources, columns: ColumnsState, look: Look.
//! let view = ColumnsView::new(
//!     |s: &App| ColumnsParts { sources: &s.listings, state: &s.columns, look: &s.look },
//!     |s: &mut App| ColumnsPartsMut { sources: &s.listings, state: &mut s.columns, look: &s.look },
//! )
//! .collection(|s, depth, c| c.clock(|s: &App| s.clock))      // the host's per-column extras
//! .pointer_settings(|s: &App| s.pointer_settings())          // P5's interval, the click modes
//! .header_actions(|s, cx| s.header_actions(cx))              // Refresh, Sort by, the filter
//! .on_intent(|s, intent, tx| s.columns_policy(intent, tx));  // Strata's policy
//! let strip = view.build(s);                                 // the view (not memoised)
//! // poll: view.tick(s, now, tx) | s.columns.autoscroll().drive(now, animations, tx)
//! // C9:   PreviewDrawer::new(..).arrangement(Arrangement::Columns(view.fit(s, &rules)))
//! //           .build(strip, look, cx); and drawer.light_line(s.columns.last_edge_lit())
//! ```
//!
//! # What the view holds, and what the host does
//!
//! The host keeps its listings ([`ColumnSources`]: the folders, root first, as data) and a
//! [`ColumnsState`], and changes them together: it opens a folder and calls
//! [`ColumnsState::push`] (or [`ColumnsState::swap_from`] for a sibling), closes columns with
//! [`ColumnsState::truncate`], rebuilds the strip with [`ColumnsState::replace`], and returns to
//! an ancestor with [`ColumnsState::return_to`], which selects the folder it came from. Which
//! folder opens when (a click, Return, mirroring with its 75 ms wait, Back), and what a click on a
//! file does, is Strata's policy: the view reports it ([`ColumnsIntent`]) and the host decides.
//!
//! The view holds the interaction and the placement:
//!
//! - **One focus stop.** The strip is focusable and takes the keys; every column's collection is
//!   built with C1e's `Collection::keys_by_host`, so a press anywhere focuses the strip, and the
//!   strip sends each key to the column that holds the keyboard cursor (`Collection::key`). Rune
//!   can't move focus between elements from code at the pinned rev, and Strata's whole strip is
//!   one Tab stop anyway. Only the active column shows its cursor.
//! - **Left and Right** (and `h`, `l` through C1c's aliases) reach the column's edge, which C1c
//!   reports: Right goes into the open child column (its cursor on its first entry if it has
//!   none), else asks the host to open the folder under the cursor
//!   ([`ColumnsIntent::EnterFolder`]); Left goes to the parent (its cursor on the folder it has
//!   open if it has none, once listed if it is loading), and past the root to the host. Escape
//!   that C1c doesn't take (nothing selected) closes the deepest column, unless the host says
//!   not to (10xer, [`ColumnsView::escape_closes`]). Ctrl+A, the page keys, Home, End and
//!   type-ahead act in the active column only, as C1c does in any collection.
//! - **Presses.** A press targets its column (its header shows the actions) and makes it active,
//!   without a reveal; it cancels a slow-click rename pending in another column. The second press
//!   of a click that opened a column leaves that column active (a double click on a closed
//!   folder). Activating the folder whose column is open goes to that column. A plain click on a
//!   column's empty space closes the deeper columns and, in an inactive column, selects its first
//!   visible entry, or in the active one clears every column; a click on the blank strip clears
//!   every column. A click on a header (its title or blank space) focuses the column and keeps
//!   its selection, and a drag from there bands (C1e's `header_defers_press`).
//! - **The strip's scroll.** A keyboard move, an opened or closed column and a background click
//!   reveal the active column as Strata's `ColumnSpan::reveal_target` does: a column clipped
//!   even a little comes whole into view, a shown one stays, one wider than the strip that fills
//!   it stays, and the columns after it (a mirrored child) come along where there is room. A
//!   sideways wheel or touchpad gesture scrolls the strip (Rune's axis routing); the strip's bar
//!   and each column's are C24's ([`crate::scroll::BarLook::strip`], `BarLook::column`).
//! - **Lifecycle motion.** A new column slides 24 points in from the left as it fades in; a
//!   sibling fades in where the column it replaces stood (its shell kept, its body new); a closed
//!   column narrows and fades out (Rune presence with `hold_layout`), taking no input while it
//!   leaves. With animations off, and on a rebuild, nothing moves.
//! - **Widths.** A column's right edge resizes it, 6 points either side of the line, never below
//!   one standard column (300 at the default text size) and never past the area the host sets
//!   ([`ColumnsState::set_width_cap`]); the release hands the host the width once
//!   ([`ColumnsIntent::Width`]), and a cancelled drag puts it back. Resting on an edge for 400 ms
//!   shows a "Column width" caption. New columns open at the saved width narrowed to the area.
//! - **Drags.** A held file drag near the strip's left or right edge, or a listing's top or
//!   bottom, scrolls it ([`drag_scroll`]), and the column under it shows an accent wash and ring.
//!   Drops on a column go to the host with the column ([`ColumnsIntent::Dropped`]), and the strip
//!   stays where it is. File drags out start from C1b's `Intent::BeginDrag` (X2's).
//! - **The preview slot** (C9): [`ColumnsView::fit`] tells the drawer about the strip, and the
//!   columns after the active one borrow the reserved slot's width; [`ColumnsState::last_edge_lit`]
//!   says when to light the drawer's line.
//! - **What assistive technology hears.** The strip is a group named "Columns" whose active
//!   descendant is the active column's cursor row, or that column's list while it has no cursor,
//!   so a change of column is heard as the list it lands in. Each column is a list box named by
//!   its folder; its header's buttons are named ("Close this pane", the host's), the spinner is a
//!   progress indicator named "Loading", the resize edge is hidden.
//!
//! # Decisions and deviations
//!
//! - **Keys reach the active column through the strip** (owner ruling): the strip is the one
//!   focus stop. Strata focuses the row in GTK's list; here focus stays on the strip, with the row
//!   as its active descendant.
//! - **Header buttons are not Tab stops**: the close button is built unfocusable, and hosts
//!   build their header actions the same way, so a press on one focuses the strip and the keys
//!   go on to the active column (closing a column takes its button with it). Strata's header
//!   buttons take no focus on a click either, but are reachable by Tab; here the header's
//!   actions are reached by pointer, the folder menu and the shortcuts.
//! - **Reveals and the drag autoscroll glide** on Rune springs (C1d's and C24's glide lead),
//!   rather than stepping the adjustment each frame as Strata does; the speeds are Strata's. The
//!   reveal's spring stands in for Strata's 220 ms emphasized deceleration, and the lifecycle's
//!   for its 220 ms curves. The strip moves on that glide alone: each column's rows reveal
//!   within their listing (C1e's `CollectionState::set_own_viewport_reveals`), since Rune's
//!   reveal request would jump the strip too (Rune draft U-C2-4). A reveal asked for while one
//!   glides (key repeat) carries on at its speed.
//! - **In Columns the drop edge scroll is Strata's** (44 points, up to 520 points a second, with
//!   its ramp), not C1e's row-height steps: the view replaces C1e's through `on_drag_over`.
//! - **A swapped-in sibling's shell keeps the old column's width until it springs to the new
//!   one**, when they differ (the shell is kept so the new body fades in in place).
//! - **The header's actions are a slot**: Refresh, sort, Sort by, the filter field and Empty
//!   Trash are the host's ([`ColumnsView::header_actions`]); the view adds the close button in
//!   the targeted column after the first.
//! - **The badges (92–107) and the width polish (79, 83, 87, 89, 90) are C2b's** (coordinator
//!   ruling): the spread of a dragged width to the other columns, autofit, a text-size change.
//!   C2b also takes the header title's weight (Strata's 700; the kit has no bold UI weight
//!   token, so it is `ui_medium` here) and the accent on the resize line's hover.
//!
//! # Known gaps
//!
//! - **A change of the strip's width more than half a second after a reveal** isn't followed:
//!   the reveal is worked out again on each tick while the strip's drawn width moves within
//!   half a second of it (a drawer opening in the same handler, a width spring), counted from
//!   the reveal, so a window resized on and on isn't held to the column. A scroll of the
//!   person's (the strip moving other than toward the reveal's target) ends it sooner.
//! - **A wheel in the frame a reveal is asked for loses one step of it**: Rune applies the
//!   view's request after the wheel, and the view can't hear a wheel over a listing (Rune hands
//!   `on_wheel` only up to the nearest scroll view), so the glide takes one step from where
//!   the wheel left the strip before the next tick stops it there.
//! - **An idle collection costs about 0.14 ms a frame while another scrolls**, where an idle
//!   rune-kit Table costs nothing, so four columns scroll at about 1.75× four Tables' frame time
//!   at p50 (about 0.55 ms, steady across runs) and 1.8× to 2.2× at p95 (1.6 to 2.1 ms, with
//!   the machine's load; `tests/perf.rs`). Four bare C1e collections in a row cost the same,
//!   so it is C1e's, open for a C1e follow-up.
//! - **A vertical swipe that crosses columns scrolls each of them** (Rune draft U-C24-3).
//! - **Shift+wheel can't be tested headless** (Rune draft U-C24-7); the plain-wheel half is.
//! - **During a width spring** (a swapped sibling's), the columns' spans the drag autoscroll and
//!   [`ColumnsView::drop_target_at`] work from are where the layout ends, not where the frame
//!   draws them (Rune draft U-C2-3).
//! - **For the 220 ms a column comes or goes, [`ColumnsView::drop_target_at`] maps points by
//!   the live columns where they end**: a point inside a new column's 24 pt slide maps to the
//!   column before it, and a point over a closing column (drawn fading over the one that
//!   replaces it, and inert) to the live column under it, as Rune routes the drop itself.
//!
//! # Tests
//!
//! Each Behavior bullet, and what checks it. Scripts are in `tests/scripts/c2/`, on the gallery
//! page (`crates/strata-gallery/src/scenes/c2.rs`, whose host stands in for Strata's open, close
//! and back policy); "page" tests are in `crates/strata-gallery/tests/c2.rs`; "unit" tests in
//! `tests.rs` and `drag_scroll/tests.rs`. *Partial* covers the half named; the rest is the
//! host's or another work package's. Numbers are the bullets' order in `featuremap.py qa --node
//! browser/view-modes/columns`.
//!
//! | Bullet | Check |
//! | --- | --- |
//! | 1 A single click on a folder opens it as the next column | `open-folder`; page `a_click_on_a_folder_opens…`, *partial*: opening the listing is the host's |
//! | 2 Each column after the first has a "Close this pane" X | `close-button`; page `the_close_button_closes…`, `after_the_close_button_the_keys…` (the keyboard goes on working) |
//! | 3 Clicking a folder whose column is open focuses and reveals it | `reopen-open-folder`; page `a_click_on_a_folder_whose_column_is_open…` |
//! | 4 A slow second click on a selected, open folder's name renames | `slow-click-rename`; page `a_slow_second_click…`, `a_press_in_another_column_cancels…` |
//! | 5 Double-clicking a closed folder ends as a single click | `double-click-closed-folder`; page `a_double_click_on_a_closed_folder…` |
//! | 6 The same with double-click folders, from a clipped parent | page `with_double_click_folders…` |
//! | 7 A plain click on a folder in a multi-selection collapses and opens | page `a_plain_click_on_a_folder_in_a_multi_selection…` |
//! | 8 Single-click previews collapse to a previewable file and preview it | Strata policy (the preview); the collapse is C1b's, as 7 |
//! | 9 A sibling fades in where the old column stood; deeper ones shrink away | page `a_sibling_fades_in_its_place…` (what is drawn); unit `a_new_column_slides_in_and_a_sibling_fades…` (the motion each change takes) |
//! | 10 Pressing a folder doesn't shift the strip and back | page `a_double_click_on_a_closed_folder…` (the press moves no strip) |
//! | 11 A closed column narrows and fades out over 220 ms | page `a_closed_column_fades_as_it_narrows`, `the_close_button_closes…` (it leaves on a transition, then goes) |
//! | 12 The close animation covers X, Escape, Backspace, a file click in a parent | page `a_closed_column_fades…`, *partial*: any closed column animates the same way; which keys close is the host's |
//! | 13 A plain click on a file in a parent closes the deeper columns | `file-click-closes-deeper`, *partial*: Strata policy, the page stands in |
//! | 14 With single-click previews off, that click opens no preview | Strata policy |
//! | 15 A press on a file that becomes a drag leaves deeper columns open | `file-drag-keeps-columns`; page `a_file_drag_or_a_modified_click…` |
//! | 16 Ctrl+click and Shift+click on a file leave them open | page `a_file_drag_or_a_modified_click…` (modifiers held in process) |
//! | 17 With animations off a closing column goes at once | page `with_animations_off…` |
//! | 18 A closing column takes no clicks, hovers or drops | page `a_closing_column_takes_no_presses…` (Rune's presence makes it inert) |
//! | 19 Another location or a rebuild replaces columns with no exit | page `a_rebuilt_strip_replaces…` ([`ColumnsState::replace`]) |
//! | 20, 21 Deleting the focused open folder reveals and mirrors the next | Strata policy (deletion and mirroring) |
//! | 22 A keyboard move reveals a column clipped by up to 4 px | page `a_keyboard_move_reveals…`, `a_keyboard_reveal_glides_the_strip`, `holding_down_glides…`, `holding_left_glides…`, `right_into_a_clipped_new_column…`, `a_column_opened_with_its_first_entry…`, `a_reveal_in_the_same_handler…`, `a_drawer_opening_in_the_same_handler…`, `a_reveal_follows_the_strips_width…`; unit `a_column_clipped_even_a_little…` |
//! | 23 A column wider than the strip that fills it stays | page `a_column_wider_than_the_strip…`; unit `a_column_wider_than_the_strip…` |
//! | 24 Mirroring shows a folder's contents without moving focus | *partial*: [`Focus::No`] keeps the active column (page tests open with it); mirroring is Strata's |
//! | 25–27, 29–32 Mirroring, Quick Preview, the 75 ms wait | Strata policy (the view reports [`ColumnsIntent::Cursor`]) |
//! | 28 With no cursor, Home/Ctrl+Up land first, End/Ctrl+Down last | `home-end-no-cursor`; page `ctrl_a_page_and_home…` |
//! | 33, 34 Open as Administrator; the lock notice | Strata policy (the host fills the column's `empty` part) |
//! | 35 Back to an ancestor selects the folder you came from | `back-to-ancestor`; page `back_to_an_ancestor…`, `back_to_an_ancestor_scrolls…`, `back_to_a_clipped_ancestor…`, `forward_to_a_deeper_open_column…`, *partial*: navigation is the host's ([`ColumnsState::return_to`]) |
//! | 36 The came-from folder's own column stays closed | `back-to-ancestor` |
//! | 37, 38 Back to a non-ancestor; a sidebar ancestor | Strata policy |
//! | 39 Left into a column with no cursor lands on its open folder | page `left_into_a_column_with_no_cursor…`; Right into one, its first entry: `right_into_a_child_column…` |
//! | 40 …once the column is listed; nothing if the folder went | page `left_into_a_loading_column…`, `a_late_listing_in_a_column_left_behind…` (none once the keyboard left it), `a_late_listing_in_the_active_column…` (the strip's glide goes on); [`ColumnsState::sync`] |
//! | 41 F2, Delete and copy fall back only in the deepest column | Strata policy |
//! | 42, 43 F5 keeps focus; F2 after a reload | Strata and C5/C21 (the strip keeps focus through a reload) |
//! | 44–46 Renaming a folder whose column is open | C5 and Strata (the host keeps the column under its new name) |
//! | 47 The header shows the name middle-ellipsized, centred with its actions | PNG goldens `c2-*` |
//! | 48 Header actions show only in the targeted column | `header-actions-target`, *partial*: the actions are the host's slot |
//! | 49 Empty Trash's sensitivity | Strata policy |
//! | 50 An open menu holds a column's header actions | page `a_secondary_press_on_a_header_targets…`, *partial*: the menu (C21) calls [`ColumnsState::hold_target`] |
//! | 51 Right-clicking a title or blank header opens the folder's menu | `header-right-click`; page `a_secondary_press_on_a_header…`, *partial*: the menu is C21's |
//! | 52 New Folder from a parent's header menu goes in that folder | Strata policy (the menu's intent carries the depth) |
//! | 53 Escape on a header menu returns the arrows to that column | page `a_secondary_press_on_a_header_targets…`, *partial*: the menu is C21's |
//! | 54 Right-clicking a header button opens no folder menu | `header-button-right-click`; page `a_secondary_press_on_a_header…` |
//! | 55 The chooser's header menu | Strata policy (the chooser's menu) |
//! | 56 The header spinner shows only while loading | `spinner-while-loading`, `spinner-hidden-when-listed`; page `a_loading_column_shows…` |
//! | 57 Clicking a title focuses the column, keeps its selection and deeper columns | `title-click`; page `a_click_on_a_title…` |
//! | 58 A dialog or filter opened before a deferred refocus keeps focus | N/A by design: the view moves no focus, and its buttons take none, so the strip keeps it (page `after_the_close_button_the_keys_still_reach_the_strip`) |
//! | 59–66 The filter field | C18 and Strata (the field sits in the header's slot) |
//! | 67 The open path: strong on the active column's folder, faint before | PNG goldens (`RowMarks::active_parent`, `active_path`) |
//! | 68 An empty-space click pulses the parent's row | Deferred (Strata marks it unverified; coordinator ruling) |
//! | 69 Empty space in an inactive column: deeper close, first visible selected | `background-click-inactive`; page `a_click_on_empty_space_in_an_inactive_column…` |
//! | 70 Empty space in the active column clears every column, deeper close | `background-click-active`; page `a_click_on_empty_space_in_the_active_column…` |
//! | 71 The blank strip right of the last column clears the selection | `blank-strip-click`; page `a_click_on_the_blank_strip…`; hidden from assistive technology (`the_blank_strip_is_no_control…`) |
//! | 72 Clicking a heading focuses the column, keeps selection and child | `title-click` |
//! | 73 Empty space keeps the column's scroll | page `a_click_on_empty_space_keeps_the_columns_scroll`, *partial*: a column with blank space below its rows can't scroll here, so the test checks the click lands and a scrolled column keeps its offset |
//! | 74 Shift+wheel scrolls the strip; vertical ticks only the listing | page `a_sideways_wheel…`, *partial*: the vertical half (Rune draft U-C24-7) |
//! | 75 Horizontal touchpad gestures scroll the strip | page `a_sideways_wheel…`, `a_wheel_during_the_reveals_recheck_wins`, `a_wheel_in_the_same_frame_as_a_key_wins` (one step lost, see Known gaps) |
//! | 76 Columns fill the strip's height; the paste destination's header is marked | PNG goldens, *partial*: the active column's header is marked; the destination is Strata's |
//! | 77 The horizontal bar shows no square at its leading edge | PNG `c2-overflow-*` (C24's strip bar, shown) |
//! | 78 Dragging an edge resizes only that column, never below 300 | page `dragging_an_edge_resizes…`; unit `widths_never_draw_below…` |
//! | 79 The release spreads the width to the other columns | C2b |
//! | 80 A press 6 px either side of the line resizes the left column | page `a_press_either_side_of_the_line…` |
//! | 81 Resting 400 ms on an edge shows "Column width", following it | `resize-caption`; page `a_rest_on_a_column_edge…` |
//! | 82 The edge meeting the preview lights its divider line | page `the_edge_of_the_column_beside_the_preview…` (with C9's `light_line`) |
//! | 83, 87 Autofit | C2b |
//! | 84 Drags stop at the visible browser area | page `an_edge_drag_stops_at_the_area…`, *partial*: the area is the host's ([`ColumnsState::set_width_cap`]) |
//! | 85 The cap never goes below a standard column, scaled | page `an_edge_drag_stops…`; unit `widths_never_draw_below…` |
//! | 86 A resize is saved; new columns open at it, open ones keep theirs | page `dragging_an_edge_resizes…`, *partial*: saving is the host's |
//! | 88 A saved width wider than the area opens narrowed; the setting stays | page `a_saved_width_wider_than_the_area…` |
//! | 89, 90 A text-size change narrows; a window shrink doesn't | C2b |
//! | 91 A cancelled edge drag restores the width | page `a_cancelled_edge_drag…` |
//! | 92–107 The size badge and its Appearance item | C2b (coordinator ruling); 97, 106, 107 are Strata's |
//! | 108 A press moving less than the threshold starts no drag | `below-threshold`; page `a_press_that_moves_less…` (C1b's threshold) |
//!
//! `operations/drag-and-drop/column-autoscroll`:
//!
//! | Bullet | Check |
//! | --- | --- |
//! | A1 Within 44 px of the strip's edges the columns scroll | page `a_drag_near_the_strips_edge…` |
//! | A2 Within 44 px of a listing's top or bottom it scrolls | page `a_drag_near_a_listings_bottom…`; unit `a_listing_scrolls_near…` |
//! | A3 Only file-list and `text/uri-list` drags | *partial*: C1e's zones pass on only drags they accept, and a refused one ends the autoscroll (unit `autoscroll_stops_on…`); Rune's binder offers only files, so a refused drag needs X2's |
//! | A4 Speed peaks within 16 px, at 720 and 520 px/s | unit `the_speed_peaks…` |
//! | A5 It starts at 35% and is full after 350 ms | unit `the_speed_starts_at_35_percent…` |
//! | A6 It stops on leaving, dropping, ending, or at the limit | page `a_drag_near_the_strips_edge…`; unit `autoscroll_stops_on…` |
//! | A7 The column under the drag is washed, one autoscroll revealed too | page `the_column_under_a_held_drag…`; unit `the_column_under_the_drag_follows…`; PNG `c2-drop-column-*` |
//! | A8 The wash stays at the limit and clears on leaving | page `the_column_under_a_held_drag…`, `the_wash_clears_when_the_drag_leaves_the_column…` |
//! | A9 A drop on a partly shown column goes there; the strip stays | `drop-on-partly-shown-column`; page `a_drop_on_a_partly_shown_column…`, `a_drop_clears_the_wash_even…` |
//!
//! From other nodes, which C1c and C1d left to the views: `browser/selection`'s Ctrl+A in the
//! focused column (`ctrl-a-active-column`), returning keeps a multi-selection
//! (`left-right-active`), Escape clears only the active column and then closes the deepest
//! (`escape-closes-deepest`), Shift+End from the anchor (`shift-end-from-anchor`);
//! `browser/scrolling`'s page keys in the focused column only (`page-active-column`, *partial*:
//! Rune delivers the keypad's Page Up and Page Down as the same logical keys, so they aren't told
//! apart) and the jumps with no cursor (`home-end-no-cursor`); the keyboard cursor outline in the
//! active column only (`left-right-active`, PNG `c2-keyboard-*`); C1c's opt-in type-ahead in the
//! active column only (page `type_ahead_finds…`); a middle press keeps the active column (page
//! `a_middle_press…`). Perf: `tests/perf.rs`.
//!
//! # Left to other work packages and hosts
//!
//! - Strata: which folder opens and when (clicks, Return, mirroring and its 75 ms wait, Quick
//!   Preview), Back and Forward, deletion, the click on a file, saving the column width, the area
//!   that caps it, 10xer's Escape, the paste destination.
//! - C2b: the size badge (92–107) and the width polish (79, 83, 87, 89, 90).
//! - C5 (rename), C18 (the filter field), C21 (the menus, and [`ColumnsState::hold_target`] while
//!   one is open), C9 (the drawer; the clipped-column reveal button is C9's area, coordinator
//!   ruling), X2 (system drags: P1's start, zone keys, [`ColumnsState::drag_over`] from P1's
//!   motion, [`ColumnsView::drop_target_at`]).
//! - A drag from the blank strip beside the last column starting a band in it
//!   (`browser/selection/marquee`): not built; the blank strip has no collection.
//!
//! # Rune drafts (not filed)
//!
//! Written up for the owner to file; each repro runs headless (`rune::Runtime`). Probes:
//! `strata-spikes/c2-probes`.
//!
//! - **U-C2-1: an ancestor can't follow a drag over its children's drop zones.** Rune tells only
//!   the innermost zone under a file drag that it hovers and moves (`runtime/input/drop.rs`,
//!   `drop_target`), so a strip can't track a drag across the columns inside it, as GTK's
//!   `DropControllerMotion` on a scroller does; C1e forwards its zones' moves instead
//!   (`Collection::on_drag_over`). Repro (probe `p3`): a row with `on_drop_files`,
//!   `on_drop_hover` and `on_drop_move` holding two children with their own; `drag_files` over a
//!   child and move: the row hears nothing until the drag reaches its own space. Ask: a passive
//!   drop-motion observer on an ancestor, heard alongside the target's.
//! - **U-C2-3: `on_layout` reports a layout spring's end, not where the frame draws.** During a
//!   width spring, a sibling's `on_layout` hears its final rect once, while
//!   `ElementInfo::rect` moves for several frames, so an app hit-testing by layout is ahead of
//!   what is drawn. Repro (probe `p6`): two columns in a row; widen the first from 300 to 500 in
//!   an animated commit: the second's `on_layout` hears x 500 at once while its drawn x goes
//!   310, 381, 440, 473 over the next frames. Ask: the drawn rect in `on_layout` (or a drawn
//!   counterpart), or a flag that a spring is moving it.
//!
//! - **U-C2-4: a scoped reveal can't be kept to its own viewport's axis.**
//!   `Tx::scroll_into_view_in(scope, row)` scrolls the nearest vertical viewport around the row
//!   and the nearest horizontal one too, even when that one is outside `scope`, and at once in an
//!   animated commit, so a list inside a horizontally scrolled strip can't reveal its row
//!   without jumping the strip, and the strip can't glide there itself in the same frame.
//!   Repro (probe `p7`): four 300 pt columns in a 900 pt `scroll_x` strip, each a vertical
//!   viewport of virtual rows; `scroll_into_view_in("c3", "row-3-2")` (a row already in view
//!   vertically) puts column 3's drawn x at 900, 600, 600 over the next frames, where the
//!   strip's own `scroll_to` to the same place glides 900, 887, 858, 823. Ask: a reveal limited
//!   to an axis, or to the viewports inside its scope. C2 works around it with C1e's
//!   own-viewport reveals, an offset worked out from the collection's geometry, which gives up
//!   Rune's following of a row that layout moves after the reveal (C2's rows don't move).
//!
//! Drafts this view leans on, already written: C15's `Tx::focus` (focus by key), U-C24-3 (a
//! vertical gesture's latch), U-C24-7 (headless Shift+wheel), R-C4-2 (no word to a drag whose
//! element went: [`ColumnsState::reset`]).

#![warn(missing_docs)]

mod column;
pub mod drag_scroll;
mod fit;
mod intent;
pub mod keys;
mod pointer;
mod reveal;
mod sources;
mod state;
mod strip;
mod view;
pub mod width;

#[cfg(test)]
mod tests;

pub use intent::ColumnsIntent;
pub use sources::ColumnSources;
pub use state::{ColumnState, ColumnsState, Focus, Opening};
pub use view::{ColumnsLabels, ColumnsParts, ColumnsPartsMut, ColumnsView, HeaderCx};
