# 13. Comment on #313: the window menu on Wayland too, and `wm_capabilities`

| | |
| --- | --- |
| Kind | Comment on [#313](https://github.com/anomalyco/rune/issues/313) (window menu on X11, tiled states) |
| Drafts | U8 (S5, from S1 R3) |
| Affects | P3, X1 |
| Retires | X1's right-click on the header doing nothing on Linux; buttons shown that the compositor can't honour |
| Checked | `window/chrome.rs:95` `window_menu` is Windows only at `5d421fd`; at `04b38ae` it takes a point but is still a no-op off Windows (`window/chrome.rs:96`). |

Paste from here.

---

From a file manager that draws its own header: on Wayland the window menu is missing as well.
`chrome::window_menu` (`window/chrome.rs:96` at `04b38ae`) does nothing off Windows, so a
right-click on a `.window_drag()` region opens nothing; `xdg_toplevel.show_window_menu` (with the
press serial) is what GNOME and KDE expect, and `_GTK_SHOW_WINDOW_MENU` on X11 where the window
manager advertises it.

Two more pieces would let a custom header match the desktop:
- `xdg_toplevel.wm_capabilities`, so an app hides buttons the compositor can't honour (sway
  advertises only fullscreen; Hyprland fullscreen and maximise).
- The desktop's titlebar double-click action
  (`org.gnome.desktop.wm.preferences action-double-click-titlebar`, also in the Settings
  portal), instead of always maximising (`window/chrome.rs:72–77` at `5d421fd`).
