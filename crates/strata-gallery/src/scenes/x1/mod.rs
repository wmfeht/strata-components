//! The gallery page for X1 Shell demo: Strata's header component and the shell window
//! `examples/shell` runs, on fake data. Scene names start with `x1-` and end in `-light` or
//! `-dark`.
//!
//! Work package: `docs/work/phase-2-composites/X1-shell-demo.md`.
//!
//! - `x1-header-states-*` stacks five headers: a fresh profile (Close only), X11 with every
//!   button, a maximised window, Wayland with every button turned on (Minimize can't work and
//!   the maximised state is unknown), and a window without focus.
//! - `x1-shell-*` is the themed empty window (a bundled theme, the desktop's settings, a fake
//!   overrides file) with its header and readout; `x1-shell-maximized-*` the same maximised.
//! - `x1-volumes-*` has the mounted-volumes dialog open over it.
//!
//! The page stands in for the platform: a press on Maximize flips the window's maximised state
//! as an X11 report would, the volumes arrive [`VOLUMES_AFTER`] seconds after they are asked
//! for, and keys stand in for the user and the desktop (all unmodified):
//!
//! | Key | Does |
//! | --- | --- |
//! | `1`, `2`, `3` | Turns the Minimize, Maximize or Close button preference off or on |
//! | `e` | Edits the fake overrides file: it alternates between [`FILE`] and [`FILE_BROKEN`] |
//! | `d` | Deletes the fake file |
//! | `s` | The desktop's text scale moves between 1 and 1.25 |
//! | `a` | The desktop's animations switch |
//!
//! A file edit is read [`reload::SETTLE`] after the last one, as `examples/shell` reads the
//! real file; the look then moves through `theme::switch`, as a theme switch does.

pub mod reload;
pub mod shell;

use rune::{Cx, El, Key, KeyEvent, Mods, Tx, col, text};
use strata_components::header::{WindowAction, WindowButtons, WindowChrome};
use strata_kit::overrides::Overrides;
use strata_kit::theme::{self, Theme, bundled_theme};

use crate::{Gallery, Scene};
use reload::Debounce;
use shell::{Hooks, ShellWindow, Volume, Volumes};

/// The fake overrides file the shell scenes open with: a Frame density inset, and the bar's
/// fill and hairline colours per appearance, so the header shows each.
pub const FILE: &str = r##"# The gallery's fake overrides.toml: the page never reads a real file.
frame.inset = 3

[light]
color.border_base = "#d20f39"
color.bg_deep = "#e6e9ef"

[dark]
color.border_base = "#7aa2f7"
color.bg_deep = "#16161e"
"##;

/// [`FILE`] with a row that has a problem: it doesn't apply, and the rest still does.
pub const FILE_BROKEN: &str = r##"# The gallery's fake overrides.toml: the page never reads a real file.
frame.inset = 3
color.text_base = "nope"

[light]
color.border_base = "#d20f39"
color.bg_deep = "#e6e9ef"

[dark]
color.border_base = "#7aa2f7"
color.bg_deep = "#16161e"
"##;

/// How long the fake volume listing takes, in seconds.
pub const VOLUMES_AFTER: f64 = 0.2;

/// What the page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// One shell window.
    #[default]
    Shell,
    /// Five headers, one per window state.
    HeaderStates,
}

/// X1's demo state, held in `Gallery::x1`.
#[derive(Default)]
pub struct Demo {
    pub mode: Mode,
    pub window: ShellWindow,
    /// The bundled theme's id.
    pub theme: &'static str,
    /// The fake overrides file: `None` when deleted.
    pub file: Option<&'static str>,
    pub overrides: Overrides,
    pub debounce: Debounce,
    /// A file edit the next poll times.
    pub edited: bool,
    /// The volumes were asked for; the next poll times the fake listing.
    pub volumes_asked: bool,
    pub volumes_due: Option<f64>,
}

fn theme_for(id: &str) -> &'static Theme {
    bundled_theme(id).unwrap_or_else(|| panic!("{id} is bundled"))
}

fn fake_volumes() -> Vec<Volume> {
    vec![
        Volume {
            name: "System".into(),
            detail: "/".into(),
        },
        Volume {
            name: "Photos 2024".into(),
            detail: "/run/media/ada/Photos 2024".into(),
        },
        Volume {
            name: "projects on nas.local".into(),
            detail: "smb://nas.local/projects".into(),
        },
    ]
}

