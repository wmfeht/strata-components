# 37. rune-kit `Table`: reusable column sizing, reorder, visibility, sideways overflow, fit

| | |
| --- | --- |
| Kind | New issue (related #331) |
| Drafts | R-K1 to R-K7 (C4), U-C11b-2 |
| Affects | C4 (list table, built on its own collection instead of `Table`), C11b (sheet grid) |
| Retires | C4's and C11b's own tables styled as rune-kit's |
| Checked | By source: `TableColumn` has setters only plus `id()` and `clamp()`, `sized` is private (`rune-kit/src/table.rs:128–132, 661` at `5d421fd`); unchanged at `04b38ae`. No program needed. |

Paste from here.

---

**Title:** rune-kit `Table`: reusable column sizing, column reorder and visibility, sideways overflow, fit to content, and a header over app rows

Context: a file manager's list view needs Table's look over its own rows (its own selection
model, marquee and row drags), and a spreadsheet preview needs ranges and multi-key sorts.

**What's missing** (`5d421fd`, unchanged at `04b38ae`):
1. `TableColumn`'s width, flex, min, max, align and sortable have no getters, and `sized` (which
   sizes a cell to its column, `table.rs:661`) is private.
2. No column reorder by dragging headings, and no column visibility (a hidden set that keeps
   widths); a secondary press on the header reaches no handler.
3. No row groups with sticky section headers (see (link to 45)).
4. Table's rows take presses with `on_press_tx`, so an app can't put Table's header and sizing
   over rows it draws itself.
5. Columns whose minimums exceed the width are clipped; nothing scrolls sideways.
6. A double-click on a header edge resets to the declared width; there's no fit to content. The
   accessors are `fn` pointers, so one app can't build several tables over its own state.
7. No cell ranges, no sort by several keys with the later keys shown, no row-header column that
   stays put sideways.

**Ask.** Getters and a public `TableColumn::size(el, override)`; `Table::on_reorder(from, to)`
with the order and a hidden set in `TableState`, and `on_header_secondary_press`; a public
header element and the resolved widths for app-drawn rows; sideways scrolling below the
minimums; fit to the widest heading or cell on screen; closures for the accessors.

Related: #331 (build on each row element).
