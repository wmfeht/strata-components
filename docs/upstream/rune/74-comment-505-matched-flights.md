# 74. Comment on #505: matched flights that start late or arc, and rotation in a beat

| | |
| --- | --- |
| Kind | Comment on [#505](https://github.com/anomalyco/rune/issues/505) (timed curves for presence transitions, matched moves and the press scale) |
| Drafts | U-C23-1, U-C23-2 |
| Affects | C23 (trash flight, shudder) |
| Retires | C23's deviations 1 (a straight path), 2 (stagger by arrival) and 4 (a scale-only shake) |
| Checked | By source at `04b38ae`: `.motion()` takes a spring with no delay, and `Beat` has opacity and scale only. No program needed. |

Paste from here.

---

Two more needs for matched moves, from a file manager's "move to Trash" flight (icons fly from
the list into the Trash button):

- **A start delay and a path.** Two `El::matched` flyers swapped in one commit can differ only by
  their springs: a staggered departure takes a commit per flyer, and an arcing path can't be
  had. A delay on `.motion()` (or a per-element delay for matched moves) and a path bend
  (`matched_arc(height)`) would cover it, with timed curves as asked here.
- **Rotation in a finite beat.** A shudder (±8° with a scale) can't be a `Beat`, which has
  opacity and scale only; `El::rotate` springs on commits instead.
