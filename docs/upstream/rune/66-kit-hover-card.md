# 66. rune-kit `HoverCard`: placements, gap, width, hover on the surface, a role and name

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C22-2 |
| Affects | C22 (folder peek) |
| Retires | C22's computed anchor and its overhang deviation |
| Checked | By source: `Placement` and `HoverCard` (`rune-kit/src/overlay.rs:16, 550` at `5d421fd`); unchanged at `04b38ae` (C22 re-checked at `26769da`). |

Paste from here.

---

**Title:** rune-kit `HoverCard`: no LeftStart, LeftEnd or RightEnd, a fixed gap and width, no hover on the surface, and no role or name

At `5d421fd`, `Placement` (`rune-kit/src/overlay.rs:16`) has no LeftStart, LeftEnd or RightEnd;
the card's gap is 4 pt and its width 200 to 320 pt; the surface takes no hover handler; and
the card has no role or name: assistive technology hears an unnamed group
(`kit-root/{key}-card`, role `group`).

**Ask.** Those placements, a gap and width, hover on the surface, and a role and label (as
Kobalte's `HoverCard.Content` takes ARIA attributes).
