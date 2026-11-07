//! The modal shell: backdrop, motion, dismissal, sizing and key routing around any panel.

use std::rc::Rc;

use rune::{El, Key, KeyEvent, Mods, Spring, SpringPair, Transition, Tx, col, el};
use strata_kit::look::Look;

use super::keys::{self, Route};
use super::{BACKDROP_BLUR, BACKDROP_KEY, ENTRANCE_SCALE, MOTION_SECONDS, PANEL_RADIUS, WIDTH};

/// A modal panel over a blurred backdrop: what every dialog shares (see the module docs).
///
/// ```ignore
/// DialogShell::new("palette", "Search", |s: &mut App| s.palette = false)
///     .open(s.palette)
///     .width(640.0)
///     .child(palette_contents)
///     .build(&look, &mut layer);
/// ```
///
/// Keys: `<key>-layer` (the full-window layer the panel centres in), `<key>-dialog` (the
/// panel: role `dialog`, named by `label`), and the shared [`BACKDROP_KEY`].
pub struct DialogShell<S> {
    key: String,
    label: String,
    description: Option<String>,
    children: Vec<El>,
    open: bool,
    blocking: bool,
    stacked: bool,
    width: f32,
    fade_only: bool,
    return_focus: Option<String>,
    footer: Vec<String>,
    dismiss: Rc<dyn Fn(&mut S)>,
}

impl<S: 'static> DialogShell<S> {
    /// A closed shell keyed `key`, named `label` for assistive technology. `on_dismiss` runs
    /// on Escape and on a click outside the panel; it closes the dialog without confirming.
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        on_dismiss: impl Fn(&mut S) + 'static,
    ) -> Self {
        DialogShell {
            key: key.into(),
            label: label.into(),
            description: None,
            children: Vec::new(),
            open: false,
            blocking: false,
            stacked: false,
            width: WIDTH,
            fade_only: false,
            return_focus: None,
            footer: Vec::new(),
            dismiss: Rc::new(on_dismiss),
        }
    }

    /// Shown while true. Closing plays the exit motion; the dialog is inert as it leaves.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// What assistive technology reads after the name: the subtitle.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds `child` to the panel, top to bottom.
    pub fn child(mut self, child: El) -> Self {
        self.children.push(child);
        self
    }

    /// Ignores clicks outside the panel while true: a dialog in a state that can't be
    /// cancelled (an operation in progress), or a form with something typed in it. Escape
    /// still calls `on_dismiss`.
    pub fn blocking(mut self, blocking: bool) -> Self {
        self.blocking = blocking;
        self
    }

    /// Opened over another dialog: draws no backdrop of its own, so the one beneath keeps the
    /// blur until the last dialog closes.
    pub fn stacked(mut self, stacked: bool) -> Self {
        self.stacked = stacked;
        self
    }

    /// The panel's width in points, when the window has room: a narrower window fills its
    /// width less a margin.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Opens and closes with a fade alone, no scale: Settings' layer.
    pub fn fade_only(mut self) -> Self {
        self.fade_only = true;
        self
    }

    /// On close, focus goes to the element keyed `key` (or its first focusable child) instead
    /// of the element focused before the dialog opened.
    pub fn return_focus(mut self, key: impl Into<String>) -> Self {
        self.return_focus = Some(key.into());
        self
    }

    /// The keys of the footer buttons that can take focus, in document order: Left and Right
    /// move between them.
    pub fn footer_keys(mut self, keys: Vec<String>) -> Self {
        self.footer = keys;
        self
    }

    /// Pushes the backdrop and the panel onto `layer`, the overlays `rune_kit::root` mounts,
    /// when open.
    pub fn build(self, look: &Look, layer: &mut Vec<El>) {
        if !self.open {
            return;
        }
        let kit = &look.kit;
        let (panel_motion, backdrop_motion) = motion(look.animations, self.fade_only);
        if !self.stacked {
            layer.push(
                el().key(BACKDROP_KEY)
                    .absolute(0.0, 0.0)
                    .w_full()
                    .h_full()
                    .fill(kit.c.scrim)
                    .backdrop_blur(BACKDROP_BLUR)
                    .transition(backdrop_motion)
                    .a11y_hidden(),
            );
        }

        let panel_key = format!("{}-dialog", self.key);
        let mut panel = col()
            .key(panel_key.clone())
            .role("dialog")
            .label(self.label)
            .w_full()
            .max_w(self.width)
            // Shrinks to the window's height; the body inside scrolls.
            .shrink()
            .min_h(0.0)
            .radius(look.round(PANEL_RADIUS))
            .fill(kit.c.layer_01)
            .border(1.0, kit.c.border_base)
            .children(self.children);
        if let Some(d) = self.description {
            panel = panel.description(d);
        }
        panel = if look.glow.on {
            panel.shadow(
                0.0,
                look.glow.radius,
                look.glow.color.alpha(look.glow.opacity),
            )
        } else {
            kit.shadow_layers(&kit.elevation.overlay)
                .fold(panel, |p, s| p.box_shadow(s))
        };
        let footer = self.footer;
        panel = panel.on_key(move |_s: &mut S, ev: &KeyEvent, tx: &mut Tx| {
            let route = keys::route(ev, tx.key_repeat(), tx.focused(), &panel_key, &footer);
            match route {
                Route::Pass => false,
                Route::Consume => true,
                Route::Press => send(tx, Key::Text(" ".into()), false),
                Route::Forward => send(tx, Key::Tab, false),
                Route::Back => send(tx, Key::Tab, true),
            }
        });
        if let Some(key) = self.return_focus {
            panel = panel.focus_return(key);
        }
        let dismiss = self.dismiss;
        panel = panel.modal(!self.blocking, move |s: &mut S| dismiss(s));

        layer.push(
            col()
                .key(format!("{}-layer", self.key))
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                // A narrow window: the panel fills its width less this margin.
                .padding(look.space.lg)
                .center()
                .transition(panel_motion)
                .child(panel),
        );
    }
}

/// Sends `key` through the window's key routing, after this handler. Rune applies it on the
/// next frame.
fn send(tx: &mut Tx, key: Key, shift: bool) -> bool {
    tx.send_key(KeyEvent {
        key,
        mods: Mods {
            shift,
            ..Mods::default()
        },
    });
    true
}

/// The panel's and the backdrop's presence motion: a 200 ms scale-and-fade (a fade for
/// Settings), or none when the user turned animations off. Rune follows the host's Reduce
/// Motion with a short fade; animations off means none at all, as in Strata.
fn motion(animations: bool, fade_only: bool) -> (Transition, Transition) {
    if !animations {
        return (Transition::none(), Transition::none());
    }
    let spring = Spring::new(MOTION_SECONDS, 0.0);
    let springs = SpringPair::new(spring, spring);
    let panel = if fade_only {
        Transition::opacity()
    } else {
        Transition::scale(ENTRANCE_SCALE)
    };
    (
        panel.springs(springs),
        Transition::opacity().springs(springs),
    )
}
