//! Zoom, pan and fit over `Pixels` (C14a): the surface under C14b's image, model, route and
//! G-code views and C13's video, with the zoom model C12's pages reuse.
//!
//! Work package: C14a, `docs/work/phase-1-leaves/C14a-pixels-viewport.md`. Plan §4 C14.
//!
//! ```ignore
//! Viewport::new(&s.image, |s: &mut App| &mut s.image)
//!     .key("image")
//!     .pixels(s.photo.pixels())
//!     .label("photo.png")
//!     .transparent(true)
//!     .on_zoom(|s: &mut App, r: ZoomReport, _tx| s.render_at(r.wanted_px(8192)))
//!     .build(&s.look, cx)
//! ```
//!
//! The host keeps a [`ViewportState`] and tells it the content's size in content pixels
//! ([`ViewportState::set_content`]); the pixels arrive as a `rune::Pixels` the host owns (an
//! `ImageCache` image, P7's video frames, a page from Strata's renderer). The viewport owns no
//! I/O and never reads the `Pixels`' own size.
//!
//! # What it does
//!
//! - **Modes** ([`Mode`]): Fit, ScaleDown (fit, never enlarged), Fill, Actual (1:1) and Free.
//!   Fit, ScaleDown and Fill follow the frame as it resizes. A level is device pixels per
//!   content pixel, so 1:1 is pixel for pixel at any display scale.
//! - **Zoom** with Ctrl+wheel and pinch, about the pointer; `+` and `-` step through
//!   [`ViewportState::steps`] about the centre, and `0` or Ctrl+0 resets to the default mode.
//!   Each change of level sends a [`ZoomReport`] to [`Viewport::on_zoom`].
//! - **Pan** by drag, by the wheel, and by the arrow keys, only along an axis the content
//!   overflows, held at the content's edges. An axis that fits stays centred.
//! - **Checkerboard** behind transparent content ([`Viewport::transparent`]), from Kit tokens.
//! - **Overlay**: children placed in content pixels ([`Viewport::overlay`], [`ContentMap`]),
//!   which pan with the content and keep their own size as it zooms (C14b's route marker; C13's
//!   stand-in before the first frame).
//! - **Accessibility**: the frame is an image named by [`Viewport::label`], with the zoom level
//!   as its value ("56%").
//!
//! [`Interaction::Fixed`] turns the input off for a surface that only shows (the video frame,
//! the model render): no focus, no keys, no wheel or pinch handlers, no drag. With no handlers
//! attached, Ctrl+wheel and a pinch over it reach the app (Strata's text size). A Fixed frame
//! isn't a control, so a press or a drag source on an element around it works (C13's click to
//! play), as does [`Viewport::drag_source`].
//!
//! # Updating without a view commit
//!
//! - **Pixels.** The content element always has an explicit size, computed from the content's
//!   size the host gave, so `Pixels::set` (a video frame, a sharper render) draws on the next
//!   frame without re-running the view or the layout, even when the image changes shape. A
//!   `Pixels` with only one side set would re-run the view on a change of shape.
//! - **Pan.** The pan is a `rune::Live` offset on the content element. Drag, wheel and arrow pans
//!   set it and call `Tx::live_only`, so they skip the view, the diff and the layout. The view
//!   never reads the pan (the checkerboard covers only what's visible, which doesn't depend on
//!   it), which keeps Rune's live-only check quiet.
//! - **Zoom** commits: the accessible value and the content's laid-out size change. The content
//!   lands at once (`El::instant_changes`), or a layout spring would lag behind the instant pan
//!   and the point under the pointer would wander.
//!
//! # Deliberate change, for Strata to decide
//!
//! Strata's image preview is a fitted picture with no zoom. Over it, Ctrl+wheel changes the
//! interface text size (`app/window/key-dispatch`), and a drag drags the file out. Plan §4 C14
//! asks for an image view with zoom and pan, so over a [`Interaction::Full`] viewport:
//!
//! - Ctrl+wheel (and a pinch) zooms the image instead of changing the text size;
//! - a drag above fit pans instead of dragging the file out. With the default
//!   [`PanDrag::WhenOverflowing`], the host's [`Viewport::drag_source`] is attached only while
//!   nothing overflows (at fit or below), and the pan's `on_drag` only while something does, so
//!   one drag never does both (checked in `tests.rs`). The source has to be the viewport's: a
//!   drag goes to the element pressed, and a drag source on an element around the viewport's
//!   focusable frame never hears it.
//!
//! [`Interaction::Fixed`] and [`PanDrag::Never`] keep Strata's behaviour exactly.
//!
//! # What uses it
//!
//! - **C14b**: the image view (Full); the model view, route and G-code views (Fixed). The model
//!   view orbits by its two camera buttons only: the feature map says "Only the two buttons
//!   change the view" and "The model picture keeps the grab cursor and file drag"
//!   (`preview/preview-panel/model-views`), so this module has no orbit drag. C14b's work
//!   package ("Orbit by drag sends a camera intent") conflicts with that; it's C14b's to
//!   resolve.
//! - **C13**: the video surface (Fixed, opaque), with the host updating its `Pixels` in place.
//! - **C12**: its page column scrolls in its own scroll view (Rune virtual rows mount from the
//!   scroll offset, so a live pan can't move them). It reuses [`zoom`] (the factors,
//!   [`ZoomLimits`], [`ZoomReport`], [`ctrl_zero_key`]) and [`checkerboard`].
//!
//! # Tests
//!
//! The WP's "Scripts:" line (wheel zoom about the pointer, pinch, fit after a resize, pan limits,
//! the keyboard) is met by in-process tests through Runtime input that assert state
//! (`crates/strata-gallery/tests/c14a.rs`), because no feature-map bullet describes image zoom or
//! pan: it is new behaviour. Only the G-code bullet ("scales down to fit the pane and is never
//! enlarged") has a script, `tests/scripts/c14a/scale-down-resize.json`.
//!
//! # Rune
//!
//! - Pixels sample bilinear only, so a magnified image is smoothed, not pixelated.
//! - `Pixels::set` builds no mips, so a large streamed image shrunk to fit aliases; hosts use
//!   `ImageCache` for stills, which does.
//! - An image over 8192 pixels on a side doesn't draw: the host downsamples it, and keeps the
//!   content size at the original's so 1:1 still means the original's pixels.
//! - In a Linux or Windows window, Rune turns any Ctrl+wheel over an `on_magnify` handler into a
//!   pinch with no modifiers, so Ctrl+Shift+wheel can't be told from a pinch, and a declined
//!   pinch doesn't fall back to the wheel. Headless runs send Ctrl+wheel to `on_wheel` instead.
//!   The viewport zooms on both paths; Fixed attaches neither.
//! - There is no repeating pattern fill; the checkerboard is a shape of squares.
//!
//! # Open items
//!
//! - Strata's keymaps haven't been checked for plain `+`, `-` and `0` while a preview has focus.
//!   [`Viewport::zoom_keys`] takes another key set ([`zoom_key`] is the default).

