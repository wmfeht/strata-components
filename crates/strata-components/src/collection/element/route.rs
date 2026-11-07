//! Input routing: Rune's events on the collection, turned into controller inputs, and the
//! controllers' changes and effects applied.
//!
//! Every pointer handler sits on the collection's container, not on rows: a row's handlers
//! would make it a hit target per row, and Rune reports a `.virtualize` stack's `on_drag` y
//! rebased to the mounted rows (Rune draft U-C1e-1). Content coordinates come from the window
//! point, the viewport's window origin (recorded from `on_layout`) and the scroll offset
//! (followed by `on_scroll_offset`), as `contract::geometry` defines them.

use std::rc::Rc;

use rune::{Drag, KeyEvent, Mods, Phase, PointerButton, PointerEvent, Tx};

use super::seam::{
    Ctx, Effect, Flow, Hit, KeyInput, PointerInput, PointerPhase, Region, default_pointer,
};
use super::{Config, keys};
use crate::collection::selection::Changed;
use crate::contract::{EntrySource, Point, Rect};

/// Where an input came from, for what the element does after it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Source {
    /// A pointer input; for a move, its content point, which a marquee begun by it hears.
    Pointer(Option<Point>),
    Key,
    Tick,
}

/// Runs `f` with the controllers and a [`Ctx`], then applies what it changed and asked for.
fn dispatch<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &mut S,
    tx: &mut Tx,
    from: Source,
    f: impl FnOnce(&mut super::Controllers, &mut Ctx<'_>) -> Flow,
) -> Flow {
    let now = cfg.clock.map_or(0.0, |c| c(s));
    let row_height = (cfg.row_height)((cfg.look)(s));
    let (src, st) = (cfg.get_mut)(s);
    let geometry = cfg
        .lines
        .geometry(src.len(), row_height, st.viewport.w, st.viewport.h);
    let visible = st.visible_content();
    let before = st.selection.cursor();
    let super::CollectionState {
        selection,
        controllers,
        ..
    } = &mut *st;
    let mut cx = Ctx::new(src, &*geometry, visible, now, selection);
    let flow = f(controllers, &mut cx);
    let (changed, effects) = cx.finish();
    let moved = before != st.selection.cursor();
    after(cfg, s, tx, from, changed, moved, effects);
    flow
}

/// What the element does once a controller handled an input.
fn after<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &mut S,
    tx: &mut Tx,
    from: Source,
    changed: Changed,
    moved: bool,
    effects: Vec<Effect>,
) {
    let mut intents = Vec::new();
    let mut revealed = false;
    for effect in effects {
        match effect {
            Effect::Reveal(index) => {
                let (src, st) = (cfg.get_mut)(s);
                st.reveal(&cfg.key, src, index, tx);
                revealed = true;
            }
            Effect::BeginMarquee { at, mods } => {
                // A band begun by a move also takes that move, so it doesn't lag one behind.
                let then = match from {
                    Source::Pointer(moved_to) => moved_to,
                    _ => None,
                };
                begin_marquee(cfg, s, tx, at, mods, then);
            }
            Effect::ScrollTo { y, glide } => {
                let key = keys::viewport_path(&cfg.key);
                if glide {
                    tx.scroll_to(key, 0.0, y);
                } else {
                    tx.scroll_to_instant(key, 0.0, y);
                }
            }
            Effect::KeyboardMode(on) => {
                let st = (cfg.get_mut)(s).1;
                st.keyboard = on;
                st.hover_live = !on;
            }
            Effect::WakeAfter(d) => tx.wake_after(d),
            Effect::Intent(intent) => intents.push(intent),
        }
    }
    // A key that moves the cursor keeps it in view; a pointer moves it only to what it shows.
    if moved && !matches!(from, Source::Pointer(_)) && !revealed {
        let (src, st) = (cfg.get_mut)(s);
        if let Some(index) = st.selection.cursor_index(src) {
            st.reveal(&cfg.key, src, index, tx);
        }
    }
    if changed.cursor
        && let Some(f) = &cfg.on_cursor
    {
        let cursor = (cfg.get)(s).1.selection.cursor();
        f(s, cursor, tx);
    }
    if changed.selection
        && let Some(f) = &cfg.on_selection
    {
        f(s, tx);
    }
    if let Some(f) = &cfg.on_intent {
        for intent in intents {
            f(s, intent, tx);
        }
    }
}

