# 11. Comment on #309: edge resize from a borderless window, on Wayland

| | |
| --- | --- |
| Kind | Comment on [#309](https://github.com/anomalyco/rune/issues/309) (resize a borderless window from edges the app draws; filed 2026-11-08). S5 drafted U6 as a new issue before it existed. |
| Drafts | U6, its resize half (S5, from S1 R1) |
| Affects | P3, X1 |
| Retires | P3's "no edge resize on Wayland" known limit (owner's P3 decision), and on X11 the `_NET_WM_MOVERESIZE` sent from a second connection |
| Checked | No `drag_resize_window` or `window_resize` anywhere in `rune/src` at `5d421fd` or `04b38ae`. S1's sway measurement; not re-run. |

Paste from here.

---

Adding Linux evidence from a file manager that draws its own header in a borderless window.

Under headless sway (Wayland and Xwayland), plain winit 0.30.13's
`Window::drag_resize_window(ResizeDirection::East)`, called from a press 3 px inside the right
edge, grows the window by the pointer's 100 px travel. A Rune window can't do it: nothing calls
`drag_resize_window`, `with_raw_handle` yields only the `wl_surface` (and `xdg_toplevel.resize`
needs winit's toplevel and the press serial), so on Wayland there is no workaround at all. On
X11 an app can send `_NET_WM_MOVERESIZE` from a second connection, but that connection can't
release winit's implicit grab.

The proposed `El::window_resize(WindowEdge)` covers it. On Wayland the press has to reach
winit inside the same event (its serial), as `.window_drag()` does. The corner cursors are #145
and #292.
