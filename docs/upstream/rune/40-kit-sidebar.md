# 40. rune-kit `Sidebar` and `Reorder`: a rail form, focus without pressing, sections, keyboard moves

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U18, U19, U20, U21, U22 (C8) |
| Affects | C8 (places sidebar) |
| Retires | C8's own list, headings, section and icon-only rail on `SidebarRow`, its parked reorder move |
| Checked | By source at `5d421fd`: `focus_follows()` on `sidebar-list` (`rune-kit/src/shell/sidebar.rs:133`), `SidebarHeader` `focusable(false)` (`:239`), `Reorder::follow` private (`shell/reorder.rs:36`). Unchanged at `04b38ae`, except that `cbc93701` makes a reorder let go of a lost release. |

Paste from here.

---

**Title:** rune-kit `Sidebar`: no icon-only rail or compact step, headers that can't take focus, collapsed sections that drop the open row, and `Reorder` without keys or cancel

Context: a file manager's places sidebar (bookmarks, devices, a pending-unmount spinner) with a
compact icon-only rail and drag-to-reorder bookmarks.

**What's missing** (`5d421fd`):
1. `SidebarRow` has no icon-only form and no tooltip: with an empty label it keeps the label slot
   and gap, and draws an ellipsis. `SplitView`'s sidebar knows only open and closed.
2. The list presses the row focus reaches (`focus_follows`, `sidebar.rs:133`), and
   `SidebarHeader` takes no focus (`focusable(false)`, `:239`). A file manager opens a place
   only on Enter or a click, and steps onto headings.
3. A collapsed `SidebarSection` drops every row, with no way to keep the open item's row in view.
4. `Reorder` has no keyboard move; a drag past the end clamps with no way to cancel; a middle
   button reorders too; `Reorder::follow` is private; `SidebarRow::reorder` takes a plain `fn`
   accessor; its drop runs with no `Tx`; and a drag released off the row ends with no press.
5. `SidebarRow`'s icon box is fixed at 16 pt and its row inset at a private 8; the selected fill
   is fixed to `c.pressed`; the busy spinner is `icon_muted`; it names the row by its label only;
   and `over` isn't public.

**Ask.** An icon-only rail form with tooltips and a compact `SplitView` step; focusable headers
and an option not to press on focus; a collapsed section that can keep one row; keyboard moves,
cancel, primary-only drags, a public `follow`, closures and a `Tx` for `Reorder`; the icon box,
inset, selected fill, spinner colour and accessible name as options.

Related: #396.
