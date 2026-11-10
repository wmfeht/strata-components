# 54. Comment on #149: what a kit Slider needs, from a file manager's sliders

| | |
| --- | --- |
| Kind | Comment on [#149](https://github.com/anomalyco/rune/issues/149) (rune-kit: no Slider component) |
| Drafts | C20a's #149 comment draft |
| Affects | C20a (slider), C13 (volume and timeline) |
| Retires | C20a's own Slider (with 53) |
| Checked | No rune-kit Slider at `04b38ae`. The Page repro is 53's, run at both revs. |

Paste from here.

---

From a file manager's settings and media sliders (GTK `GtkScale` parity), beyond the issue's
list:

- snap points with a snap distance (GTK's 12 px), and labelled snaps that act as toggles;
- a page size (Page Up and Down are always 10 steps today: (link to 53));
- a free drag beside a stepped keyboard;
- a wheel that is off by default, so the slider's scroll view keeps the wheel;
- `Cancel` beside live and final reports (Escape restores the value);
- sizes that follow the text size.
