# 96. Comment on #354: a text-field menu's Undo and Redo

| | |
| --- | --- |
| Kind | Comment on [#354](https://github.com/anomalyco/rune/issues/354) (text fields: expose whether undo and redo are available) |
| Drafts | U-C21-3 |
| Affects | C21 (field menu) |
| Retires | C21's undo of a cloned `TextEdit` when the menu opens, to learn whether Undo would do anything |
| Checked | By source: no `can_undo` or `can_redo` at `5d421fd` or `04b38ae`. |

Paste from here.

---

The same need for an app-drawn text-field context menu (Linux has no native one). Today we
clone the field's `TextEdit` when the menu opens and undo the clone to see whether Undo would
change anything. `TextEdit::can_undo()` and `can_redo()` would replace that.