/// The X11 window manager's report for a window that can do everything.
fn x11(maximized: bool) -> WindowChrome {
    WindowChrome {
        maximized: Some(maximized),
        fullscreen: Some(false),
        can_minimize: Some(true),
        can_maximize: Some(true),
        active: true,
    }
}

fn open(g: &mut Gallery, dark: bool, mode: Mode) {
    g.open("x1", dark);
    let id = if dark { "tokyo-night" } else { "omarchy-light" };
    let file = Some(FILE);
    let (overrides, present) = reload::read(file.map(str::as_bytes));
    let window = ShellWindow {
        location: "~/Projects".into(),
        buttons: WindowButtons::ALL,
        chrome: x11(false),
        overrides: reload::status(&overrides, present, 0),
        ..ShellWindow::default()
    };
    g.look = reload::build(theme_for(id), &window.desktop, &overrides);
    g.dark = g.look.kit.dark;
    g.x1 = Demo {
        mode,
        window,
        theme: id,
        file,
        overrides,
        ..Demo::default()
    };
}

fn maximized(g: &mut Gallery, dark: bool) {
    open(g, dark, Mode::Shell);
    g.x1.window.chrome = x11(true);
}

fn volumes(g: &mut Gallery, dark: bool, volumes: Volumes) {
    open(g, dark, Mode::Shell);
    g.x1.window.volumes_open = true;
    g.x1.window.volumes = volumes;
}

/// Golden scenes: each gets an accessibility and a PNG golden.
pub const SCENES: &[Scene] = &[
    ("x1-header-states-light", |g| {
        open(g, false, Mode::HeaderStates)
    }),
    ("x1-header-states-dark", |g| {
        open(g, true, Mode::HeaderStates)
    }),
    ("x1-shell-light", |g| open(g, false, Mode::Shell)),
    ("x1-shell-dark", |g| open(g, true, Mode::Shell)),
    ("x1-shell-maximized-light", |g| maximized(g, false)),
    ("x1-shell-maximized-dark", |g| maximized(g, true)),
    ("x1-volumes-light", |g| {
        volumes(g, false, Volumes::Listed(fake_volumes()))
    }),
    ("x1-volumes-dark", |g| {
        volumes(g, true, Volumes::Listed(fake_volumes()))
    }),
];

/// Scenes for review with `--scene` only: the dialog's other states.
pub const REVIEW: &[Scene] = &[
    ("x1-volumes-loading-light", |g| {
        volumes(g, false, Volumes::Loading)
    }),
    ("x1-volumes-loading-dark", |g| {
        volumes(g, true, Volumes::Loading)
    }),
    ("x1-volumes-empty-light", |g| {
        volumes(g, false, Volumes::Listed(Vec::new()))
    }),
    ("x1-volumes-empty-dark", |g| {
        volumes(g, true, Volumes::Listed(Vec::new()))
    }),
    ("x1-volumes-failed-light", |g| volumes(g, false, failed())),
    ("x1-volumes-failed-dark", |g| volumes(g, true, failed())),
];

fn failed() -> Volumes {
    Volumes::Failed("No GLib bridge: the GLib thread couldn't start".into())
}

fn window(g: &mut Gallery) -> &mut ShellWindow {
    &mut g.x1.window
}

/// Stands in for the window manager: Maximize flips the state as an X11 report would.
fn on_action(g: &mut Gallery, action: WindowAction, _: &mut Tx) {
    let chrome = &mut g.x1.window.chrome;
    if action == WindowAction::ToggleMaximize {
        chrome.maximized = chrome.maximized.map(|m| !m);
    }
}

fn on_volumes(g: &mut Gallery) {
    g.x1.window.volumes = Volumes::Loading;
    g.x1.volumes_asked = true;
}

const HOOKS: Hooks<Gallery> = Hooks {
    at: window,
    on_action,
    on_volumes,
};

pub fn view(g: &Gallery, cx: &mut Cx<Gallery>) -> El {
    match g.x1.mode {
        Mode::Shell => shell::view(&g.x1.window, g.look(), cx, HOOKS, |bar| bar),
        Mode::HeaderStates => header_states(g, cx),
    }
}

