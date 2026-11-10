# 57. rune-kit: export the Combobox fuzzy matcher, with match positions

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U17 (S5, from W1) |
| Affects | W1's `contract::fuzzy`, C17, C19, C20c, C20d |
| Retires | The port of `score` that also records matched ranges |
| Checked | `fn score` is private at `5d421fd` (`rune-kit/src/combobox.rs:44`) and `04b38ae` (`:108`, after `1ea05bc7` made the combobox a state machine). No program needed. |

Paste from here.

---

**Title:** rune-kit: export the Combobox fuzzy matcher, with match positions

A file manager's command palette, settings search and shortcut reference rank results as
`Combobox` does and bold the matched characters. rune-kit's ranking is the private
`combobox::score` (`rune-kit/src/combobox.rs:108` at `04b38ae`). It returns a score but not which
characters matched, so apps copy it to rank the same way.

**Ask.** A public `rune_kit::fuzzy_match(query, text) -> Option<FuzzyMatch { score, ranges }>`,
the one Combobox uses, with the matched character ranges, so a list filtered with it orders
exactly as a Combobox does.