mod checker;
mod state;
pub mod zoom;

#[cfg(test)]
mod tests;

use std::rc::Rc;

use rune::{Cursor, Cx, Drag, El, KeyEvent, MagnifyEvent, Pixels, SurfaceFit, Tx, WheelEvent, el};
use strata_kit::look::Look;

use crate::contract::{Dnd, DragSource};

pub use checker::checkerboard;
pub use state::ViewportState;
pub use zoom::{
    Mode, ZoomKey, ZoomLimits, ZoomReport, ctrl_zero_key, default_steps, no_keys, zoom_key,
};

/// How much input the viewport takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Interaction {
    /// Zoom, pan and the keys; focusable.
    #[default]
    Full,
    /// Shows the content in its mode and takes no input at all.
    Fixed,
}

/// When a primary drag pans (with [`Interaction::Full`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PanDrag {
    /// Only while the content overflows the frame. Otherwise a drag drags the file
    /// ([`Viewport::drag_source`]), or does nothing.
    #[default]
    WhenOverflowing,
    /// Always: a drag never drags the file.
    Always,
    /// Never: drags are the host's; the wheel and the arrows still pan.
    Never,
}

/// The text the viewport shows assistive technology.
#[derive(Clone, Copy, Debug)]
pub struct Labels {
    /// The accessible value for a level (device pixels per content pixel): "56%".
    pub level: fn(f32) -> String,
}

