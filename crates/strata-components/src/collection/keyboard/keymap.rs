//! The key map: which chords run which [`Command`]s, and which keys stand in for the arrows.

use rune::{Chord, ChordKey, ChordMod};

use super::command::{Command, RangeKind, Target, Vertical};
use super::settings::Letters;
use crate::collection::element::KeyInput;
use crate::contract::Direction;

/// Which chords run which commands, and which other keys act as an arrow.
///
/// [`KeyMap::default_map`] and [`KeyMap::tenxer`] are Strata's shipped tables
/// (`src/ui/keys/bindings/navigation.rs` and `tenxer.rs` at `upstream/main` 35ee677ab). A host
/// changes them as its users rebind keys: [`KeyMap::bind`], [`KeyMap::unbind`],
/// [`KeyMap::alias`]. Checking a binding against the rest of the table (Strata's refused and
/// noted keys) is the host's.
///
/// Keys match with [`KeyInput::matches`]: Rune's layout-aware chords, so a letter follows the
/// key's label under the active layout and a key that types a non-Latin character falls back
/// to the US key in its place. Modifiers match exactly, as Strata's do: Ctrl+Shift+Up is not
/// Ctrl+Up, so it doesn't jump.
/// Bindings apply whatever [`Letters`] says. An alias stands in for an arrow, as `h j k l` do
/// in the default map; one on a plain printable key (no Ctrl, Alt or Super) applies only with
/// [`Letters::Arrows`], and only without modifiers, so Shift+J doesn't extend.
///
/// Used by: C19 (the shortcut reference), C20 (keyboard settings), and hosts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyMap {
    bindings: Vec<(Chord, Command)>,
    aliases: Vec<(Chord, ChordKey)>,
}

impl KeyMap {
    /// No bindings and no aliases.
    pub fn new() -> Self {
        Self::default()
    }

    /// Strata's default map, in every view and the file chooser. `h j k l` alias the arrows.
    pub fn default_map() -> Self {
        let mut map = Self::new();
        for d in DIRECTIONS {
            let k = arrow(d);
            map.bind(plain(k), Command::Select(Target::Step(d)));
            map.bind(shift(k), Command::Extend(Target::Step(d)));
        }
        let ends = [
            (ChordKey::Home, Target::First),
            (ChordKey::End, Target::Last),
            (ChordKey::PageUp, Target::Page(Vertical::Up)),
            (ChordKey::PageDown, Target::Page(Vertical::Down)),
        ];
        for (k, target) in ends {
            map.bind(plain(k), Command::Select(target));
            map.bind(shift(k), Command::Extend(target));
        }
        map.bind(ctrl(ChordKey::Left), Command::Cursor(left()));
        map.bind(ctrl(ChordKey::Right), Command::Cursor(right()));
        map.bind(ctrl(ChordKey::Up), Command::Select(Target::First));
        map.bind(ctrl(ChordKey::Down), Command::Select(Target::Last));
        map.bind(ctrl(ChordKey::Char(' ')), Command::Toggle);
        map.bind(ctrl(ChordKey::Char('a')), Command::SelectAll);
        map.bind(plain(ChordKey::Escape), Command::Clear);
        map.bind(plain(ChordKey::Enter), Command::Activate);
        for (c, d) in VIM {
            map.alias(plain(ChordKey::Char(c)), arrow(d));
        }
        map
    }

    /// Strata's 10xer map. Every move keeps the selection; `h j k l` are bindings of their own,
    /// so they move whatever [`Letters`] says. Escape is left to the host's Escape ladder
    /// (filter, find highlights, range, preview, then the selection), which ends a range with
    /// [`KeyHandle::end_range`](super::KeyHandle::end_range).
    pub fn tenxer() -> Self {
        let mut map = Self::new();
        for d in DIRECTIONS {
            map.bind(plain(arrow(d)), Command::Cursor(Target::Step(d)));
            // `v` and `V` are the only range keys.
            map.bind(shift(arrow(d)), Command::Swallow);
        }
        for (c, d) in VIM {
            map.bind(plain(ChordKey::Char(c)), Command::Cursor(Target::Step(d)));
        }
        let up = Target::Page(Vertical::Up);
        let down = Target::Page(Vertical::Down);
        let moves = [
            (plain(ChordKey::Home), Target::First),
            (plain(ChordKey::End), Target::Last),
            (shift(ChordKey::Char('g')), Target::Last),
            // Recorded decision: in 10xer, Ctrl+Up and Ctrl+Down move the cursor only.
            (ctrl(ChordKey::Up), Target::First),
            (ctrl(ChordKey::Down), Target::Last),
            (plain(ChordKey::PageUp), up),
            (plain(ChordKey::PageDown), down),
            (ctrl(ChordKey::Char('b')), up),
            (ctrl(ChordKey::Char('f')), down),
            (ctrl(ChordKey::PageUp), up),
            (ctrl(ChordKey::PageDown), down),
            (ctrl(ChordKey::Char('u')), Target::HalfPage(Vertical::Up)),
            (ctrl(ChordKey::Char('d')), Target::HalfPage(Vertical::Down)),
        ];
        for (chord, target) in moves {
            map.bind(chord, Command::Cursor(target));
        }
        map.bind(plain(ChordKey::Char(' ')), Command::ToggleAdvance);
        map.bind(ctrl(ChordKey::Char(' ')), Command::Toggle);
        map.bind(
            plain(ChordKey::Char('v')),
            Command::Range(RangeKind::Select),
        );
        map.bind(shift(ChordKey::Char('v')), Command::Range(RangeKind::Unset));
        map.bind(ctrl(ChordKey::Char('a')), Command::SelectAll);
        map.bind(ctrl(ChordKey::Char('r')), Command::Invert);
        map.bind(plain(ChordKey::Enter), Command::Activate);
        map.bind(plain(ChordKey::Char('o')), Command::Activate);
        map
    }

