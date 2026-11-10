# 49. A secondary press on a menu's outside scrim reaches nothing

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C21-6 (and C1e's `secondary_press_at` follow-up) |
| Affects | C21 (context menus), C1e |
| Retires | C21's "a right-click outside an open menu does nothing" (bullet 184 not met) |
| Checked | By source: `secondary_target` asks only the topmost hit and its ancestors (`runtime/input/secondary.rs:9` at `5d421fd`, same at `04b38ae`); `647cbbe2`'s `press_focuses_below` is about focus, not secondary presses. Not run here. |

Paste from here.

---

**Title:** A secondary press on an open menu's outside scrim neither dismisses the menu nor reaches what's under it

A menu's outside-press catcher (rune-kit's `scrim`) has no `on_secondary_press`, and
`secondary_target` (`runtime/input/secondary.rs:9` at `5d421fd`) asks only the topmost hit and
its ancestors. So a right-click outside an open menu does nothing: the menu stays open on its
old target, and the item under the scrim can't open its own menu in the same click (as every
desktop menu does).

**Ask.** Let the scrim pass a secondary press through after dismissing (or dismiss and replay
it), as a primary press outside does.
