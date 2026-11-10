# 45. Section headers that stick across virtual rows

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C1f-4, R-K4 |
| Affects | C1e/C1f (grouped collections), C4 (grouped list table) |
| Retires | C1f's stuck-header copy drawn over the viewport's top line, pushed by a live value |
| Checked | By source: a sticky element sticks only within its parent (README, "Sticky headers") at `5d421fd`; `c9cd9df3`'s `El::stuck_reveal` (past the pin) is about a sticky header's child, not sections. |

Paste from here.

---

**Title:** Sticky section headers across `virtual_rows`

A sticky element sticks only within its parent, and a virtual list's rows are the parents, so a
header sticks only if its whole section is one row (README, "Sticky headers"). For a section of
thousands of rows that defeats virtualization.

**Repro** (headless): `virtual_rows` of 10,000 one-line rows in sections of 1,000, each
section's first row holding a `.sticky_top(0.0)` header; scroll into a section: the header
scrolls away with its row.

**Ask.** Sections of a virtual list (a section start per index, or a header builder) whose
header sticks while any of the section's rows show, pushed up by the next.
