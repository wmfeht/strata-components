# 05. Linux `drag_out`, with several MIME types, actions and an element snapshot

| | |
| --- | --- |
| Kind | New issue, linking #171 (S5 owner question 5: new issue rather than a comment) |
| Drafts | U1 (S5, from S2 R1) |
| Affects | P1, P0, X2 |
| Retires | P1's `wl_data_device` and XDND drag source, its second-`Runtime` drag icon |
| Checked | `drag_out/mod.rs:29` `SUPPORTED = cfg!(target_os = "macos")` at `5d421fd` and `04b38ae`. The headless half ran at `5d421fd` and `04b38ae` (same output); the windowed half is S2's sway run at `78d22ed`, not re-run. |

Paste from here.

---

**Title:** Linux: `drag_out` does nothing; support several MIME types, actions and an element snapshot

Related: #171 (`drag_out` on Linux, for file paths). This asks for the same drag source with
typed data: a file manager drags `text/uri-list` beside `x-special/gnome-copied-files`,
`application/x-kde-cutselection` and an in-process id, and needs copy, move, link and ask.

**What's missing** (`5d421fd`, unchanged at `04b38ae`): `drag_out::SUPPORTED` is
`cfg!(target_os = "macos")` (`drag_out/mod.rs:29`). `DragItem` carries text and files only, there
is no action, and the live-frame snapshot that macOS uses for the drag image
(`drag_out::snapshot`) is `pub(crate)`.

**Repro.** Headless, run with `--script drag.json --trace t.json`; windowed, press the tile and
drag it out over a file manager.

```json
{"events": [{"t": 0.1, "type": "move", "x": 70, "y": 45}, {"t": 0.15, "type": "down", "x": 70, "y": 45}, {"t": 0.2, "type": "move", "x": 90, "y": 45}, {"t": 0.3, "type": "move", "x": 200, "y": 80}, {"t": 0.4, "type": "up", "x": 200, "y": 80}]}
```

```rust
use rune::{AppSpec, Cx, DragItem, El, col, el};
#[derive(Default)] struct S;
fn view(_: &S, _: &mut Cx<S>) -> El {
    col().w_full().h_full().child(el().key("tile").size(140.0, 90.0).fill(rune::oklch(0.55, 0.15, 260.0))
        .drag_out(|_: &S| Some(DragItem::new().text("hello from rune")))
        .on_drag_start(|_: &mut S, _| println!("on_drag_start"))
        .on_drag_end(|_: &mut S, outcome, _| println!("on_drag_end {outcome:?}"))
        .on_drag(|_: &mut S, d, _| println!("on_drag {:?}", d.phase)))
}
fn main() { rune::run(AppSpec::new("drag", (320.0, 200.0), S::default, view)); }
```

Headless (at `5d421fd` and `04b38ae`), the trace records `drag-out #0/tile` and the program
prints `on_drag_start` then `on_drag_end Cancelled`. Windowed under sway it prints only `on_drag Down`, `Move`s and `Up`:
no system drag, no `on_drag_start` or `on_drag_end` (measured at `78d22ed`).

**Ask.**
- `drag_out` on Wayland (`wl_data_device`) and X11 (XDND).
- `DragItem` with arbitrary MIME types and lazily produced data per type.
- Copy, move, link and ask, and the outcome's action.
- A public snapshot of an element from the live frame, for the drag icon.

**Why.** A downstream crate starts a `wl_data_device` drag from `on_drag` on winit's display
(which needs (link to 01)), and renders the drag icon from a second headless `Runtime`.

Related: #171, #357 (in-app drop targets), (link to 06).
