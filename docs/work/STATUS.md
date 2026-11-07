# Work status

Maintained by the coordinator on `integration`. States: waiting (dependencies not merged), ready, planning, coding, review N, merged, blocked.

| WP | State | Note | Merge commit |
| --- | --- | --- | --- |
| W0 | merged | 3 review rounds; png dev-dep approved; PNG goldens blessed on NVIDIA (lavapipe/CI pending) | d4bcd36 |
| W1 | merged | 1 review round + fixes; Rune binder cannot read URIs before drop (needs Rune or P1) | 6fda6da |
| W1b | merged | contract doc change for C1a: EntryId lives for the process, never reused; rename may change it | 3bd5284 |
| W2 | merged | 1 review round + fixes; row.sidebar/row.palette estimates for K2 to confirm; token-name review by K1/K2/C1e owners pending; PNG goldens on NVIDIA | 341e8cb |
| W3 | planning |  |  |
| S1 | merged | P3: X11 shim conditional on mutter/KWin hand test; Wayland resize/menu blocked on Rune (R1-R3, R7); Hyprland/GNOME/KDE/lavapipe hand test pending | c5794e1 |
| S2 | merged | 3 review rounds; P0 feasible w/o Rune patch on libwayland>=1.23; Ubuntu 24.04 GNOME: DnD unsupported; R1-R5 drafted; Hyprland/GNOME/KDE/Dolphin hand test pending | 702338f |
| S3 | merged | sway Wayland/X11 measured; Hyprland/GNOME/KDE/Firefox hand test pending (QEMU Tier B not run); 3 owner decisions | f5682e4 |
| S4 | review 1 (fixing B1/B2) | onig ParseState !Send and pointer-equality found in review; backend to be re-asked of owner; C10a waits | |
| S5 | merged | 2 review rounds; 16 Rune issue drafts (U1-U17, U4 withdrawn) await owner filing; crate name taken -> owner; release.yml verified locally only (GitHub run pending) | 85ca12b |
| C10a | waiting | | |
| C14a | ready | | |
| C15 | merged | built on Rune primitives (deviation from WP doc); 6 bullets Strata policy, O6/F13/K6 partial gaps (Rune drafts); PNG goldens on NVIDIA | b38d879 |
| C1a | review 2 (final fixes) | plan restarted after W1b; selection as id set + index overlay (100k in <1 ms) | |
| C20a | ready | | |
| C20b | ready | | |
| C20c | ready | | |
| C21 | ready | | |
| C22 | ready | | |
| C23 | ready | | |
| C24 | ready | | |
| C25 | ready | | |
| C5 | ready | | |
| C6 | ready | | |
| C7 | ready | | |
| C8 | ready | | |
| C9 | coding | own split + DrawerState (not SplitView); HostedPreview trait for C10–C14; focus via focus_return stubs | |
| K1 | ready | | |
| K2 | planning | | |
| K3 | ready | | |
| K4 | ready | | |
| P0 | waiting | | |
| P3 | waiting | | |
| P5 | waiting | | |
| P6 | ready | | |
| P7 | ready | | |
| P8 | waiting | | |
| P9 | merged | 1 review round + fixes; ci.yml gains libglib2.0-dev (approved exception); example compiled, not run; no desktop hand test | 54786b4 |
| C10b | waiting | | |
| C11 | waiting | | |
| C12 | waiting | | |
| C13 | waiting | | |
| C16 | ready | | |
| C17 | ready | | |
| C18 | waiting | | |
| C19 | ready | | |
| C1b | waiting | | |
| C1c | waiting | | |
| C1d | waiting | | |
| C1e | waiting | | |
| C26 | waiting | | |
| P1 | waiting | | |
| P2 | waiting | | |
| P4 | waiting | | |
| X1 | waiting | | |
| C14b | waiting | | |
| C2 | waiting | | |
| C3 | waiting | | |
| C4 | waiting | | |
| X3 | waiting | | |
| C20d | waiting | | |
| X2 | waiting | | |
