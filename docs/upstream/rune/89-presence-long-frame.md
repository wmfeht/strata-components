# 89. Presence motion takes a long frame's whole step

| | |
| --- | --- |
| Kind | New issue |
| Drafts | C15's "a frame longer than 33 ms" gap |
| Affects | C15 (dialog open and close) |
| Retires | C15's known gap for the bullet "A frame longer than 33 ms…" |
| Checked | From C15's notes at the pin: since `aea4606` motion that a late frame starts holds its first pose, but motion already running takes the whole step. Not re-run here. |

Paste from here.

---

**Title:** Running presence motion takes a long frame's whole step; clamp it (to 33 ms, as GTK does)

Since `aea4606`, motion that a late frame starts holds its first pose for the overrun instead of
jumping. Motion already running still takes the whole step, so a dialog's 200 ms open that
meets a 150 ms frame jumps most of the way. GTK clamps a frame's step to 33 ms for its
animations.

**Ask.** Clamp a running transition's step to a maximum (33 ms), or make it an option.