impl Default for Labels {
    fn default() -> Labels {
        Labels {
            level: |level| format!("{:.0}%", level * 100.0),
        }
    }
}

/// Where content pixels land inside the content element, for [`Viewport::overlay`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentMap {
    /// Layout points per content pixel at the current level.
    pub points_per_px: f32,
    /// The content's drawn size in points.
    pub size: [f32; 2],
}

impl ContentMap {
    /// The point for content pixel `px`, from the content's top-left corner.
    pub fn point(&self, px: [f32; 2]) -> [f32; 2] {
        [px[0] * self.points_per_px, px[1] * self.points_per_px]
    }

    /// The rect `[x, y, w, h]` for a rect in content pixels.
    pub fn rect(&self, px: [f32; 4]) -> [f32; 4] {
        let k = self.points_per_px;
        [px[0] * k, px[1] * k, px[2] * k, px[3] * k]
    }
}

type ZoomFn<S> = Rc<dyn Fn(&mut S, ZoomReport, &mut Tx)>;
type OverlayFn<'a> = Box<dyn FnOnce(&ContentMap) -> El + 'a>;

/// A zoomable, pannable surface over a `Pixels`: `Viewport::new(&s.view, access) …
/// .build(look, cx)`. It fills its parent; give the parent a bounded size.
pub struct Viewport<'a, S> {
    state: &'a ViewportState,
    access: fn(&mut S) -> &mut ViewportState,
    key: String,
    pixels: Option<&'a Pixels>,
    label: Option<String>,
    transparent: bool,
    interaction: Interaction,
    pan_drag: PanDrag,
    overlay: Option<OverlayFn<'a>>,
    on_zoom: Option<ZoomFn<S>>,
    labels: Labels,
    keys: fn(&KeyEvent) -> Option<ZoomKey>,
    drag: Option<(Dnd<S>, DragSource<S>)>,
}

