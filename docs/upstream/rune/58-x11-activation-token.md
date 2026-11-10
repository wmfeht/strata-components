# 58. X11: `AppHandle::activate(token)` on an open window ignores the startup id

| | |
| --- | --- |
| Kind | New issue (related #22) |
| Drafts | U-P8-1 |
| Affects | P8 (single instance) |
| Retires | P8's note that a forwarded launch may not raise the window on mutter or KWin |
| Checked | By source: `window/lifecycle.rs:382` `let _ = token;` at `5d421fd`; `lifecycle.rs` was rewritten past the pin (308 lines) and not re-read for this. winit 0.30.13's `focus_window` sends `CURRENT_TIME`. The snippet hasn't been run. |

Paste from here.

---

**Title:** X11: `AppHandle::activate(token)` on an open window ignores `DESKTOP_STARTUP_ID`

Context: a single-instance app forwards a second launch's startup id to the running instance,
which raises its window with it.

**What's missing** (`5d421fd`, by source). `open()` drops the token for an open X11 window
(`window/lifecycle.rs:382`, `let _ = token; w.focus();`), and winit's `focus_window` sends
`_NET_ACTIVE_WINDOW` with source 1 and `CURRENT_TIME`. No startup-notification remove is sent;
winit sends one only while it creates a window. mutter and KWin may refuse a timestamp-less
request from an application and only mark the window urgent, and the launcher's busy cursor
stays until it times out.

**Repro.** Under `Xvfb :99`, run a Rune app with its window open and, from another thread, call
`handle.activate("demo_TIME12345")`. Watch the root window:

```rust
// Beside the app: an x11rb client listening on the root window.
use x11rb::{connection::Connection, protocol::{Event, xproto::*}};
let (conn, screen) = x11rb::connect(None)?;
let root = conn.setup().roots[screen].root;
conn.change_window_attributes(root, &ChangeWindowAttributesAux::new()
    .event_mask(EventMask::SUBSTRUCTURE_NOTIFY))?;
conn.flush()?;
loop {
    if let Event::ClientMessage(m) = conn.wait_for_event()? {
        let name = conn.get_atom_name(m.type_)?.reply()?.name;
        println!("{} {:?}", String::from_utf8_lossy(&name), m.data.as_data32());
    }
}
```

Expected, as GTK does: `_NET_ACTIVE_WINDOW` with `data[1]` = 12345 (the id's `_TIME`), and
`_NET_STARTUP_INFO_BEGIN` "remove: ID=demo_TIME12345".

**Ask.** On X11, `activate(token)` for an open window takes the `_TIME` timestamp from the
startup id for `_NET_ACTIVE_WINDOW`, and broadcasts the startup id's "remove" message.

Related: #22 (single instance on Linux).