/// The five header states, each under a caption, each with its own keys.
fn header_states(g: &Gallery, cx: &mut Cx<Gallery>) -> El {
    let look = g.look();
    let k = &look.kit;
    let all = WindowButtons::ALL;
    let states = [
        (
            "fresh",
            "A fresh profile",
            WindowButtons::default(),
            x11(false),
        ),
        ("x11", "X11, every button", all, x11(false)),
        ("maximized", "X11, maximised", all, x11(true)),
        (
            "wayland",
            "Wayland, every button turned on",
            all,
            WindowChrome {
                maximized: None,
                fullscreen: None,
                can_minimize: Some(false),
                can_maximize: None,
                active: true,
            },
        ),
        (
            "backdrop",
            "A window without focus",
            all,
            WindowChrome {
                active: false,
                ..x11(false)
            },
        ),
    ];
    let reduced = !look.animations || cx.system_reduced_motion();
    cx.reduced_motion(reduced);
    col()
        .key("x1-header-states")
        .w_full()
        .h_full()
        .fill(k.c.bg_base)
        .gap(look.space.md)
        .py(look.space.lg)
        .children(states.into_iter().map(|(id, caption, buttons, chrome)| {
            let w = ShellWindow {
                location: "~/Projects".into(),
                buttons,
                chrome,
                ..ShellWindow::default()
            };
            col()
                .key(format!("x1-state-{id}"))
                .w_full()
                .gap(look.space.xs)
                .child(
                    text(caption)
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .px(look.space.lg),
                )
                .child(shell::bar(&w, look, &format!("x1-{id}"), false, HOOKS))
        }))
}

/// Keys no focused element took while this page is open (the table in the module docs).
pub fn on_key(g: &mut Gallery, key: &KeyEvent, tx: &mut Tx) -> bool {
    let Key::Text(t) = &key.key else {
        return false;
    };
    if key.mods != Mods::default() {
        return false;
    }
    let d = &mut g.x1;
    let buttons = &mut d.window.buttons;
    match t.as_str() {
        "1" => buttons.minimize = !buttons.minimize,
        "2" => buttons.maximize = !buttons.maximize,
        "3" => buttons.close = !buttons.close,
        "e" => {
            d.file = Some(if d.file == Some(FILE) {
                FILE_BROKEN
            } else {
                FILE
            });
            d.edited = true;
        }
        "d" => {
            d.file = None;
            d.edited = true;
        }
        "s" => {
            let desktop = &mut d.window.desktop;
            desktop.text_scale = if desktop.text_scale == 1.0 { 1.25 } else { 1.0 };
            relook(g, tx);
        }
        "a" => {
            d.window.desktop.animations = !d.window.desktop.animations;
            relook(g, tx);
        }
        _ => return false,
    }
    true
}

/// Rebuilds the look from the theme, the desktop's settings and the overrides, and moves to it
/// as a theme switch does.
fn relook(g: &mut Gallery, tx: &mut Tx) {
    let d = &g.x1;
    let next = reload::build(theme_for(d.theme), &d.window.desktop, &d.overrides);
    theme::switch(&mut g.look, next, tx);
}

/// Times the fake file's reads and the fake volume listing on the page's clock.
pub fn poll(g: &mut Gallery, t: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    let d = &mut g.x1;
    if std::mem::take(&mut d.edited) {
        let due = d.debounce.changed(t);
        tx.wake_after(std::time::Duration::from_secs_f64((due - t).max(0.0)));
    }
    if d.debounce.take_due(t) {
        let (overrides, present) = reload::read(d.file.map(str::as_bytes));
        let reloads = d.window.overrides.reloads + 1;
        d.window.overrides = reload::status(&overrides, present, reloads);
        d.overrides = overrides;
        relook(g, tx);
        changed = true;
    }
    let d = &mut g.x1;
    if std::mem::take(&mut d.volumes_asked) {
        d.volumes_due = Some(t + VOLUMES_AFTER);
        tx.wake_after(std::time::Duration::from_secs_f64(VOLUMES_AFTER));
    }
    if d.volumes_due.is_some_and(|due| t + 1e-6 >= due) {
        d.volumes_due = None;
        d.window.volumes = Volumes::Listed(fake_volumes());
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests;
