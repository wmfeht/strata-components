# 46. `on_press` also fires after the element's own drag

| | |
| --- | --- |
| Kind | New issue |
| Drafts | R-C4-1 |
| Affects | C4 (header sort click after a resize or reorder drag), C2 |
| Retires | C4's held-back sort click cleared in `ListTable::build` |
| Checked | Repro run at `5d421fd` and `04b38ae`: same output (`e96c55ee`'s press-run rule past the pin doesn't change it). |

Paste from here.

---

**Title:** An element with `on_drag` and `on_press` hears a press after its own drag past the threshold

`pointer_up` queues `Press` whenever the release is inside the captured element
(`runtime/input/pointer.rs:324` at `5d421fd`), so a header that both sorts on a click and
resizes or reorders on a drag also sorts at the end of every drag.

**Repro** (headless; ran at `5d421fd` and `04b38ae`):

```rust
use rune::{Cx, Drag, El, Env, Runtime, col, el};
#[derive(Default)] struct S { log: Vec<String> }
fn view(_: &S, _: &mut Cx<S>) -> El {
    col().w_full().h_full().child(el().key("header").size(300.0, 40.0)
        .on_drag(|s: &mut S, d: &mut Drag, _| s.log.push(format!("drag {:?}", d.phase)))
        .on_press(|s: &mut S| s.log.push("press".into())))
}
fn main() {
    let mut rt = Runtime::new(S::default(), view, (300.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.pointer_move(50.0, 20.0);
    rt.pointer_down(50.0, 20.0);
    rt.pointer_move(75.0, 20.0);
    rt.pointer_move(100.0, 20.0);
    rt.pointer_up(100.0, 20.0);
    rt.frame(0.016);
    println!("{:?}", rt.state.log);
}
```

Output: `["drag Down", "drag Move", "drag Move", "drag Up", "press"]`.

**Ask.** No press after a drag that moved past the threshold, or a flag on `Drag` (at `Up`)
saying a press will follow.