impl<'a, S: 'static> Viewport<'a, S> {
    /// The viewport for `state`, which `access` finds in the app's state.
    pub fn new(state: &'a ViewportState, access: fn(&mut S) -> &mut ViewportState) -> Self {
        Viewport {
            state,
            access,
            key: "viewport".into(),
            pixels: None,
            label: None,
            transparent: false,
            interaction: Interaction::Full,
            pan_drag: PanDrag::WhenOverflowing,
            overlay: None,
            on_zoom: None,
            labels: Labels::default(),
            keys: zoom_key,
            drag: None,
        }
    }

    /// The frame's key ("viewport" unless set). The content is `<key>-content`, its
    /// checkerboard `<key>-checker` and the overlay `<key>-overlay`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// The image to show, drawn into the content's size. Without one the content is empty (an
    /// overlay can stand in for it).
    pub fn pixels(mut self, pixels: &'a Pixels) -> Self {
        self.pixels = Some(pixels);
        self
    }

    /// The image's accessible name: a file name, "Route preview".
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Whether the content may be transparent: a checkerboard shows behind it. Off unless set,
    /// so an opaque surface (a video, a model render on the theme's surface) shows no board.
    pub fn transparent(mut self, on: bool) -> Self {
        self.transparent = on;
        self
    }

    pub fn interaction(mut self, interaction: Interaction) -> Self {
        self.interaction = interaction;
        self
    }

    pub fn pan_drag(mut self, pan_drag: PanDrag) -> Self {
        self.pan_drag = pan_drag;
        self
    }

    /// Children over the content, built with where content pixels land. The element `f` returns
    /// sits in a box the content's size (keyed `<key>-overlay`), pans with the content, and is
    /// rebuilt at each level. Its children appear under the image to assistive technology.
    pub fn overlay(mut self, f: impl FnOnce(&ContentMap) -> El + 'a) -> Self {
        self.overlay = Some(Box::new(f));
        self
    }

    /// Called when the level changes, with where the content now draws.
    pub fn on_zoom(mut self, f: impl Fn(&mut S, ZoomReport, &mut Tx) + 'static) -> Self {
        self.on_zoom = Some(Rc::new(f));
        self
    }

    pub fn labels(mut self, labels: Labels) -> Self {
        self.labels = labels;
        self
    }

    /// Which keys zoom, reset and pan while the viewport has focus ([`zoom_key`] unless set).
    pub fn zoom_keys(mut self, keys: fn(&KeyEvent) -> Option<ZoomKey>) -> Self {
        self.keys = keys;
        self
    }

    /// Dragging the content drags the file, through `dnd`, whenever a drag doesn't pan: always
    /// with [`PanDrag::Never`] or [`Interaction::Fixed`], and at fit or below with
    /// [`PanDrag::WhenOverflowing`].
    ///
    /// It has to be the viewport's: a drag goes to the element pressed, and the viewport's frame
    /// is a control (focusable, with key and wheel handlers), so a drag source on an element
    /// around it never hears the drag.
    pub fn drag_source(mut self, dnd: Dnd<S>, source: DragSource<S>) -> Self {
        self.drag = Some((dnd, source));
        self
    }

    pub fn build(self, look: &Look, cx: &mut Cx<S>) -> El {
        let Viewport {
            state: st,
            access,
            key,
            pixels,
            label,
            transparent,
            interaction,
            pan_drag,
            overlay,
            on_zoom,
            labels,
            keys,
            drag,
        } = self;
        // Handlers are built here, so each one carries the scale this frame draws at.
        let scale = cx.env().scale;
        let full = interaction == Interaction::Full;
        let placed = match (st.frame(), st.content(), st.level_at(scale)) {
            (Some(frame), Some(content), Some(level)) => Some((frame, content, level)),
            _ => None,
        };

        let mut frame = el()
            .key(key.clone())
            .w_full()
            .h_full()
            .min_w(0.0)
            .min_h(0.0)
            .clip_soft(0.0)
            .instant_changes()
            .role("image");
        if let Some(label) = label {
            frame = frame.label(label);
        }
        {
            let on_zoom = on_zoom.clone();
            frame = frame.on_layout_tx(move |s: &mut S, r: [f32; 4], tx: &mut Tx| {
                let report = access(s).set_frame([r[2], r[3]], scale);
                emit(&on_zoom, s, report, tx);
            });
        }

        match placed {
            Some((frame_size, content, level)) => {
                let drawn = zoom::drawn_size(content, level, scale);
                let visible = [drawn[0].min(frame_size[0]), drawn[1].min(frame_size[1])];
                if transparent && visible[0] > 0.0 && visible[1] > 0.0 {
                    frame = frame.child(checkerboard(format!("{key}-checker"), visible, look).pin(
                        Some((frame_size[0] - visible[0]) / 2.0),
                        Some((frame_size[1] - visible[1]) / 2.0),
                        None,
                        None,
                    ));
                }
                let map = ContentMap {
                    points_per_px: level / scale,
                    size: drawn,
                };
                let mut body = el()
                    .key(format!("{key}-content"))
                    .role("presentation")
                    .pin(
                        Some((frame_size[0] - drawn[0]) / 2.0),
                        Some((frame_size[1] - drawn[1]) / 2.0),
                        None,
                        None,
                    )
                    .size(drawn[0], drawn[1])
                    .live_offset(&st.live_pan);
                if let Some(px) = pixels {
                    // Fit keeps the shape if the pixels' shape differs from the content's
                    // (a stream that changed size before the host said so).
                    body = body.pixels_fit(px, SurfaceFit::Fit);
                }
                if let Some(overlay) = overlay {
                    body = body.child(
                        el().key(format!("{key}-overlay"))
                            .role("presentation")
                            .pin(Some(0.0), Some(0.0), None, None)
                            .size(drawn[0], drawn[1])
                            .child(overlay(&map)),
                    );
                }
                frame = frame.child(body);
                if full {
                    frame = frame.a11y_value((labels.level)(level));
                }
            }
            // Before the first layout: the fit the first layout will land on, so the first
            // frame isn't blank.
            None if st.mode() == Mode::Fit && st.content().is_some() => {
                if let Some(px) = pixels {
                    frame = frame.child(
                        el().key(format!("{key}-content"))
                            .role("presentation")
                            .w_full()
                            .h_full()
                            .pixels_fit(px, SurfaceFit::Fit),
                    );
                }
            }
            None => {}
        }

        let overflow = st.overflow();
        let pans = full
            && match pan_drag {
                PanDrag::Always => true,
                PanDrag::WhenOverflowing => overflow[0] || overflow[1],
                PanDrag::Never => false,
            };
        if !pans && let Some((dnd, source)) = drag {
            frame = dnd.drag_source(frame, source);
        }
        if !full {
            return frame;
        }
        frame = frame
            .focusable(true)
            .focus_ring(2.0, -2.0, look.kit.c.border_focus)
            .on_key(key_handler(
                access,
                on_zoom.clone(),
                keys,
                scale,
                look.space.xxl,
            ))
            .on_wheel(wheel_handler(access, on_zoom.clone(), scale))
            .on_magnify(magnify_handler(access, on_zoom, scale));
        if pans {
            frame =
                frame
                    .cursor(Cursor::Grab)
                    .on_drag(move |s: &mut S, d: &mut Drag, tx: &mut Tx| {
                        access(s).drag(d);
                        tx.live_only();
                    });
        }
        frame
    }
}

