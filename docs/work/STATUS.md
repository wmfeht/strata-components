# Work status

Maintained by the coordinator on `integration`. States: waiting (dependencies not merged), ready, planning, coding, review N, merged, blocked.

| WP | State | Note | Merge commit |
| --- | --- | --- | --- |
| W0 | merged | 3 review rounds; png dev-dep approved; PNG goldens blessed on NVIDIA (lavapipe/CI pending) | d4bcd36 |
| W1 | merged | 1 review round + fixes; Rune binder cannot read URIs before drop (needs Rune or P1) | 6fda6da |
| W1b | merged | contract doc change for C1a: EntryId lives for the process, never reused; rename may change it | 3bd5284 |
| W1c | merged | contract additions for C1e: collection slot; geometry docs no longer rely on on_drag | 0b52156 |
| W2b | merged | Look::fallback = Strata defaults via K2; slot tokens moved off w2-tokens goldens to w2-slots review scene; confirms row.sidebar 24 / row.palette 53 | 8c12ef3 |
| W1d | merged | contract changes for C1c: UniformRows::page_step = rows that fit minus one (meaning changed, 10->9); Selection::toggle_in_run for Space inside v/V | 21f28e7 |
| C1f | waiting (after C1e merges) | contract addition for C4: Lines spans (group-header lines), per C1e lines.rs design | |
| G1 | merged | owner request: index is rune-kit Buttons grouped by lane; markers from registry (has golden scenes); index goldens hide markers, review scene shows them | 9b4bf5c |
| W2 | merged | 1 review round + fixes; token-name review by K1/K2/C1e owners pending; PNG goldens on NVIDIA | 341e8cb |
| W3 | merged | 3 review rounds + final nits; private headless sway sessions, env-scoped watchdog; Platform CI workflow never run (needs push to main/PR, owner); lavapipe run local only | 5c8ea08 |
| S1 | merged | P3: X11 shim conditional on mutter/KWin hand test; Wayland resize/menu blocked on Rune (R1-R3, R7); Hyprland/GNOME/KDE/lavapipe hand test pending | c5794e1 |
| S2 | merged | 3 review rounds; P0 feasible w/o Rune patch on libwayland>=1.23; Ubuntu 24.04 GNOME: DnD unsupported; R1-R5 drafted; Hyprland/GNOME/KDE/Dolphin hand test pending | 702338f |
| S3 | merged | sway Wayland/X11 measured; Hyprland/GNOME/KDE/Firefox hand test pending (QEMU Tier B not run); 3 owner decisions | f5682e4 |
| S4 | merged | owner: onig + dedicated highlighter thread, N=64, RUSTSEC ignore, WTFPL ok; 3 review rounds + 1 owner-allowed fix round; plan §4 C10/C10a doc edits are owner items | 183f7e2 |
| S5 | merged | 2 review rounds; 16 Rune issue drafts (U1-U17, U4 withdrawn) await owner filing; crate name taken -> owner; release.yml verified locally only (GitHub run pending) | 85ca12b |
| C10a | planned (approval pending resume) | | |
| C14a | review 1 done (fixes pending resume) | B1 Ctrl+wheel zoom direction inverted off macOS (attach on_magnify only on macOS); S1 +/- wrong way at extreme fits; S2 set_content sends no report; S3-S6 | image zoom/pan is new vs Strata (plan asks for it): deliberate change for owner; pixels set without view commits (probed) | |
| C15 | merged | built on Rune primitives (deviation from WP doc); 6 bullets Strata policy, O6/F13/K6 partial gaps (Rune drafts); PNG goldens on NVIDIA | b38d879 |
| C1a | merged | 2 review rounds; select_all 149 us / Shift range 148 us at 100k (release, local only); hosts call before_removal + sync | b8c3b8c |
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
| C8 | merged | 2 review rounds; own list on SidebarRow+Reorder; keyboard pin reorder left out (no bullet); hover-time folder check needs X2 binder; Rune drafts U18-U23; PNG goldens on NVIDIA | 6277a99 |
| C9 | merged | 3 review rounds; own split + DrawerState; HostedPreview trait; focus via focus_return stubs (Rune Tx::focus draft); PNG goldens on NVIDIA | bad5d4c |
| K1 | ready | | |
| K2 | merged | 1 review round + fixes; Strata values at upstream/main 35ee677ab; font list arrives as data (Rune #114/U16); Switch/Button sizes are rune-kit gaps | db41ea7 |
| K3 | ready | | |
| K4 | ready | | |
| P0 | ready | | |
| P3 | ready | | |
| P5 | ready | | |
| P6 | ready | | |
| P7 | ready | | |
| P8 | ready | | |
| P9 | merged | 1 review round + fixes; ci.yml gains libglib2.0-dev (approved exception); example compiled, not run; no desktop hand test | 54786b4 |
| C10b | waiting | | |
| C11 | waiting | | |
| C12 | waiting | | |
| C13 | waiting | | |
| C16 | ready | | |
| C17 | ready | | |
| C18 | ready | | |
| C19 | ready | | |
| C1b | coding (state machine done; adapter waits on C1e merge) | 37 unit tests; PointerHandle shared settings (deviation); no rename timeout (Strata) | |
| C1c | coding (machine done, 51 tests incl. Space-in-range; adapter waits on C1e) | default + 10xer maps; layout-aware chord workaround (U-C1c-1) | |
| C1d | coding (machine done, 17 tests; adapter waits on C1e + C1b adapter) | ~10 Hz apply while auto-scrolling keeps perf meter silent (deviation) | |
| C1e | review 2 done (round 3 = last; fixes pending resume) | all controller seam requests in; scroll p95 0.59 ms vs Table 0.88; S14 sticky group lines deferred to contract WP C1f before C4 | |
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
