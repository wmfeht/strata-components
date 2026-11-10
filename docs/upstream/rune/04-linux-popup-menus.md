# 04. Linux pop-up menus: field menus draw nothing, and in-window submenus run off the window

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U14 (S5, from S1 R6), U-C21-1 |
| Affects | C5, C6, C10b, C18, C21, C22, the chooser's minimum size |
| Retires | C21's in-window `FieldMenu` attached to each field; the chooser's minimum-size floor |
| Checked | `popup_menu/mod.rs:38` `SUPPORTED = false` off macOS and Windows at `5d421fd` (`:40` at `04b38ae`). The submenu repro ran headless at `5d421fd` and `04b38ae` (`--tree`): the same rects at both. |

Paste from here.

---

**Title:** Linux: `popup_menu` is unsupported, so a text field's menu shows nothing, and in-window submenus can't leave the window

Context: a file manager on Rune: context menus with an "Open With" submenu, a small file
chooser window, and text fields everywhere.

**What's missing** (`5d421fd`, unchanged at `04b38ae`):
- `popup_menu_supported()` is false off macOS and Windows (`popup_menu/mod.rs:38`). A
  right-click in a text field with no `on_secondary_press` queues the native field menu (Cut,
  Copy, Paste, Select All); on Linux it answers `None` and nothing shows. Headless `--trace`
  logs `popup-menu:open`, so tests see a menu no Linux user does.
- In-window menus scroll to fit, but rune-kit submenus only flip and shift. In a 420 × 400
  window (Rune's default minimum), an 8-row "Open With" submenu sits at `[288, 90, 160, 228]`,
  28 pt past the right edge; a 20-row one shifts to `[288, 8, 160, 564]` and runs 172 pt past
  the bottom (the program below, at `5d421fd` and `04b38ae`).

**Repro** (headless, rune-kit at the same rev; run with `--tree t.json --size 420x400 --at 1`,
then again with `--long`, and read the `ctx-item1-sub-menu` rect):

```rust
use rune::*;
use rune_kit::{Kit, MenuEntry, MenuItem, context_menu};
struct S;
fn view(_: &S, _: &mut Cx<S>) -> El {
    let k = Kit::neutral(true);
    let rows = if rune::tooling::extra_args().iter().any(|a| a == "--long") { 20 } else { 8 };
    let sub = (0..rows).map(|i| MenuEntry::item(MenuItem::new(format!("App {i}")))).collect();
    let mut layer = Vec::new();
    let trigger = context_menu("ctx", el(), Some(PointerPress { x: 130.0, y: 60.0 }))
        .item(MenuItem::new("Open"))
        .item(MenuItem::new("Open With").submenu(sub, true))
        .item(MenuItem::new("Copy"))
        .build(&k, &mut layer);
    rune_kit::root(col().w_full().h_full().fill(k.c.bg_base).child(trigger), layer)
}
fn main() { App::new(AppSpec::new("menus", (420.0, 400.0), || S, view)).run(); }
```

**Ask**, either of:
- Menus, submenus and anchored popovers as `xdg_popup` surfaces on Wayland (override-redirect
  windows on X11), with the native field menu among them; or
- where native pop-ups aren't available, an in-window fallback: Rune draws the field menu itself
  (or hands the app the request: field, point and Rune's items, e.g.
  `AppSpec::on_field_menu(|s, request, tx|)`), and submenus shrink and scroll as menus do.

**Why.** Today every text field needs an app-drawn menu attached by hand, rune-kit fields can't
take one at all ((link to 03)), and a menu's submenu is clipped in small windows.

Related: #255 (overlays ignore right clicks on Windows and macOS).
