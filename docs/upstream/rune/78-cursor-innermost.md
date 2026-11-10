# 78. Inside a draggable parent, a child's cursor never shows

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C12-4 |
| Affects | C12 (PDF text over draggable pages) |
| Retires | C12's text band showing the page's grab cursor |
| Checked | Repro run at `5d421fd` and `04b38ae`: same output. The cursor comes from `primary_target` (`runtime/input/hit.rs:162` at the pin). |

Paste from here.

---

**Title:** The cursor is the press target's, so a hoverable child with its own cursor inside a draggable parent shows the parent's

**Repro** (headless; ran at `5d421fd` and `04b38ae`):

```rust
use rune::{Cursor, Cx, Drag, El, Env, Runtime, col, el};
fn view(_: &(), _: &mut Cx<()>) -> El {
    col().key("page").size(300.0, 200.0).cursor(Cursor::Grab).on_drag(|_: &mut (), _: &mut Drag, _| {})
        .child(el().key("text-band").size(300.0, 40.0).cursor(Cursor::Text).on_hover_change(|_: &mut (), _, _| {}))
}
fn main() {
    let mut rt = Runtime::new((), view, (300.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.pointer_move(100.0, 20.0);
    rt.frame(0.016);
    println!("pointer over the text band (Cursor::Text) inside the page (Cursor::Grab): {:?}", rt.cursor());
}
```

Output: `pointer over the text band (Cursor::Text) inside the page (Cursor::Grab): Grab`.

**Ask.** The cursor of the innermost element under the pointer that sets one.
