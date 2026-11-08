//! Why a value is a default, or the last good one: reported as data, each kind once.

use std::collections::HashSet;
use std::mem::discriminant;

use super::keys::Key;

/// Why the module fell back. The watcher reports each distinct one once in its life; the app
/// decides what to log. The module never prints.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Fallback {
    /// No session bus to connect to: every value stays its default.
    NoBus(String),
    /// The Settings portal didn't answer: not running, not activatable, or it failed. Values
    /// stay as they were, and the watcher reads again if a portal takes the name.
    NoPortal(String),
    /// The portal's answer lacks this key, so it keeps its default or its last value.
    Unset(Key),
    /// The portal sent this key a value of the wrong type, or an unusable one: the key keeps
    /// its value. `value` is the portal's, as text.
    Invalid { key: Key, value: String },
    /// The portal left the bus. The values it gave are kept.
    PortalLost,
}

impl Fallback {
    fn identity(&self) -> (std::mem::Discriminant<Fallback>, Option<Key>) {
        let key = match self {
            Fallback::Unset(key) | Fallback::Invalid { key, .. } => Some(*key),
            Fallback::NoBus(_) | Fallback::NoPortal(_) | Fallback::PortalLost => None,
        };
        (discriminant(self), key)
    }
}

/// The fallbacks reported so far: a kind (per key, for the per-key kinds) is reported once,
/// whatever its detail text says.
#[derive(Default)]
pub(super) struct Reported(HashSet<(std::mem::Discriminant<Fallback>, Option<Key>)>);

impl Reported {
    /// True the first time `fallback`'s kind (and key) is seen.
    pub(super) fn first(&mut self, fallback: &Fallback) -> bool {
        self.0.insert(fallback.identity())
    }
}
