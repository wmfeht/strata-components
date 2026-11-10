# 80. Comment on #350: inline images in read-only rich text

| | |
| --- | --- |
| Kind | Comment on [#350](https://github.com/anomalyco/rune/issues/350) (inline images as part of a text run) |
| Drafts | R3 (C11) |
| Affects | C11 (document view: inline images in Markdown and documents) |
| Retires | C11's images as blocks of their own |
| Checked | By source at `5d421fd` and `04b38ae`: inline boxes exist for edit-field pills only. No program needed. |

Paste from here.

---

The same need in a read-only document preview: images inside a paragraph (badges in a README,
inline figures in a document) can't sit in the text run, and inline boxes exist for edit-field
pills only. The selection and copy behaviour asked here (copy the image's text or alt) is what
a document view needs too.