fn begin_marquee<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &mut S,
    tx: &mut Tx,
    at: Point,
    mods: Mods,
    then: Option<Point>,
) {
    let has = (cfg.get)(s).1.controllers.marquee.is_some();
    if !has {
        return;
    }
    dispatch(cfg, s, tx, Source::Pointer(None), |c, cx| {
        if let Some(m) = c.marquee.as_mut() {
            m.begin(at, mods, cx);
            if let Some(p) = then {
                m.moved(p, cx);
            }
        }
        Flow::Handled
    });
}

/// The entry and region under content point `p`.
fn hit<S: 'static>(cfg: &Rc<Config<S>>, s: &S, p: Point) -> Hit {
    let look = (cfg.look)(s);
    let row_height = (cfg.row_height)(look);
    let (src, st) = (cfg.get)(s);
    let geometry = cfg
        .lines
        .geometry(src.len(), row_height, st.viewport.w, st.viewport.h);
    let Some(index) = geometry.index_at(p) else {
        return Hit {
            index: None,
            region: Region::Gutter,
        };
    };
    let rect = geometry.rect_of(index);
    let local = Point::new(p.x - rect.x, p.y - rect.y);
    let region = match &cfg.regions {
        Some(f) => f(s, index, local),
        None => default_region(cfg, s, src, index, local),
    };
    Hit {
        index: Some(index),
        region,
    }
}

/// The default row's regions: its icon (and the gap after it) is content, the drawn width of
/// its name is the name, as Strata counts a label's rendered text (not the space it fills) as
/// the entry; the rest is gutter.
fn default_region<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &S,
    src: &dyn EntrySource,
    index: usize,
    local: Point,
) -> Region {
    let look = (cfg.look)(s);
    let entry = src.entry(index);
    let (label_w, _) = rune::text::measure(entry.name, &[], look.kit.ty.ui_compact, None);
    let left = strata_kit::slots::collection::row_inset(look) + look.space.xs;
    let name = left + look.icon_size.md + look.space.sm;
    if local.x >= name && local.x < name + label_w {
        Region::Name
    } else if local.x >= left && local.x < name {
        Region::Content
    } else {
        Region::Gutter
    }
}

/// `on_drag` on the container.
pub(super) fn on_drag<S: 'static>(cfg: &Rc<Config<S>>, s: &mut S, d: &mut Drag, tx: &mut Tx) {
    let [wx, wy] = d.window_point();
    let window = Point::new(wx, wy);
    let st = (cfg.get)(s).1;
    let phase = match d.phase {
        Phase::Down => PointerPhase::Down,
        Phase::Move => PointerPhase::Move,
        Phase::Up => PointerPhase::Up,
    };
    // A press outside the viewport (on a header) isn't the collection's.
    if phase == PointerPhase::Down && !st.viewport.contains(window) {
        tx.live_only();
        return;
    }
    let point = st.content_point(window);
    {
        let st = (cfg.get_mut)(s).1;
        if phase == PointerPhase::Down {
            st.pending = None;
            st.keyboard = false;
        }
        st.hover_live = true;
    }
    let input = PointerInput {
        phase,
        button: tx.button(),
        point,
        window,
        travel: Point::new(d.dx, d.dy),
        hit: hit(cfg, s, point),
        mods: tx.mods(),
        clicks: d.clicks,
        cancelled: d.cancelled(),
        now: cfg.clock.map_or(0.0, |c| c(s)),
    };
    route_pointer(cfg, s, tx, &input);
}

/// `on_secondary_press_local` on the container.
pub(super) fn on_secondary<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &mut S,
    e: PointerEvent,
    tx: &mut Tx,
) {
    let window = Point::new(e.window_x, e.window_y);
    let st = (cfg.get)(s).1;
    if !st.viewport.contains(window) {
        return;
    }
    let point = st.content_point(window);
    (cfg.get_mut)(s).1.pending = None;
    let input = PointerInput {
        phase: PointerPhase::Down,
        button: PointerButton::Secondary,
        point,
        window,
        travel: Point::default(),
        hit: hit(cfg, s, point),
        mods: tx.mods(),
        clicks: 1,
        cancelled: false,
        now: cfg.clock.map_or(0.0, |c| c(s)),
    };
    route_pointer(cfg, s, tx, &input);
}

