# 86. Off macOS, Ctrl+wheel over `on_magnify` becomes an inverted pinch with no fallback

| | |
| --- | --- |
| Kind | New issue |
| Drafts | C14a's "Ctrl+wheel turned into a pinch zooms the wrong way" draft |
| Affects | C14a (image viewport), C12 (PDF zoom) |
| Retires | C14a's `on_magnify` on macOS only, with Ctrl+wheel handled in `on_wheel` elsewhere |
| Checked | By source: `MagnifyEvent::new(-dy / PINCH_WHEEL)` (`runtime/input/gesture.rs:106` at `5d421fd`, `:113` at `04b38ae`). Not run here. |

Paste from here.

---

**Title:** Off macOS, a Ctrl+wheel over any `on_magnify` (ancestors included) becomes a pinch: inverted, without modifiers, and with no fallback

`pinch_wheel` (`runtime/input/gesture.rs:106` at `5d421fd`) turns a Ctrl+wheel over an
`on_magnify` handler into `MagnifyEvent::new(-dy / 200)`:
- the zoom is inverted against the wheel convention (a wheel turned away zooms out);
- the event has no modifiers, so Ctrl+Shift+wheel can't be told from a pinch;
- a declined pinch doesn't fall back to the wheel;
- it applies to an `on_magnify` anywhere up the chain, so an ancestor's zoom takes Ctrl+wheel
  from a viewport inside it.

On Linux winit 0.30.13 delivers no pinch at all, so this conversion is the only zoom path there.

**Ask.** Keep the wheel's direction, carry the modifiers, fall back to `on_wheel` when the pinch
is declined, and limit it to the innermost handler.
