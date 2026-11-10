# 35. Selection across several texts: `select_text`, triple-click, an empty end, links

| | |
| --- | --- |
| Kind | New issue |
| Drafts | R1, R5, R6 (C11); U-C10b-6, U-C10b-7 |
| Affects | C11 (document view), C10b (code view) |
| Retires | C11's D4 (a find match across units selects its first part) and D9; C10b's D11 and D12 |
| Checked | By source: `Tx::select_text(key, range)` takes one text (`el/tx.rs:318` at `5d421fd`). The repros below are C10b's and C11's probes at the pin; not re-run here. |

Paste from here.

---

**Title:** Selection across texts: code can't select a range spanning two texts, a triple-click stops at one text, and a drag that starts past a line's end loses it

Context: a document or code preview draws one logical line or paragraph as several texts
(virtual rows, a long line split for layout), inside one selection group.

**What happens** (`5d421fd`):
1. `Tx::select_text(key, range)` selects within one text, so a find match across two virtual
   rows can't be selected from code.
2. A triple-click selects one text's paragraph, even inside a selection group. Repro: a
   selection group of two selectable texts "aaa" and "bbb" side by side in a row; triple-click
   "aaa": `Runtime::selected_text()` is "aaa".
3. A selection's report leaves out a start text whose selected part is empty. Repro: a group of
   two selectable texts "one" and "two" in a column; press right of "one"'s end and drag into
   "two": `TextSelection::texts()` holds "two" alone, so the line break after "one" is lost from
   the copy.
4. A press on a link inside selectable text is the link's: a double-click on it selects no word,
   and a Shift-click doesn't extend the selection.
5. The README says "a virtual row scrolling out drops the selection it holds" (`README.md:1443`
   at `5d421fd`); the runtime pins both ends and keeps it (the code is right, the README isn't).

**Ask.**
1. A `select_text` from (key, offset) to (key, offset).
2. A way to mark texts as parts of one paragraph (or take the paragraph across texts sharing a
   row key), so a triple-click selects them all.
3. Report the start text with its empty range at its end, as the end text's range is reported.
4. A double-click or Shift-click on a link in selectable text selects, as on plain text.
5. The README matched to the runtime.

Related: (link to 36).