    /// Binds `chord` to `command`, after any binding it already has. A chord bound twice runs
    /// its first command.
    pub fn bind(&mut self, chord: Chord, command: Command) -> &mut Self {
        self.bindings.push((chord, command));
        self
    }

    /// Removes every chord bound to `command`.
    pub fn unbind(&mut self, command: Command) -> &mut Self {
        self.bindings.retain(|(_, c)| *c != command);
        self
    }

    /// Removes the binding of `chord`, whatever it runs.
    pub fn unbind_chord(&mut self, chord: Chord) -> &mut Self {
        self.bindings.retain(|(c, _)| *c != chord);
        self
    }

    /// Makes `chord` act as the plain key `arrow` (an arrow, or any key the map binds plainly):
    /// it runs what that key is bound to.
    pub fn alias(&mut self, chord: Chord, arrow: ChordKey) -> &mut Self {
        self.aliases.push((chord, arrow));
        self
    }

    /// Removes every alias.
    pub fn clear_aliases(&mut self) -> &mut Self {
        self.aliases.clear();
        self
    }

    /// The chords that run `command`, bindings first, then aliases of the keys bound to it.
    pub fn chords_for(&self, command: Command) -> Vec<Chord> {
        let mut chords: Vec<Chord> = self
            .bindings
            .iter()
            .filter(|(_, c)| *c == command)
            .map(|(chord, _)| *chord)
            .collect();
        for (alias, key) in &self.aliases {
            if self.bound(plain(*key)) == Some(command) {
                chords.push(*alias);
            }
        }
        chords
    }

    /// Every binding, in order.
    pub fn bindings(&self) -> &[(Chord, Command)] {
        &self.bindings
    }

    /// The command `input` runs.
    pub(super) fn lookup(&self, input: &KeyInput<'_>, letters: Letters) -> Option<Command> {
        let hit = |c: &Chord| input.matches(c);
        if let Some((_, command)) = self.bindings.iter().find(|(c, _)| hit(c)) {
            return Some(*command);
        }
        self.aliases
            .iter()
            .filter(|(c, _)| letters == Letters::Arrows || !typed(c))
            .find(|(c, _)| hit(c))
            .and_then(|(_, key)| self.bound(plain(*key)))
    }

    fn bound(&self, chord: Chord) -> Option<Command> {
        self.bindings
            .iter()
            .find(|(c, _)| *c == chord)
            .map(|(_, command)| *command)
    }
}

const DIRECTIONS: [Direction; 4] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];

const VIM: [(char, Direction); 4] = [
    ('h', Direction::Left),
    ('j', Direction::Down),
    ('k', Direction::Up),
    ('l', Direction::Right),
];

const fn left() -> Target {
    Target::Step(Direction::Left)
}

const fn right() -> Target {
    Target::Step(Direction::Right)
}

const fn arrow(d: Direction) -> ChordKey {
    match d {
        Direction::Up => ChordKey::Up,
        Direction::Down => ChordKey::Down,
        Direction::Left => ChordKey::Left,
        Direction::Right => ChordKey::Right,
    }
}

const fn plain(key: ChordKey) -> Chord {
    Chord::new(key)
}

const fn shift(key: ChordKey) -> Chord {
    Chord::new(key).with(ChordMod::Shift)
}

const fn ctrl(key: ChordKey) -> Chord {
    Chord::new(key).with(ChordMod::Ctrl)
}

/// Whether `chord` is a plain printable key: one that types in a text field, and that
/// type-to-search claims.
fn typed(chord: &Chord) -> bool {
    let held = [
        ChordMod::Primary,
        ChordMod::Ctrl,
        ChordMod::Alt,
        ChordMod::Super,
    ];
    matches!(chord.key(), ChordKey::Char(_)) && !held.iter().any(|m| chord.has(*m))
}
