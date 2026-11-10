# 48. No "press outside" for a field

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C5-2 |
| Affects | C5 (inline rename), C6 (path bar: a press outside cancels a frame later) |
| Retires | C5's D4 list of six presses that keep the field focused, each turned into a host commit |
| Checked | By source at `5d421fd` and `04b38ae`: no `on_press_outside` or capture-phase press. C5's probe at the pin; not re-run. |

Paste from here.

---

**Title:** A field can't hear a press outside it that leaves it focused

Context: a file manager's inline rename ends when the user clicks elsewhere, as a GTK
capture-phase click or a web `pointerdown` listener on the document can.

**What's missing** (`5d421fd`, unchanged at `04b38ae`). A press that leaves a field focused
tells it nothing: a `preserve_focus_on_press` control, a link, a `keeps_text_selection` element,
part of another text box, a press that dismisses a modal, or any press with another button.
Repro: a focused `text_input` beside a `preserve_focus_on_press(true)` button; press the button,
or right-click beside the field: no handler of the field's runs, and it stays focused.

**Ask.** `El::on_press_outside(|s, press, tx|)` for an element and its subtree, any button.

Related: #106 ((link to 09)).
