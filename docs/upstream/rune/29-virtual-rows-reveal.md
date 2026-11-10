# 29. `virtual_rows`: `reveal_selected` and `scroll_key_to` miss rows that aren't laid out

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C17-4, U-C12-3 |
| Affects | C17 (search palette), C12 (PDF view) |
| Retires | C17's `Tx::scroll_into_view_in` on each arrow; C12's re-request of a zoom's centre row a frame later |
| Checked | U-C17-4 repro run at `5d421fd` and `04b38ae`: same output. U-C12-3 from C12's probe; not re-run. |

Paste from here.

---

**Title:** `virtual_rows`: `reveal_selected` does nothing for rows built by index, and `scroll_key_to` places only a row already laid out

**What's missing** (`5d421fd`, unchanged at `04b38ae`):
- `runtime/commit/measure.rs:531` checks `focus.reveal_selected` only in the branch for rows
  handed over as children (`virtualize`), not for `virtual_rows`' index-built rows, so a
  newly `focus_selected` row never scrolls into view.
- After a commit that resizes every row far down a list (a zoom), the row under the centre isn't
  mounted at the old offset, and `scroll_key_to` for it does nothing (500 rows, row 300).

**Repro** (headless; ran at `5d421fd` and `04b38ae`):

```rust
use rune::{Cx, El, Env, GrowthSpec, Runtime, ScrollSpec, col, el};
struct S { selected: usize }
fn by_children(s: &S, _: &mut Cx<S>) -> El {
    col().key("vp").size(300.0, 200.0).scroll(ScrollSpec { stick_bottom: false }).child(
        col().key("list").w_full().growth(GrowthSpec::default()).virtualize(20.0).reveal_selected(true)
            .children((0..100).map(|i| el().key(format!("r{i}")).w_full().h(20.0).focus_selected(i == s.selected))))
}
fn by_index(_: &S, _: &mut Cx<S>) -> El {
    col().key("vp").size(300.0, 200.0).scroll(ScrollSpec { stick_bottom: false }).child(
        col().key("list").w_full().growth(GrowthSpec::default()).reveal_selected(true)
            .virtual_rows(100, 20.0, |_: &S, i| format!("r{i}").into(), |s: &S, i| el().w_full().h(20.0).focus_selected(i == s.selected)))
}
fn reveal(name: &str, view: fn(&S, &mut Cx<S>) -> El) {
    let mut rt = Runtime::new(S { selected: 0 }, view, (300.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.update(|s, _| s.selected = 60);
    for _ in 0..60 { rt.frame(0.016); }
    let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("r60")).map(|e| e.rect[1]);
    println!("{name}: row 60 selected, drawn at {row:?}");
}
fn main() {
    reveal("rows as virtualize children", by_children);
    reveal("rows built by virtual_rows ", by_index);
}
```

Output:

```
rows as virtualize children: row 60 selected, drawn at Some(180.0)
rows built by virtual_rows : row 60 selected, drawn at None
```

**Ask.** The same reveal for index-built rows (their keys are known), and `scroll_key_to` that
places a virtual row by key from the commit's layout, as `scroll_into_view` reaches an unmounted
row.
