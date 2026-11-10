# 32. `El::modal`: say why it was dismissed, and a capture layer that leaves focus where it is

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C20b-2 (the reason half; its `Tx` half is fixed, see the README), U-C24b-4 |
| Affects | C20b (colour picker), C24b (autoscroll shield), C5 (rename commits on blur), C17 |
| Retires | C20b's Escape taken in an ancestor's `on_key` first, and its outside-press report a frame late; C5's "autoscroll commits the rename" note |
| Checked | By source: `ModalSpec`'s dismiss is `Fn(&mut S, &mut Tx)` with no reason at `04b38ae` (`el.rs:1175–1185`; `modal_tx` added in `22aa8cde`). U-C24b-4 from C24's probe at the pin; not re-run. |

Paste from here.

---

**Title:** `El::modal`: tell the dismissal why (Escape or a press outside), and offer a modal that doesn't take focus

**What's missing** (`04b38ae`):
- `El::modal` and `El::modal_tx` (new in `22aa8cde`) call the same dismissal for Escape and for
  a press outside, with no reason. A colour picker that cancels on Escape and keeps the colour on
  a press outside has to catch Escape in an ancestor's `on_key` first.
- `El::modal` moves focus into itself, so a text field that commits on blur commits when any
  modal opens, even one with nothing to type into (an autoscroll shield that only catches
  presses and Escape). Repro: focus a `text_input`, mount a full-window `.modal(false, ..)` with
  no focusable child: the field's `ElementInfo::focused` goes false until the modal closes.

**Ask.**
- `modal_with(|s, why: Dismiss, tx| ..)`, with `Dismiss::Escape` and `Dismiss::Outside`.
- An input-capture layer (presses, Escape) that doesn't take focus, or a modal option that
  leaves focus where it is.

Related: #291 (rune-kit `Dialog` drops a dialog without `on_close`).
