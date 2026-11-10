# 27. A scroll request past the mounted rows runs the view; the wheel doesn't

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C24-8 |
| Affects | C24 (scroll bars), rune-kit's own `ScrollBar` |
| Retires | C24's allowance for Rune's "stepping its own glide" warning on a thumb drag |
| Checked | Measured by C24 at the pin (its review probes); not re-run here. `runtime/commit` changed past the pin (`a4069984` decides a commit's kind once); not re-measured at head. |

Paste from here.

---

**Title:** A scroll request that moves a virtual list past its mounted rows commits several times and runs the view; a wheel as far doesn't

A thumb drag on a long virtual list asks for a scroll on every move. Each request that moves
past the mounted rows commits about four times and runs the app's view about three times that
frame, where a wheel delta moving as far commits once and runs no view. Over a long list the
commits trip Rune's own warning ("an instant scroll … on 8 frames in a row, each with a
commit"), and rune-kit's `ScrollBar` trips it the same way.

Measured at `5d421fd`, per frame:

| What moves the list | Commits | View runs |
| --- | --- | --- |
| Our scroll bar's thumb drag | +4 | 3 |
| rune-kit `ScrollBar`, thumb drag | +4 | 3 |
| Wheel jumps of up to 6,000 pt | +1 | 0 |
| `scroll_to_instant` of 400 pt or more | 2–3 | 2–3 |
| `scroll_to_with` past the mounted rows | +4 | 3 |

**Repro** (headless): 1,000 virtual rows with `.scroll_bar_styled(..)`; press the thumb and move
it 25 pt a frame for 12 frames, counting `Runtime::stats.commits` and the view's runs; then
wheel the same distances.

**Ask.** Let a scroll request remount virtual rows as the wheel does, without running the view.