fn emit<S>(f: &Option<ZoomFn<S>>, s: &mut S, report: Option<ZoomReport>, tx: &mut Tx) {
    if let (Some(f), Some(report)) = (f, report) {
        f(s, report, tx);
    }
}

fn key_handler<S: 'static>(
    access: fn(&mut S) -> &mut ViewportState,
    on_zoom: Option<ZoomFn<S>>,
    keys: fn(&KeyEvent) -> Option<ZoomKey>,
    scale: f32,
    step: f32,
) -> impl Fn(&mut S, &KeyEvent, &mut Tx) -> bool + 'static {
    move |s: &mut S, e: &KeyEvent, tx: &mut Tx| {
        let Some(k) = keys(e) else { return false };
        let st = access(s);
        st.set_scale(scale);
        if st.level().is_none() {
            return false;
        }
        let report = match k {
            ZoomKey::In => st.zoom_step(1),
            ZoomKey::Out => st.zoom_step(-1),
            ZoomKey::Reset => st.reset(),
            ZoomKey::Pan(x, y) => {
                let o = st.overflow();
                let along = |on: bool, dir: i8| if on { -f32::from(dir) * step } else { 0.0 };
                let d = [along(o[0], x), along(o[1], y)];
                if d == [0.0, 0.0] {
                    return false;
                }
                st.pan_by(d);
                tx.live_only();
                return true;
            }
        };
        emit(&on_zoom, s, report, tx);
        true
    }
}

fn wheel_handler<S: 'static>(
    access: fn(&mut S) -> &mut ViewportState,
    on_zoom: Option<ZoomFn<S>>,
    scale: f32,
) -> impl Fn(&mut S, WheelEvent, &mut Tx) -> bool + 'static {
    move |s: &mut S, e: WheelEvent, tx: &mut Tx| {
        let st = access(s);
        st.set_scale(scale);
        if st.level().is_none() {
            return false;
        }
        if e.mods.ctrl {
            // Ctrl+Shift+wheel is Strata's thumbnail size; Ctrl+Alt is nobody's here.
            if e.mods.shift || e.mods.alt {
                return false;
            }
            let report = st.zoom_by(zoom::wheel_factor(e.dy), Some([e.x, e.y]));
            emit(&on_zoom, s, report, tx);
            return true;
        }
        let o = st.overflow();
        let d = [if o[0] { e.dx } else { 0.0 }, if o[1] { e.dy } else { 0.0 }];
        if d == [0.0, 0.0] {
            // Nothing to pan that way: the scroll view around the viewport scrolls.
            return false;
        }
        st.pan_by(d);
        tx.live_only();
        true
    }
}

fn magnify_handler<S: 'static>(
    access: fn(&mut S) -> &mut ViewportState,
    on_zoom: Option<ZoomFn<S>>,
    scale: f32,
) -> impl Fn(&mut S, MagnifyEvent, &mut Tx) -> bool + 'static {
    move |s: &mut S, e: MagnifyEvent, tx: &mut Tx| {
        let st = access(s);
        st.set_scale(scale);
        if st.level().is_none() {
            return false;
        }
        let report = st.zoom_by(zoom::magnify_factor(e.delta), Some([e.x, e.y]));
        emit(&on_zoom, s, report, tx);
        true
    }
}
