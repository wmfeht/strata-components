# 25. The double-click interval is fixed at 0.5 s on Linux

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U13 (S5, from S1 R4) |
| Affects | P5, C1b (click runs), C4 (fit-to-content double-click), X1 (title-bar double-click) |
| Retires | C1b's own click-run counting with the host's double-click time; C4's deviation "fit at Rune's 0.5 s" |
| Checked | Repro run at `5d421fd` and `04b38ae`: same output. `window/create.rs:86–92` sets it on macOS and Windows only (both revs). |

Paste from here.

---

**Title:** Linux: the double-click interval is fixed at 0.5 s

**Repro** (headless; ran at `5d421fd` and `04b38ae`):

```rust
use rune::{Cx, El, Env, Runtime, col, el};
#[derive(Default)] struct S { doubles: u32 }
fn view(_: &S, _: &mut Cx<S>) -> El {
    col().w_full().h_full().child(el().key("dbl").size(200.0, 60.0).on_press(|_: &mut S| {}).on_double_press(|s: &mut S| s.doubles += 1))
}
fn main() {
    for gap in [0.45, 0.55] {
        let mut rt = Runtime::new(S::default(), view, (300.0, 200.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let (x, y) = rt.find("dbl").unwrap();
        rt.pointer_move(x, y);
        rt.pointer_down(x, y); rt.pointer_up(x, y); rt.frame(gap);
        rt.pointer_down(x, y); rt.pointer_up(x, y); rt.frame(0.016);
        println!("presses {gap} s apart: doubles {}", rt.state.doubles);
    }
}
```

Output:

```
presses 0.45 s apart: doubles 1
presses 0.55 s apart: doubles 0
```

Under sway with GNOME's `double-click` set to 1200 ms, presses 583–1033 ms apart never pair
(measured at `78d22ed`). macOS and Windows read the system value when a window is created
(`window/create.rs:86–92`); Linux doesn't. `Runtime::double_click_interval` is public, but a
windowed app has no `Runtime`. The interval also drives `.window_drag()`'s double-click
maximise, word and line selection, and `Drag::clicks`.

**Ask.** Read the desktop's interval on Linux (the Settings portal's
`org.gnome.desktop.peripherals.mouse double-click`, else GSettings, else `kdeglobals`'
`DoubleClickInterval`), or add `App::double_click_interval(secs)` and a `Tx` setter.

Related: (link to 24).
