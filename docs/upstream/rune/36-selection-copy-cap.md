# 36. Copying a selection across unbuilt virtual rows stops at a cap, silently

| | |
| --- | --- |
| Kind | New issue |
| Drafts | R4 (C11) |
| Affects | C11 (document view), C10b (code view) |
| Retires | C11's and C10b's Ctrl+A copy from their own model (and the frame in which Ctrl+C copies Rune's cut report) |
| Checked | By source: `MAX_VIRTUAL_COPY_ROWS = 10_000` and `MAX_VIRTUAL_COPY_BYTES = 4 MiB` (`runtime/virtual/selection.rs:21, 25`) at `5d421fd` and `04b38ae`. C11's probe P3 at the pin (10,057 of 12,000 texts); not re-run here. |

Paste from here.

---

**Title:** A selection across unbuilt virtual rows is cut at 10,000 rows or 4 MiB, and nothing says so

**What happens** (`5d421fd`). Copying or reading a selection that spans virtual rows not built
stops at 10,000 rows or 4 MiB (`MAX_VIRTUAL_COPY_ROWS`, `MAX_VIRTUAL_COPY_BYTES`,
`runtime/virtual/selection.rs:21, 25`): `selection_copy` sees a cut list, and nothing says it was cut
(a select-all over 12,000 texts reported 10,057).

**Ask.** Mark a cut report (`TextSelection::truncated()`), or let `selection_copy` ask the app
for the rows it didn't build.