fn route_pointer<S: 'static>(cfg: &Rc<Config<S>>, s: &mut S, tx: &mut Tx, input: &PointerInput) {
    let input = *input;
    let moved_to = (input.phase == PointerPhase::Move).then_some(input.point);
    dispatch(cfg, s, tx, Source::Pointer(moved_to), move |c, cx| {
        if let Some(m) = c.marquee.as_mut()
            && m.active()
        {
            match input.phase {
                PointerPhase::Move => m.moved(input.point, cx),
                PointerPhase::Up if input.cancelled => {
                    m.cancel(cx);
                }
                PointerPhase::Up => m.end(cx),
                PointerPhase::Down => {}
            }
            if input.phase != PointerPhase::Down {
                return Flow::Handled;
            }
        }
        match c.pointer.as_mut() {
            Some(p) => p.pointer(&input, cx),
            None => default_pointer(&input, cx),
        }
    });
}

/// `on_key` on the container. True when a controller took the key.
pub(super) fn on_key<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &mut S,
    event: &KeyEvent,
    tx: &mut Tx,
) -> bool {
    let input = KeyInput {
        event,
        physical: tx.physical_key(),
        base: tx.key_base(),
        repeat: tx.key_repeat(),
        now: cfg.clock.map_or(0.0, |c| c(s)),
    };
    let flow = dispatch(cfg, s, tx, Source::Key, |c, cx| {
        if let Some(p) = c.pointer.as_mut() {
            p.key_seen(&input);
        }
        if event.key == rune::Key::Escape
            && let Some(m) = c.marquee.as_mut()
            && m.cancel(cx)
        {
            return Flow::Handled;
        }
        match c.keyboard.as_mut() {
            Some(k) => k.key(&input, cx),
            None => Flow::Pass,
        }
    });
    let st = (cfg.get_mut)(s).1;
    if flow == Flow::Handled {
        st.pending = None;
        st.keyboard = true;
        st.hover_live = false;
        true
    } else {
        false
    }
}

/// `on_pointer_move_local` on the container: hover comes back once the pointer moves.
pub(super) fn on_pointer_move<S: 'static>(cfg: &Rc<Config<S>>, s: &mut S, tx: &mut Tx) {
    let st = (cfg.get_mut)(s).1;
    if st.hover_live {
        tx.live_only();
    } else {
        st.hover_live = true;
    }
}

/// `on_layout_tx` on the viewport: where it shows, as bookkeeping (the view doesn't read it).
pub(super) fn on_layout<S: 'static>(cfg: &Rc<Config<S>>, s: &mut S, r: [f32; 4], tx: &mut Tx) {
    let per = cfg.lines.per_line().max(1);
    let st = (cfg.get_mut)(s).1;
    st.viewport = Rect::from(r);
    st.per_line = per;
    tx.live_only();
}

/// `on_visible_rows` on the rows stack, with lines turned into entries.
pub(super) fn on_visible<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &mut S,
    lines: std::ops::Range<usize>,
) -> bool {
    let per = cfg.lines.per_line().max(1);
    let (src, st) = (cfg.get_mut)(s);
    let len = src.len();
    let range = (lines.start * per).min(len)..(lines.end * per).min(len);
    st.visible = range.clone();
    st.per_line = per;
    match &cfg.on_visible {
        Some(f) => f(s, range),
        None => false,
    }
}

/// The controllers' timeouts, from the host's poll.
pub(super) fn tick<S: 'static>(cfg: &Rc<Config<S>>, s: &mut S, tx: &mut Tx) -> bool {
    let mut handled = false;
    dispatch(cfg, s, tx, Source::Tick, |c, cx| {
        for flow in [
            c.pointer.as_mut().map(|p| p.tick(cx)),
            c.keyboard.as_mut().map(|k| k.tick(cx)),
            c.marquee.as_mut().map(|m| m.tick(cx)),
        ]
        .into_iter()
        .flatten()
        {
            handled |= flow == Flow::Handled;
        }
        Flow::Pass
    });
    handled
}

/// A drag over the collection near the viewport's top or bottom edge glides it that way, a
/// row's height a move, so a drop target out of view can be reached.
pub(super) fn drop_edge_scroll<S: 'static>(
    cfg: &Rc<Config<S>>,
    s: &mut S,
    point: PointerEvent,
    tx: &mut Tx,
) {
    let row_height = (cfg.row_height)((cfg.look)(s));
    let st = (cfg.get)(s).1;
    let y = point.window_y - st.viewport.y;
    let offset = st.scroll_offset();
    let to = if y < row_height {
        (offset - row_height).max(0.0)
    } else if y > st.viewport.h - row_height {
        offset + row_height
    } else {
        tx.live_only();
        return;
    };
    tx.scroll_to(keys::viewport_path(&cfg.key), 0.0, to);
}
