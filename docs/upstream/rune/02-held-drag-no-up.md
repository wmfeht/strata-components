# 02. A held drag hears no `Up` when its element leaves or a modal mounts

| | |
| --- | --- |
| Kind | New issue |
| Drafts | R-C4-2, U-C10b-8, U-C24-4 (and C11b's D11, C2's `ColumnsState::reset`, P1's capture paths 2 and 4) |
| Affects | C2, C4, C10b, C11b, C24, P1 |
| Retires | `stop()`/`reset()` calls hosts must make on a view switch, C10b's "bar wasn't laid out" check, C24's `Bar::release` |
| Checked | Repro run at `5d421fd` and `04b38ae`: same output. At head `runtime/input/modal_focus.rs:130` still drops the press with no `Up`, and nothing in `runtime/commit` reads the held press. |

Paste from here.

---

**Title:** A drag whose element is removed, or under a modal that mounts, never hears `Phase::Up`

Context: Strata (lgse/strata), a file manager on Rune (column resizes, scroll bar thumbs, range drags).

**What's missing.** Escape mid-drag ends a drag with a cancelled `Phase::Up`
(`cancel_drag`, `runtime/input/pointer.rs` at `5d421fd`). Two other ends deliver nothing:
- the element holding the drag leaves the tree (a view switch, a preview closing);
- a modal mounts while the drag is held: `ModalEffect::CancelGestures` drops the press
  (`runtime/input/modal_focus.rs:130` at `04b38ae`).

State the handler set at `Down` (a highlighted thumb, a resize in progress) then stays set.

**Repro** (headless; ran at `5d421fd` and `04b38ae`):

```rust
use rune::{Cx, Drag, El, Env, Key, KeyEvent, Mods, Runtime, col, el};
#[derive(Default)] struct S { show: bool, modal: bool, log: Vec<String> }
fn view(s: &S, _: &mut Cx<S>) -> El {
    let mut root = col().w_full().h_full();
    if s.show {
        root = root.child(el().key("tile").size(100.0, 60.0).on_drag(|s: &mut S, d: &mut Drag, _| {
            s.log.push(format!("{:?}{}", d.phase, if d.cancelled() { " cancelled" } else { "" }))
        }));
    }
    if s.modal {
        root = root.child(col().key("modal").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).modal(false, |_: &mut S| {}));
    }
    root
}
fn run(name: &str, mid: impl FnOnce(&mut Runtime<S>)) {
    let mut rt = Runtime::new(S { show: true, ..S::default() }, view, (300.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let (x, y) = rt.find("tile").unwrap();
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 20.0, y);
    rt.frame(0.016);
    mid(&mut rt);
    rt.frame(0.016);
    rt.pointer_move(x + 40.0, y);
    rt.pointer_up(x + 40.0, y);
    rt.frame(0.016);
    println!("{name}: {:?}", rt.state.log);
}
fn main() {
    run("Escape mid-drag", |rt| rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() }));
    run("element removed", |rt| rt.update(|s, _| s.show = false));
    run("modal mounted  ", |rt| rt.update(|s, _| s.modal = true));
}
```

Output:

```
Escape mid-drag: ["Down", "Move", "Up cancelled"]
element removed: ["Down", "Move"]
modal mounted  : ["Down", "Move"]
```

**Ask.** End the drag with a cancelled `Phase::Up` (as Escape does) when its element is removed,
starts exiting, or a modal takes the pointer.

**Why.** Every downstream drag (column resize, scroll bar thumb, cell range, a drag that
starts a system drag) needs its own "did my element go?" check, or a call from the host on
every view switch. Missing one leaves a bar shown and tinted until the next press.

Related: the public cancel asked in (link to 08).
