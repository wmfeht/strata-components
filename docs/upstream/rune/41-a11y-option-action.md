# 41. Route an assistive client's action on an option to its container

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C1e-8 |
| Affects | C1e (every collection: list, icons, columns, table), C20d (icon picker, D10) |
| Retires | "A screen-reader user selects with the keyboard only" in C1e; C20d's D10 |
| Checked | By source: an AccessKit `Click` becomes a press on the node itself only when it is pressable (`a11y/actions.rs:55` at `5d421fd`); nothing forwards it. Same at `04b38ae`. No program needed. |

Paste from here.

---

**Title:** An assistive client's click on a list option does nothing when the container handles input

Context: a file manager's views put every pointer handler on the collection's container, not on
rows (a row's handlers would make each row a hit target, and selection, marquee and drag
thresholds live in the container). Options carry `role("option")`, `focus_selected` and a
label, and the container names the active descendant.

**What's missing** (`5d421fd`, unchanged at `04b38ae`). An assistive client's default action on
an option (AT-SPI's "click", AccessKit `Action::Click`) becomes a press only on a pressable node
(`a11y/actions.rs:55`), and the option has no press handler, so nothing happens. There is no way to route the action to the
container with the option's key.

**Ask.** An `El` hook on the container that hears accessibility actions on its descendants
with the target's key (`on_a11y_action(|s, key, action, tx|)`), or a per-option "action goes to
this ancestor" marker.
