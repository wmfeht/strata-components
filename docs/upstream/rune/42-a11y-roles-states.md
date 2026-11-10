# 42. Accessibility: `aria-current`, a scroll bar role, busy, and a two-axis control

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C6-2, U-C10b-3, R-C14b-1, U-C20b-3 |
| Affects | C6 (breadcrumbs), C10b and C24 (scroll bars), C14b (3D model view), C20b (colour plane) |
| Retires | C6's current crumb as a label last in the list, C10b's position bar read as a slider, C14b's spinner label standing for busy |
| Checked | By source at `5d421fd` and `04b38ae`: `a11y/roles.rs` maps no `"scrollbar"`; no `El::current` or `El::busy` builder. No program needed. |

Paste from here.

---

**Title:** Accessibility: no `aria-current`, no scroll bar role, no busy state, and no way to expose a two-axis control

**What's missing** (`5d421fd`, unchanged at `04b38ae`):
- **Current item.** No `aria-current`, so the current breadcrumb or the menu's current folder
  can't say so.
- **Scroll bar.** `a11y/roles.rs` maps no `"scrollbar"`, so a scroll bar with a value reads as a
  slider.
- **Busy.** No builder sets AccessKit's busy, so a picture being rendered is heard only through
  a spinner's label.
- **Two axes.** A saturation and value plane can only be one slider with a value text: no range,
  set-value or increments per axis.

**Ask.** `El::current(kind)`; `"scrollbar"` mapped to `Role::ScrollBar` with the range value and
steps a slider has; `El::busy(bool)`; and guidance (or a builder) for a two-axis control, such as
two child value nodes on one focus stop.

Related: #111.
