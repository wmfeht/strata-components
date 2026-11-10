//! S3's three owner decisions (`docs/decisions/S3-foreign-parent.md`, "Owner decisions"),
//! each **pending the owner's ruling**. The shim follows S3's recommendation for each, and
//! keeps it here so a different ruling changes only this file.
//!
//! - **Decision 1** (libwayland 1.22): [`without_winit_display`].
//! - **Decision 2** (`x11:` parents for a Wayland window): [`x11_parent_on_wayland`].
//! - **Decision 3** (no activation token): nothing here. A parentless window is placed and
//!   focused by the compositor's policy (option a); calling `WindowHandle::focus` for it
//!   (option b) is the caller's choice.

use super::outcome::{Fallback, Outcome, Reason, WindowSystem};

/// S3 decision 1, pending the owner's ruling: on libwayland below 1.23 winit's display is
/// out of reach, and the window opens parentless (option a). Option b would hold the shim
/// until Rune exposes the display (U9); option c (reading libwayland's private proxy layout)
/// isn't to ship.
pub(super) fn without_winit_display() -> Outcome {
    Outcome::parentless(Reason::NoDisplay, Fallback::Compositor)
}

/// S3 decision 2, pending the owner's ruling: a Wayland window can't be transient for an X11
/// window, so it opens parentless (option a, what Strata does today). Option b would run the
/// window in a helper process on Xwayland; option c would ask Rune for an Xwayland window.
pub(super) fn x11_parent_on_wayland() -> Outcome {
    Outcome::parentless(
        Reason::Mismatch {
            parent: WindowSystem::X11,
            window: WindowSystem::Wayland,
        },
        Fallback::Compositor,
    )
}
