# S3 Foreign parent survey

Spike S3 ([work package](../work/phase-0-groundwork/S3-foreign-parent-survey.md)) asks whether
a Rune window can be made transient for another app's window on Hyprland, GNOME, KDE and sway,
for the portal FileChooser (plan §4 P4, §8). This record is what P4 builds against.

## Verdict

**No blocker found.** All four compositors implement `zxdg_importer_v2`. Rune gives a shim the
window's `wl_surface` or X11 window id before the first frame, which is enough to parent the
window from outside Rune. On sway, a Rune window parented this way floats, stacks above its
parent and shows over a fullscreen parent. On Xwayland, `WM_TRANSIENT_FOR` set from a second X
connection does the same. Real `wayland:` handles from GTK 3, GTK 4 and Chromium imported
cleanly.

Three conditions need an [owner decision](#owner-decisions) before P4 ships. None stops P4 from
starting:

1. Ubuntu 24.04's libwayland 1.22 gives the shim no route to winit's `wl_display`.
2. An `x11:` parent can't be honoured by a chooser running on Wayland.
3. The FileChooser portal carries no activation token, so a parentless chooser is placed and
   focused only by the compositor's own policy. That matters most on GNOME.

Only sway and Xwayland ran here. Every Hyprland, GNOME and KDE cell is a hand test pending with
a source-based expectation. QEMU runs (Tier B) were not approved for this spike;
[How to run Tier B](#how-to-run-tier-b) gives the commands.

## Environment

| Item | Version or setting |
| --- | --- |
| Rune | `78d22ed` (plan §0); winit 0.30.13, wayland-client 0.31.15, wayland-backend 0.3.17, x11rb 0.13.2 |
| Measured compositor | sway 1.12, headless (`WLR_BACKENDS=headless`, gles2 renderer), Xwayland 24.1.14 |
| No-WM X server | Xvfb 21.1.25 |
| Portal | xdg-desktop-portal 1.22.1 on a private bus, with a stub `impl.portal.FileChooser` |
| Requesters | GTK 3.24.52 and GTK 4.24.1 (PyGObject), zenity 4.2.2, Chromium 155.0.8059.84 |
| Read from source | Hyprland 0.56.2, Mutter 46–50, KWin 6.7, GTK 3.24.41–3.24.52, xdg-desktop-portal-gtk, Chromium `xdg_foreign_wrapper.cc` |

Every run used a private `XDG_RUNTIME_DIR`, `dbus-run-session` and a headless sway. Nothing
touched the desktop session. The prototype and raw results live outside this repository in
`~/dev/strata-spikes/s3/`: `proto/` (the `s3-child` Rune app and the `s3-peer` raw client),
`env/` (session and experiment scripts), `requesters/` and `results/`. Result paths below are
relative to `results/`.

## Results

Evidence tiers:

- **Measured** here, on headless sway or Xwayland or Xvfb.
- **Source**: read from the compositor's or toolkit's code at the version named.
- **Pending**: a hand test is pending, with the expectation from source.

"X11" means an X11 requester and an X11 chooser window, which on a Wayland session go through
Xwayland. A Wayland chooser with an `x11:` parent is covered under
[Q4](#q4-fallbacks-when-parenting-fails).

| Compositor | Session | Importer and parent effect | Requester handles | Fallback when parenting fails |
| --- | --- | --- | --- | --- |
| sway 1.12 | Wayland | **Measured.** `zxdg_importer_v1` and `_v2`; no `xdg_wm_dialog_v1`. The parented child floats, centred, and is shown over a fullscreen parent. A bad handle gets `destroyed` within one roundtrip. A v1 handle imports through v2. | **Measured.** GTK 3 and GTK 4: `wayland:` (v2 export). Chromium: `wayland:` (v1 export). zenity: `""`. | **Measured.** Parentless: tiled and focused. Over a fullscreen window: hidden and unfocused. |
| sway 1.12 | X11 (Xwayland) | **Measured.** `WM_TRANSIENT_FOR` with the dialog type: floated, `transient_for` in IPC, shown over a fullscreen parent, provided it's set before sway maps the window. | **Measured.** GTK: `x11:400003`. Chromium: `x11:0x400003`. | **Measured.** A bad xid still floats a dialog-type window, with no parent. Centring is ours to do. |
| Hyprland 0.56.2 | Wayland | **Source, pending.** v2 importer and exporter only (no v1); `xdg_wm_dialog_v1` present; a bad handle gets `destroyed` at import; `set_parent_of` → `setNewParent`. How Hyprland floats or places a child is unverified. | **Source, pending.** GTK 4 and GTK ≥ 3.24.45: v2 handle. Chromium: v2 handle (the only exporter offered). GTK < 3.24.45: `""`. | **Pending.** Parentless. Strata's existing window rule centres the chooser (`src/portal/window_geometry.rs`). |
| Hyprland 0.56.2 | X11 (Xwayland) | **Pending.** Hyprland's XWM reads transients (the binary has `set_transient` and a loop guard). | **Source.** `x11:<hex>`, as on sway. | **Pending.** As on sway. |
| GNOME (Mutter 46–50) | Wayland | **Source, pending.** v2 importer only, in 46, 47, 48 and main. `set_parent_of` → `meta_window_set_transient_for`. A bad handle gets `destroyed`. An invalid child surface is a **fatal protocol error**. Ubuntu 24.04 (Mutter 46): no parenting at all from Rune, because of libwayland 1.22 ([decision 1](#1-ubuntu-2404-and-libwayland-122)). | **Source, pending.** GTK 4 and GTK ≥ 3.24.45: v2 handle. Ubuntu 24.04's GTK 3.24.41 has only the v1 exporter, finds no exporter on Mutter, warns "Server is missing xdg_foreign support" and sends `""`. | **Pending.** Parentless. Mutter places it; focus without an activation token is uncertain ([decision 3](#3-no-activation-token-and-focus-of-a-parentless-chooser)). |
| GNOME | X11 | **Pending.** GNOME 50 ships no Xorg session; Ubuntu 24.04's GNOME 46 still has one. Under Xwayland, Mutter's XWM honours `WM_TRANSIENT_FOR`. | **Source.** `x11:<hex>`. | **Pending.** Centre over the parent's geometry. |
| KDE (KWin 6.7.5) | Wayland | **Source, pending.** v2 only (`xdgforeign_v2.cpp`). A bad handle gets an inert object and `destroyed` straight away. `set_parent_of` links the transient. | **Source, pending.** As GNOME, with Fedora 44's GTK 3.24.52 (v2). Firefox: not installed here. | **Pending.** Parentless. KWin's placement and focus-stealing prevention apply. |
| KDE (kwin_x11 6.7.5) | X11 session | **Pending.** ICCCM transients, standard in KWin. | **Source.** `x11:<hex>`. | **Pending.** Centre over the parent. |

Firefox was not installed, so every Firefox cell is pending. From source it behaves like GTK 3,
since its file picker is GTK 3's `GtkFileChooserNative` on the system GTK.

## Findings by question

### Q1. `zxdg_importer_v2` and `set_parent_of` on a winit toplevel

- **Globals** (`e1/e1.1-globals.json`): sway offers `zxdg_importer_v1` and `_v2` and the
  matching exporters, plus `xdg_activation_v1` and `zwlr_virtual_pointer_manager_v1`. It has no
  `xdg_wm_dialog_v1`. The Hyprland binary has only v2 (`CXDGForeignImporterProtocolV2`) and
  has `xdg_wm_dialog_v1`. Mutter and KWin have v2 only. wayland.app's support table is out of
  date for Hyprland (it says 0.52.1 lacks the protocol).
- **Takes effect** (`e1/e1.2-*`): imported in `App::on_window`, the Rune window maps as sway
  treats a transient. It floats at 424×302, centred, where the parentless control is tiled. It
  is shown over a fullscreen parent, where the parentless control is hidden. sway's IPC shows no
  xdg parent, so floating and fullscreen visibility are the observables.
- **Order matters** (`e1/e1.5-*`): set after the first frame (`App::on_ready`), the import
  succeeds but sway has already tiled the window. Sway decides floating at map. `on_window`
  runs before the first frame on Linux (`window/create.rs:232-233`), so it is the right hook.
- **Cross-version** (`e1/e1.3-*`): a v1-exported handle imports through v2 on sway, because
  wlroots shares one registry between v1 and v2.
- **Invalid or stale handle** (`e1/e1.4-*`): `destroyed` arrives within the roundtrip after
  `import_toplevel`, and the window stays parentless. Hyprland, Mutter and KWin send `destroyed`
  at import too (source).
- **Parent closes** (`e1/e1.6a-*`): `destroyed` reaches the shim's private queue about one
  second in, when the parent exits. It needed only `dispatch_pending` from an
  `AppHandle::update`, because winit's loop reads the shared socket. Sway leaves the child
  floating.
- **Reopen** (`e1/e1.6b-*`): Rune destroys a Wayland window on close. On reopen `on_window`
  runs again, the new surface imports again, and it floats again.
- **Connection safety** (E1.7): with the import dropped before quitting, 11 of 11 runs exit 0
  and none logs a protocol error. With the import left to a thread-local destructor after
  `run()` returns, every run crashed with SIGSEGV in `wl_proxy_destroy`, because winit had
  already closed the display (`e1-exit/`). Mutter (`is_valid_child`) and Hyprland post a fatal
  protocol error if `set_parent_of` gets a surface that isn't an xdg_toplevel. On winit's
  shared connection that would kill the whole app. The surface from `with_raw_handle` is always
  winit's xdg_toplevel, so P4 must never pass any other surface.
- **Raw client** (`e1-raw/`): `s3-peer --import` parents a plain wayland-client toplevel the
  same way. It's the Tier B tool, since it needs no GPU and only glibc 2.34.

### Q2. `WM_TRANSIENT_FOR` on a winit X11 window

- winit 0.30 has no transient-for on X11. `with_parent_window` embeds a child window instead.
  The shim sets `WM_TRANSIENT_FOR`, `_NET_WM_WINDOW_TYPE_DIALOG` and `_NET_WM_STATE_MODAL`
  through its own x11rb connection. Properties can be set from any client.
- **winit maps the window inside `create_window`**, so `on_window` sees it already mapped
  (`mapped_when_set: true`). Sway still floated it, because its XWM decides at the Xwayland
  surface's first commit, after Rune's first frame. That is a race, not a guarantee. Set after
  the first frame, sway records `transient_for` but keeps the window tiled (`e2/e2.1-on-ready`).
- **Hidden-first is reliable.** Open with `WindowOptions { visible: false }`, set the properties
  in `on_window` (`mapped_when_set: false`), then `WindowHandle::show()`. The window floats as a
  dialog with `transient_for` set, and it shows over a fullscreen parent
  (`e2/e2.1-hidden-first*`). On Xvfb with no WM, the same three properties are present before
  map (`e2.2/`).
- A bad xid (`e2/e2.1-invalid-xid`): `GetGeometry` on the parent fails, and the WM floats a
  dialog-type window with no parent. P4 should check the parent first, then skip the
  transient-for and centre the window itself.

### Q3. Do requests carry a usable handle?

Requests went from each requester through xdg-desktop-portal to the stub (`e3/requests.jsonl`,
`e3-chromium/requests.jsonl`, exporters in `e3-exporter/`):

| Requester | Wayland | X11 | Exporter |
| --- | --- | --- | --- |
| GTK 3.24.52 `GtkFileChooserNative` (`GTK_USE_PORTAL=1`) | `wayland:<32 hex>` | `x11:400003` | `zxdg_exporter_v2` |
| GTK 4.24.1 `GtkFileDialog` (`GDK_DEBUG=portals`) | `wayland:<32 hex>` | `x11:600004` | `zxdg_exporter_v2` |
| zenity 4.2.2 `--file-selection` | `""` (no parent window) | not run | none |
| Chromium 155 (`<input type=file>`) | `wayland:<32 hex>` | `x11:0x400003` | `zxdg_exporter_v1` on sway |
| Firefox | pending | pending | expected as GTK 3 |

- Every real `wayland:` handle imported into the Rune child while the requester waited: no
  `destroyed`, and the child floated over the requester (`e3/children/req1–2`,
  `e3-chromium/children/req1`). GTK unexports the handle after the response, so the import must
  happen while the request is open.
- **Hex format differs.** GTK sends bare hex and Chromium prefixes `0x`, so P4's parser must
  accept both.
- **Chromium binds whichever exporter the registry announces first** (`xdg_foreign_wrapper.cc`
  `Instantiate`). That is v1 on sway, and v2 on Hyprland, Mutter and KWin, which offer nothing
  else. On a compositor with both, P4 imports a v1 handle through v2. That works on wlroots,
  which shares the registry. A compositor with separate v1 and v2 tables would need a v1
  importer fallback; none of the four targets has v1 at all except sway.
- **GTK 3 below 3.24.45** has only the v1 exporter (3.24.43 lacks v2; 3.24.45 has it). On a
  v2-only compositor it sends `""`. That covers Ubuntu 24.04's 3.24.41 on Mutter 46.
- The portal passes `parent_window` through unchanged. Neither `OpenFile`'s options nor the 1.22
  interface XML carry an activation token. Tokens appear only in OpenURI, Email, AppChooser,
  GlobalShortcuts and DynamicLauncher.

### Q4. Fallbacks when parenting fails

The work package offered "centring, or an `xdg_activation` token only". **The token option
doesn't exist.** The FileChooser request carries no token, and the chooser can't mint one: a
Wayland compositor issues tokens only against a recent input serial on a focused surface of the
client. Centring is a no-op on Wayland (winit can't position windows there). So the fallback is
**parentless, plus per-compositor placement**:

| Case | Fallback | Evidence |
| --- | --- | --- |
| Empty handle (`""`) | Parentless | Measured (zenity) |
| `wayland:` handle → `destroyed` | Parentless. Drop the import. | Measured (E1.4) |
| `x11:` handle, chooser on Wayland | Parentless (as Strata does today, `src/ui/chooser.rs:2315`), or an X11 helper process ([decision 2](#2-x11-parents-on-a-wayland-session)) | Measured: both paths work on sway (`e3/children/req4–6`, `e3-chromium/children/req2–3`) |
| `x11:` handle with a bad xid | No transient-for. Centre on the screen. | Measured (E2.1) |
| Parent closes while the chooser is open | Keep the chooser open and parentless. | Measured (E1.6a) |
| Ubuntu 24.04 (libwayland 1.22) | Parentless ([decision 1](#1-ubuntu-2404-and-libwayland-122)) | Source |

How a parentless chooser is placed and focused, per compositor (E4.1, `e4/`):

| Compositor | Placement | Focus |
| --- | --- | --- |
| sway | Tiled beside the focused window | Focused, unless a fullscreen window is on the workspace (measured) |
| Hyprland | Strata's window rule centres it on the monitor | New windows take focus by default (pending) |
| GNOME | Mutter's placement | Uncertain without a token (pending; [decision 3](#3-no-activation-token-and-focus-of-a-parentless-chooser)) |
| KDE | KWin's placement policy | Focus-stealing prevention, Low by default (pending) |

Sway never focuses a view that maps while its workspace has a fullscreen view, **even a
transient one** (`should_focus` in `sway/tree/view.c`). Measured: over a fullscreen parent, the
parented chooser is visible but the parent keeps the seat's focus. Headless sway has no keyboard
device, so client-side focus (`on_window_focus`) never fired. Sway's seat focus in the IPC tree
is the measure.

## API recipe for P4

All of this ran in `proto/src/bin/s3-child.rs`.

1. **Hook.** Register `App::on_window` (or `WindowSpec::on_created` for `App::window`). It runs
   on the UI thread after every native creation, before the first frame. That includes every
   reopen, since Rune destroys Wayland windows on close. Never parent from `on_ready`.
2. **Parse** `wayland:<handle>`, `x11:<hex>` and `x11:0x<hex>`. An empty or unknown string means
   no parent. A parent kind that doesn't match the window's backend counts as a failure to
   report, not an error.
3. **Wayland.**
   - `WindowHandle::with_raw_handle` gives `RawWindowHandle::Wayland` with the `wl_surface`
     pointer, and nothing else. Rune exposes no display handle.
   - Get the display with `wl_proxy_get_display(surface)`, looked up with `dlopen`
     (`RTLD_NOLOAD`) and `dlsym` at run time. Don't enable wayland-sys's
     `libwayland_client_1_23` feature: features unify, and on libwayland 1.22 winit's own load
     of the library would then fail.
   - `Backend::from_foreign_display`, then a private `EventQueue` via `registry_queue_init`,
     as in Rune's `platform/activation.rs`.
   - Bind `zxdg_importer_v2`, `import_toplevel`, then `set_parent_of` on the surface wrapped with
     `ObjectId::from_ptr`. Destroy the importer. Do one `roundtrip` and read `destroyed`. It
     costs one round trip on the UI thread.
   - Keep the `zxdg_imported_v2` for the window's life. Poll with `dispatch_pending` from a Rune
     update or timer to catch a later `destroyed`.
   - **Drop the import on the UI thread before Rune's event loop ends** (in `on_close`, or before
     `quit`). Dropped later, it segfaults in `wl_proxy_destroy`.
4. **X11.**
   - Open the chooser with `WindowOptions { visible: false }`.
   - In `on_window`, take the xid from `RawWindowHandle::Xlib` or `Xcb`. Open an x11rb
     connection to `$DISPLAY` and check the parent with `GetGeometry`.
   - Set `WM_TRANSIENT_FOR`, `_NET_WM_WINDOW_TYPE` = `_NET_WM_WINDOW_TYPE_DIALOG` and
     `_NET_WM_STATE` = `_NET_WM_STATE_MODAL`, `sync`, then `WindowHandle::show()`.
   - If the window is already mapped, change `_NET_WM_STATE` with the EWMH client message
     instead.
5. **Report** the outcome to the caller (parented, parentless and why), so Strata can log it and
   apply Hyprland placement.
6. **Optional:** `xdg_wm_dialog_v1` `set_modal` on Hyprland, Mutter and KWin. sway lacks it.
   This wasn't prototyped.

## Owner decisions

These are open. Each lists options and a recommendation. S3 decides none of them.

### 1. Ubuntu 24.04 and libwayland 1.22

The shim's only route to winit's `wl_display` is `wl_proxy_get_display`, which exists from
libwayland 1.23 (2024). Ubuntu 24.04 LTS ships 1.22.0; Fedora 44 and Arch ship 1.26, and NixOS
26.05 ships 1.25. On 24.04 the chooser opens parentless on GNOME. GTK 3 apps there send `""`
anyway, because GTK 3.24.41 can't export to Mutter.

- **a.** Accept parentless on libwayland < 1.23, report it, and lift it when Rune exposes the
  display handle.
- **b.** Hold P4 until Rune adds a display handle or a parent API (the
  [Rune issue](#draft-rune-issue-for-s5)).
- **c.** Read the display pointer out of libwayland's private `struct wl_proxy` layout.

**Recommendation: a.** It costs one LTS, which Ubuntu 26.04 replaces, and GTK 3 apps on 24.04
are parentless regardless. c depends on a private ABI and should not ship.

### 2. `x11:` parents on a Wayland session

Electron, Java, Wine and other Xwayland apps send `x11:`. A Wayland chooser can't be transient
for an X11 window: no protocol exists for it. xdg-desktop-portal-gtk handles this by opening its
dialog on a second, X11 display. winit allows one event loop and one backend per process, so a
Rune chooser can't.

- **a.** Parentless. This is what Strata does today (`src/ui/chooser.rs:2315` ignores `x11:`).
- **b.** For an `x11:` parent, run that request's chooser in a helper process with
  `WAYLAND_DISPLAY` unset, so it opens on Xwayland. Measured on sway with GTK 3 and Chromium:
  it floats as a transient (`e3/children/req6`, `e3-chromium/children/req3`). The costs are a
  second process, X11 input and IME quality, and an Xwayland start on first use.
- **c.** Ask Rune to open one window on Xwayland from a Wayland app. Rune already does this for
  its GNOME overlay (`platform/x11/xwayland.rs`, override-redirect, no keyboard), but a
  focusable dialog is far more work.

**Recommendation: a for the first Rune release** (parity with today), with b recorded as the
upgrade path if users miss it.

### 3. No activation token, and focus of a parentless chooser

FileChooser carries no token, so "activation token only" isn't available, and a parentless
chooser is placed and focused by the compositor's policy alone. On sway and Hyprland a new
window takes focus. GNOME may deny focus to a tokenless Wayland window and show "is ready"
instead. That's the case for `""` and `x11:` parents and for Ubuntu 24.04, and it's pending a
hand test.

- **a.** Accept compositor policy, and verify GNOME by hand before M6.
- **b.** Also call Rune's tokenless `focus()`, which requests attention: GNOME marks the window
  urgent.
- **c.** Ask xdg-desktop-portal upstream for an activation token in FileChooser requests. That
  is outward-facing and the owner's call.

**Recommendation: a plus b.** Revisit c only if the GNOME hand test shows the chooser opening
behind the app.

## Constraints for later WPs

- **P4**
  - Follow the [API recipe](#api-recipe-for-p4).
  - Accept `x11:0x…` as well as bare hex.
  - Treat `""` and backend-mismatched parents as parentless outcomes.
  - Never call `set_parent_of` on anything but the window's own toplevel surface.
  - Drop imports before the loop ends.
  - Open hidden on X11.
  - Its sway test can't read an xdg parent from IPC. Assert floating and fullscreen visibility
    instead, plus `transient_for` for Xwayland.
  - P4's "Depends on P0 (display access)" holds: this spike reached the display without P0, but
    P0 should own that code.
- **W3**
  - Arch's `/usr/bin/sway` has `cap_sys_nice=ep`. Under a hard `RLIMIT_RTTIME` of 0 it goes
    realtime and the kernel SIGKILLs it, so start it with `setpriv --no-new-privs`.
  - Use `WLR_RENDERER=gles2` so wgpu gets dmabuf. pixman also runs, for clients that need no
    GPU.
  - Headless sway has no input devices. Clients never bind `wl_pointer` or `wl_keyboard`, so
    clicks need `zwlr_virtual_pointer_v1` (`s3-peer --click X,Y`), and focus events need a
    virtual keyboard.
  - Create the private `XDG_RUNTIME_DIR` **before** `dbus-run-session`. Otherwise an activated
    service (gvfsd here) inherits the desktop's `/run/user/<uid>`. Set `GIO_USE_VFS=local`.
    Unmount the document portal's `$XDG_RUNTIME_DIR/doc` (`fusermount3 -u -z`) before removing
    the directory.
  - `XDG_DESKTOP_PORTAL_DIR` also makes xdg-desktop-portal read `portals.conf` from that
    directory.
  - Hyprland 0.56 can't nest in sway 1.12: aquamarine needs `xdg_wm_base` v6 and sway offers v5.
    Hyprland has no headless-only mode.
- **S2 / P0**
  - Rune's public API yields the `wl_surface` but no display.
  - `wl_proxy_get_display` with `dlsym` works on libwayland ≥ 1.23. Enabling wayland-sys's 1.23
    feature would break 1.22 hosts.
  - A private queue on the foreign display needs only `dispatch_pending` from a Rune update to
    see its events.
  - Anything bound on winit's connection must be destroyed before the event loop drops.
  - One Rune issue should cover the display handle for S2, P0 and P4.
- **S5**: file the [draft below](#draft-rune-issue-for-s5).

## Draft Rune issue for S5

**Title:** Linux: no way to make a window transient for another app's window (xdg-foreign v2,
`WM_TRANSIENT_FOR`)

**Why.** An XDG portal backend (a FileChooser) gets its caller's window as `wayland:<xdg-foreign
handle>` or `x11:<xid>`, and its dialog must be transient for that window. Rune has no API for
it. Apps can only half do it from outside: `WindowHandle::with_raw_handle` yields the window
handle but no display handle, and on X11 the hooks run after winit has mapped the window.

**Steps** (headless, private session):

```sh
export XDG_RUNTIME_DIR=$(mktemp -d); chmod 700 "$XDG_RUNTIME_DIR"
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 setpriv --no-new-privs sway -c /dev/null &
sleep 1; export WAYLAND_DISPLAY=wayland-1
cargo run   # the main.rs below; then again with WAYLAND_DISPLAY unset and DISPLAY set to sway's Xwayland
```

```rust
// Cargo.toml: rune (rev 78d22ed, default-features = false, features = ["embedded-fonts"]), x11rb 0.13
use rune::raw_window_handle::RawWindowHandle;
use rune::{AppSpec, Cx, El, el};

fn view(_: &(), _: &mut Cx<()>) -> El { el() }

fn main() {
    let spec = AppSpec::new("transient", (400.0, 300.0), || (), view);
    rune::App::new(spec).args(Vec::new()).on_window(|w| {
        w.with_raw_handle(|h| match h.as_raw() {
            // Only the wl_surface: no wl_display to bind zxdg_importer_v2 on. A shim has to
            // dlsym libwayland 1.23's wl_proxy_get_display to find winit's connection.
            RawWindowHandle::Wayland(wl) => println!("wayland surface {:p}, display: none", wl.surface),
            RawWindowHandle::Xlib(x) => {
                use x11rb::protocol::xproto::ConnectionExt;
                let (c, _) = x11rb::connect(None).unwrap();
                let a = c.get_window_attributes(x.window as u32).unwrap().reply().unwrap();
                // Already mapped: WM_TRANSIENT_FOR set now arrives after the WM has placed it.
                println!("x11 window {:x}, map_state {:?}", x.window, a.map_state);
            }
            _ => {}
        }).unwrap();
    }).run();
}
```

**Expected.** A way to open a window transient for a foreign parent, for example
`WindowOptions::transient_for(ForeignParent::parse("wayland:…"))`, applied before the window
maps, with an outcome saying whether the compositor accepted it. At minimum,
`WindowHandle::with_raw_display_handle`, plus a hook that runs before an X11 window is mapped.

**Actual.**
- Wayland: `wayland surface 0x…, display: none`.
- X11: `map_state VIEWABLE`. Measured at `78d22ed`: winit maps the window in `create_window`,
  before `on_window` runs.
- The out-of-Rune workaround (`dlsym` `wl_proxy_get_display`, `Backend::from_foreign_display`,
  `zxdg_importer_v2` on a private queue) works on sway, but fails on libwayland 1.22 (Ubuntu
  24.04 LTS). It also segfaults if the import outlives the event loop.

## How to run Tier B

The local strata-qemu-testing goldens can turn the pending cells into measurements. They boot
headless from throwaway overlays, need about 8 GiB of RAM each, and download nothing. Run one
guest at a time.

| Guest | Covers | Notes |
| --- | --- | --- |
| `arch`, `omarchy-4` | Hyprland 0.56.2, Wayland and Xwayland; Chromium on omarchy-4 | `s3-child` runs here too (glibc 2.44) |
| `fedora-workstation` | KWin 6.7.5 Wayland, Firefox 155, GTK 3.24.52 | `s3-peer` only |
| `fedora-workstation-x11` | kwin_x11 6.7.5 | `s3-peer --x11` only |
| `ubuntu-2404` | GNOME 46, libwayland 1.22, GTK 3.24.41, snap Firefox | Decisions 1 and 3; `s3-peer` only |
| `nixos` | GNOME 50.4 | Prebuilt binaries need nix-ld or `patchelf` for the loader |

```sh
cd ~/dev/strata-qemu-testing
mise run vm-run -- fedora-workstation          # headless; prints the SSH port
K=~/.cache/strata-qemu-testing/keys/id_ed25519 P=<port> S=~/dev/strata-spikes/s3
ssh -i $K -p $P tester@127.0.0.1 mkdir -p s3
scp -i $K -P $P -r $S/proto/target/debug/s3-peer $S/requesters $S/qemu/guest-probe.sh tester@127.0.0.1:s3/
ssh -i $K -p $P tester@127.0.0.1 'bash ~/s3/guest-probe.sh kde' | tee $S/results/tierb-fedora-workstation.txt
```

`guest-probe.sh` takes `hyprland`, `gnome` or `kde`. It dumps the foreign globals, parents a raw
child to a peer, tries an invalid handle and a parent that exits, and captures each requester's
`parent_window` with `dbus-monitor`. It prints `hyprctl clients -j` on Hyprland and a KWin
script's `transient`, `transientFor` and `active` on KDE. GNOME has no transient-for IPC, so
judge from the run's QMP screendump.

For Firefox, open it in the guest and send Ctrl+O through the run's QMP socket
(`~/.cache/strata-qemu-testing/runs/<run>/qmp.sock`):

```sh
python3 -c 'import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.recv(4096)
for m in [b"{\"execute\":\"qmp_capabilities\"}", b"{\"execute\":\"send-key\",\"arguments\":{\"keys\":[{\"type\":\"qcode\",\"data\":\"ctrl\"},{\"type\":\"qcode\",\"data\":\"o\"}]}}"]:
    s.send(m); print(s.recv(4096))' ~/.cache/strata-qemu-testing/runs/<run>/qmp.sock
```

`guest-probe.sh` has not been run, since Tier B wasn't approved. Expect to adjust it on first
use.

## Not done

- No Hyprland, GNOME or KDE run, and no Firefox: Tier B not approved, and Firefox isn't
  installed here.
- Client-side keyboard focus wasn't observable on headless sway, which has no keyboard device.
- `xdg_wm_dialog_v1` `set_modal` wasn't prototyped: sway lacks it.
- GTK 4 with its X11 backend wasn't run through the X11 helper path. GTK 3 and Chromium were.
