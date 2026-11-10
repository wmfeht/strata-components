# 55. Linux: middle-click paste of the primary selection in text fields

| | |
| --- | --- |
| Kind | New issue (related #483) |
| Drafts | U-C24b-3 (and P2's "Rune has no primary selection") |
| Affects | C24b, C5 (rename field), C6, every text field; P2 |
| Retires | C24b's "no middle-click paste" deviation |
| Checked | By source at `5d421fd` and `04b38ae`: fields take no middle press and Rune has no primary-selection source. Not run here. |

Paste from here.

---

**Title:** Linux: text fields don't paste the primary selection on a middle click, and Rune has no primary selection

On Linux a middle click in a text field pastes the primary selection, and selecting text makes
it the primary selection. Rune's fields take no middle press (a middle tap on a `text_input`
goes to the nearest ancestor with `pointer_buttons([Middle])`), and there is no primary
selection source or reader.

**Ask.** Middle-click paste of the primary selection in text fields on Linux, selected text
offered as the primary selection (`zwp_primary_selection_v1` on Wayland, `PRIMARY` on X11), and
an app API to read it.

Related: #483 (Shift+Insert paste on Linux), (link to 07).
