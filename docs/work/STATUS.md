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
| C1f | merged | 2 review rounds; groups via CollectionState::set_groups (call before enter/sync; wrong order fails silently: doc nit open); coded a4a0cb5; stuck label built: 1 view run per group crossed, Rune draft U-C1f-6; Groups + stuck overlay header; on_press hook; C1e carry-overs; header = row height, page step leaves a row: deviations | contract addition for C4: Lines spans (group-header lines); also C1e round-3 SF1 + nits | 99f4863 |
| W2c | merged | 1 review round, no blockers; contract additions for K1: Look::fill {muted, highlight}; LookParams keeps glow on/animations; no golden moves | 07eb69c |
| K1b | waiting (after C10a) | syntect Theme from strata-kit SCOPES; replaces C10a interim theme_from_look; carries K1 should-fixes: kit.alpha(c, c.a*x) for translucent theme colours + test; port Strata's extreme-palette Contrast test; theme::switch honours look.animations; Role::Error drops danger background (note) | |
| W2d | ready (small contract follow-up from K3 review 2) | IconName additions/rulings: ClosePanel (panel-right-close) for C9 drawer close; an arrow-down name for C9 find next (and use Up/arrow-up for prev); ruling on device icons (Usb/Optical/Phone vs Strata drawing every device as a drive); Computer vs root crumb; then C9 adopts | |
| G1 | merged | owner request: index is rune-kit Buttons grouped by lane; markers from registry (has golden scenes); index goldens hide markers, review scene shows them | 9b4bf5c |
| W2 | merged | 1 review round + fixes; token-name review by K1/K2/C1e owners pending; PNG goldens on NVIDIA | 341e8cb |
| W3 | merged | 3 review rounds + final nits; private headless sway sessions, env-scoped watchdog; Platform CI workflow never run (needs push to main/PR, owner); lavapipe run local only | 5c8ea08 |
| S1 | merged | P3: X11 shim conditional on mutter/KWin hand test; Wayland resize/menu blocked on Rune (R1-R3, R7); Hyprland/GNOME/KDE/lavapipe hand test pending | c5794e1 |
| S2 | merged | 3 review rounds; P0 feasible w/o Rune patch on libwayland>=1.23; Ubuntu 24.04 GNOME: DnD unsupported; R1-R5 drafted; Hyprland/GNOME/KDE/Dolphin hand test pending | 702338f |
| S3 | merged | sway Wayland/X11 measured; Hyprland/GNOME/KDE/Firefox hand test pending (QEMU Tier B not run); 3 owner decisions | f5682e4 |
| S4 | merged | owner: onig + dedicated highlighter thread, N=64, RUSTSEC ignore, WTFPL ok; 3 review rounds + 1 owner-allowed fix round; plan §4 C10/C10a doc edits are owner items | 183f7e2 |
| S5 | merged | 2 review rounds; 16 Rune issue drafts (U1-U17, U4 withdrawn) await owner filing; crate name taken -> owner; release.yml verified locally only (GitHub run pending) | 85ca12b |
| C10a | review 3 (final; round 2 fixed: 61c4d03; gallery rune-kit markdown feature, Vim modelines, editor goldens; perf p95 2.4-6.96 ms) | | |
| C14a | merged | 3 review rounds; Ctrl+wheel follows Rune's dy sign, on_magnify macOS only; scale-only change resynced via rescale child (Rune draft); script runner passes --scale 2 (sanctioned); round-3 should-fix (rescale key by target scale) carried to C14b; image zoom/pan is new vs Strata (plan asks for it): deliberate change for owner; pixels set without view commits (probed) | 86a335e |
| C15 | merged | built on Rune primitives (deviation from WP doc); 6 bullets Strata policy, O6/F13/K6 partial gaps (Rune drafts); PNG goldens on NVIDIA | b38d879 |
| C1a | merged | 2 review rounds; select_all 149 us / Shift range 148 us at 100k (release, local only); hosts call before_removal + sync | b8c3b8c |
| C20a | ready | | |
| C20b | ready | | |
| C20c | ready | | |
| C21 | ready | | |
| C22 | ready | | |
| C23 | ready | | |
| C24 | coding | own overlay scrollbar instead of rune-kit ScrollBar (it collides across collections and can't sit outside C4's scroll_x: deviation from plan §4, owner edit); small C1e edit (edge part + on_scroll); modal autoscroll shield (wheel/keys blocked while running); follow-ups: row-margin middle press (C1b/C1e), retrofit bars + popover wheel bullets (X1) | |
| C25 | planning | | |
| C5 | ready | | |
| C6 | ready | | |
| C7 | ready | | |
| C8 | merged | 2 review rounds; own list on SidebarRow+Reorder; keyboard pin reorder left out (no bullet); hover-time folder check needs X2 binder; Rune drafts U18-U23; PNG goldens on NVIDIA | 6277a99 |
| C9 | merged | 3 review rounds; own split + DrawerState; HostedPreview trait; focus via focus_return stubs (Rune Tx::focus draft); PNG goldens on NVIDIA | bad5d4c |
| K1 | merged | 1 review round, no blockers; root typos.toml exception for a catalogue theme name (sanctioned); catalogue copied from Strata (Tinted Theming MIT + LGSE MIT notices); toml dep; Look::fallback stays neutral; syntect theme split to K1b; should-fixes (theme alpha multiplies, extreme-palette Contrast test) carried to K1b | 26fd004 |
| K2 | merged | 1 review round + fixes; Strata values at upstream/main 35ee677ab; font list arrives as data (Rune #114/U16); Switch/Button sizes are rune-kit gaps | db41ea7 |
| K3 | merged | 2 review rounds; 68 goldens re-blessed incl. 4 c1-groups added after the plan; Help is own art: circle-help absent at Lucide 1.35.0; open for a W2 follow-up (W2d): C9 drawer close should be panel-right-close (new name), C9 find prev/next should be arrows (no arrow-down name), device rule (Usb→drive vs Optical/Phone own icons), Computer→monitor vs Strata root crumb hard-drive, generator accepts stray text/entities | Strata has one vocabulary (Lucide UI + Catppuccin code files) and a colouring choice (Brand Mocha/Latte vs Theme accent): three IconSets; Strata icon stems + IconName::id aliases; gap icons fetched from Lucide 1.35.0 (read-only); re-blesses ~66 goldens that draw semantic icons; GTK-only bullets proposed N/A (owner) | 63d5dfa |
| K4 | review 1 (coded: 31 unit + 5 gallery tests; no padding migrates, none one-to-one) | TOML overrides keyed by registry token names with [light]/[dark] scopes, salvage parse with line/column errors, colour references + alpha(); CSS migration from style.css; no icon overrides (plan §10 q2, owner); K5 bullets proposed N/A (owner) | |
| P0 | review 4 (final; OWNER allowed a 4th round; round 3 B1 fixed: post-drop set_actions refused unless it settles an ask, model-checked; x11rb moved to a shared block; Platform CI on ubuntu-26.04, unverified on GitHub) | owner: S2 Q1 = a (degrade on libwayland 1.22; own-connection data-control route); owner: move Platform CI to libwayland >= 1.23 in P0 (unverified until a push); S2 Q2/Q3 deferred to P1 | |
| P3 | merged | 2 review rounds, no blockers; owner: borderless on Wayland now with NO edge resize until Rune U6, no minimise (U7) or window menu (U8); X11 borderless under Auto via shim (mutter/KWin hand test pending); no in-app resize; corners Cursor::Move; header component folded into X1; Done-when narrowed (deliberate parity regressions recorded); open should-fixes: Watcher::drop joins even when the fd dup fails (UI hang on EMFILE: detach instead), resize test asserts sway's rect not X geometry; hand tests pending (Hyprland, GNOME, KDE, mutter/KWin X11) | 23d95af |
| P5 | merged | 3 review rounds; ruling: a key a restarted portal lacks resets to default; covered window: state current, view waits for uncover, #139; tokio/zbus moved to a shared optional block (each feature enables its own sub-features); open should-fix: no test for "bus gone → NoBus once" (harness can't kill its dbus-daemon: W3 helper); hand tests pending (GNOME, Hyprland portal-gtk, KDE) | zbus 6 portal reader (bare workspace deps shared with P8); reads 4 extra keys (drag threshold, recent files, blink timeout, reduced motion); no GSettings/XSETTINGS/kdeglobals fallback (X11 without portal gets defaults); GTK-only text-scaling bullets proposed N/A (owner) | d2f7be0 |
| P6 | ready | | |
| P7 | ready | | |
| P8 | merged | 3 review rounds; U-P8-1 text in S5 appendix; 11 unit + 17 session tests incl. GLib interop; open should-fixes: refuse delivery once release starts (launch in the ReleaseName gap), release() wait to cover the drain (2x bus_timeout), held-launch test assert < 2 s; hand tests pending (Hyprland, GNOME, KDE, X11 focus; GTK Strata interop) | GApplication-compatible protocol; opt-in FileManager1 Reveal in the shim (deviation from WP doc: policy stays in Strata); zbus 6 + tokio deps (already locked); zbus types exposed (major bump = minor release) | 5c2b38a |
| P9 | merged | 1 review round + fixes; ci.yml gains libglib2.0-dev (approved exception); example compiled, not run; no desktop hand test | 54786b4 |
| C10b | waiting | | |
| C11 | waiting | | |
| C12 | ready | | |
| C13 | ready | | |
| C16 | ready | | |
| C17 | ready | | |
| C18 | ready | | |
| C19 | ready | note from C1c: KeyMap aliases (h j k l) aren't removed by unbind_chord | |
| C1b | merged | 3 review rounds; settings via Ctx::settings (shared handle dropped); no rename timeout (Strata); modifier bullets in-process (Rune draft U-C1b-1); click runs count like GTK; round-3 nits open (run distance from previous press not first; two half-bullet labels; secondary press ends a run: doc) | c4b35d3 |
| C1c | merged | 3 review rounds; default + 10xer maps; layout-aware chord workaround (U-C1c-1); open should-fix for whichever WP builds key rebinding: unbind_chord leaves aliases (h j k l) — add unalias or unbind aliases; aliases follow the key not the command | 3b4c852 |
| C1d | merged | 3 review rounds; open item S-B: a key during edge auto-scroll loses the cursor reveal — C1e seam; open should-fixes: reduced-motion test lacks the (look off, system on) and (on, off) cases; no "left to other WPs" bullet list in marquee/mod.rs (C2/C3/C4 planners: 17 marquee bullets are theirs or hosts'); ~10 Hz apply while auto-scrolling keeps perf meter silent (deviation) | b355ee6 |
| C1e | merged | 3 review rounds; all controller seam requests in; scroll p95 0.59 ms vs Table 0.88; S14 sticky group lines deferred to contract WP C1f before C4; round-3 should-fix SF1 (grid restore before first layout) + nits (KeyboardMode commits unchanged, read_only/roles not in row memo) carried to C1f | 8ea925e |
| C26 | waiting | | |
| P1 | waiting | | |
| P2 | waiting | | |
| P4 | waiting | | |
| X1 | waiting | | |
| C14b | ready | carries C14a round-3 should-fix: key the rescale child by target scale (`{key}-rescale-{scale bits}`) + regression test (2x, one frame, 1x) | |
| C2 | waiting (C24) | | |
| C3 | waiting (C25) | | |
| C4 | coding | no rune-kit Table fork (built on C1e; Table drafts R-K1..7); heading-strip marquee via a small C1e edit in C4; widths scale with text size; headings not Tab stops and fit double-click at Rune 0.5 s (deviations) | |
| X3 | waiting | | |
| C20d | waiting (C3) | | |
| X2 | waiting | from C1f: drop-by-point routing in row.rs has no test that fails without it; the system binder (never sends on_move) must test it | |
