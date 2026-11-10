# 47. Alt with a letter types the letter into a text field on Linux

| | |
| --- | --- |
| Kind | New issue (related #250) |
| Drafts | U-C19-2 |
| Affects | C18 (footer prompt), C19 (shortcut reference search) |
| Retires | The "Alt+letter types into a prompt" known gap; the prompt's Alt chords |
| Checked | Repro run at `5d421fd` and `04b38ae`: same output. `edit/input.rs:92` inserts any `Key::Text` at the pin. |

Paste from here.

---

**Title:** Linux: Alt+letter types the letter into a focused text field

Rune's editor inserts any `Key::Text`, Alt held or not (`edit/input.rs:92` at `5d421fd`), so
Alt+d types "d" in a focused field, where GTK types nothing and leaves the chord to the window's
accelerators. The window's `on_key` doesn't hear it either.

**Repro** (headless; ran at `5d421fd` and `04b38ae`):

```rust
use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, TextEdit, col, text_input};
struct S { edit: TextEdit, window_keys: Vec<String> }
fn view(s: &S, _: &mut Cx<S>) -> El {
    let k = rune_kit::Kit::neutral(true);
    col().w_full().h_full()
        .on_key(|s: &mut S, ev: &KeyEvent, _| { s.window_keys.push(format!("{:?}", ev.key)); false })
        .child(text_input(&s.edit, k.ty.ui).single_line().key("f").w(200.0).on_edit(|s: &mut S| &mut s.edit))
}
fn main() {
    let mut edit = TextEdit::new("");
    edit.focused = true;
    let mut rt = Runtime::new(S { edit, window_keys: Vec::new() }, view, (300.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.key(KeyEvent { key: Key::Text("d".into()), mods: Mods { alt: true, ..Mods::default() } });
    rt.frame(0.016);
    println!("after Alt+d the field holds {:?}; the window's on_key heard {:?}", rt.state.edit.text, rt.state.window_keys);
}
```

Output: `after Alt+d the field holds "d"; the window's on_key heard []`.

**Ask.** Insert only text the platform produced with Alt spent on it (#250's `consumed_mods`
would tell), and leave an Alt chord whose text is the unmodified key to the app's key handlers.
AltGr text keeps typing.

Related: #250.
