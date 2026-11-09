//! ICCCM 2.6.2 `MULTIPLE`: one request for several targets at once, each into its own
//! property. The requestor names the pairs (target, property) in an `ATOM_PAIR` property;
//! the owner writes each target into its property, sets a pair's property to `None` where it
//! refuses that target, and answers with one `SelectionNotify`. Clipboard managers take a
//! selection this way when its owner hands it over (`SAVE_TARGETS`, see `handoff`).
//!
//! A pair's data larger than one chunk goes by INCR, each on its own property, as a single
//! request's does.

use std::sync::Arc;

use x11rb::NONE;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, PropMode};
use x11rb::wrapper::ConnectionExt as _;

use super::super::data::SourceSpec;
use super::super::event::{Event, SourceId};
use super::selection::Request;
use super::{Actor, Cmd};

/// What one pair gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Answer {
    /// Refused: the pair's property becomes `None` in the list.
    Refused,
    /// 32-bit values of this type (`TARGETS`, `TIMESTAMP`).
    Words(Atom, Vec<u32>),
    /// The bytes of a type, as its writer made them.
    Bytes(Arc<[u8]>),
}

struct Pair {
    target: Atom,
    property: Atom,
    answer: Option<Answer>,
}

/// A `MULTIPLE` request waiting for its lazy writers.
pub(super) struct Multi {
    key: u64,
    req: Request,
    pairs: Vec<Pair>,
}

/// The pairs of an `ATOM_PAIR` list, or None if it isn't one (an odd count).
pub(super) fn parse_pairs(words: &[u32]) -> Option<Vec<(Atom, Atom)>> {
    let (pairs, rest) = words.as_chunks::<2>();
    rest.is_empty()
        .then(|| pairs.iter().map(|[t, p]| (*t, *p)).collect())
}

/// The list written back: each pair as asked, with the property of a refused one `None`.
pub(super) fn answered_pairs(pairs: &[(Atom, Atom)], refused: &[bool]) -> Vec<u32> {
    pairs
        .iter()
        .zip(refused)
        .flat_map(|(&(t, p), &no)| [t, if no { NONE } else { p }])
        .collect()
}

impl Actor {
    /// A `MULTIPLE` request for a selection we hold as `id` with `spec`.
    pub(super) fn multiple(&mut self, req: Request, id: SourceId, spec: SourceSpec, time: u32) {
        // An obsolete requestor names no property, and MULTIPLE can't work without one. A
        // property named MULTIPLE is fine: gsd-clipboard and xfsettingsd ask into it.
        if req.obsolete {
            return self.notify(&req, NONE);
        }
        let words = self
            .conn
            .get_property(
                false,
                req.requestor,
                req.property,
                AtomEnum::ANY,
                0,
                1 << 16,
            )
            .ok()
            .and_then(|c| c.reply().ok())
            .filter(|r| r.format == 32)
            .and_then(|r| r.value32().map(Iterator::collect::<Vec<u32>>));
        let Some(asked) = words.as_deref().and_then(parse_pairs) else {
            return self.notify(&req, NONE);
        };
        let key = self.next_multi;
        self.next_multi += 1;
        let mut pairs = Vec::with_capacity(asked.len());
        let mut lazy = Vec::new();
        for (i, &(target, property)) in asked.iter().enumerate() {
            let answer = if property == NONE || target == self.atoms.MULTIPLE {
                Some(Answer::Refused)
            } else if target == self.atoms.TARGETS {
                let mut list = vec![
                    self.atoms.TARGETS,
                    self.atoms.TIMESTAMP,
                    self.atoms.MULTIPLE,
                ];
                let mimes: Vec<String> = spec.mimes().map(str::to_owned).collect();
                list.extend(mimes.iter().filter_map(|m| self.atom(m)));
                Some(Answer::Words(self.atoms.ATOM, list))
            } else if target == self.atoms.TIMESTAMP {
                Some(Answer::Words(self.atoms.INTEGER, vec![time]))
            } else {
                match self
                    .name(target)
                    .and_then(|m| spec.writer(&m).cloned().map(|w| (m, w)))
                {
                    None => Some(Answer::Refused),
                    Some((mime, writer)) => {
                        self.events.push(Event::SourceAsked { source: id, mime });
                        match writer.ready() {
                            Some(Some(bytes)) => Some(Answer::Bytes(bytes)),
                            Some(None) => Some(Answer::Refused),
                            None => {
                                lazy.push((i, writer));
                                None
                            }
                        }
                    }
                }
            };
            pairs.push(Pair {
                target,
                property,
                answer,
            });
        }
        self.multis.push(Multi { key, req, pairs });
        for (index, writer) in lazy {
            // Each writer runs on a worker; its bytes come back as a command.
            let mailbox = self.mailbox.clone();
            let spawned = std::thread::Builder::new()
                .name("transfer-write".into())
                .spawn(move || {
                    let bytes = writer.get();
                    let _ = mailbox.post(Cmd::MultiReady { key, index, bytes });
                });
            if spawned.is_err() {
                self.multi_ready(key, index, None);
            }
        }
        self.multi_ready(key, usize::MAX, None);
    }

    /// A writer of `MULTIPLE` request `key` finished (`index` past the pairs: none did, just
    /// check). Once every pair has its answer, the request is answered.
    pub(super) fn multi_ready(&mut self, key: u64, index: usize, bytes: Option<Arc<[u8]>>) {
        let Some(m) = self.multis.iter().position(|m| m.key == key) else {
            return;
        };
        if let Some(pair) = self.multis[m].pairs.get_mut(index) {
            pair.answer = Some(bytes.map_or(Answer::Refused, Answer::Bytes));
        }
        if self.multis[m].pairs.iter().any(|p| p.answer.is_none()) {
            return;
        }
        let multi = self.multis.swap_remove(m);
        let requestor = multi.req.requestor;
        let mut asked = Vec::with_capacity(multi.pairs.len());
        let mut refused = Vec::with_capacity(multi.pairs.len());
        for pair in multi.pairs {
            asked.push((pair.target, pair.property));
            match pair.answer.unwrap_or(Answer::Refused) {
                Answer::Refused => refused.push(true),
                Answer::Words(ty, words) => {
                    let _ = self.conn.change_property32(
                        PropMode::REPLACE,
                        requestor,
                        pair.property,
                        ty,
                        &words,
                    );
                    refused.push(false);
                }
                Answer::Bytes(data) => {
                    self.put(requestor, pair.property, pair.target, data);
                    refused.push(false);
                }
            }
        }
        let list = answered_pairs(&asked, &refused);
        let _ = self.conn.change_property32(
            PropMode::REPLACE,
            requestor,
            multi.req.property,
            self.atoms.ATOM_PAIR,
            &list,
        );
        self.notify(&multi.req, multi.req.property);
    }
}

#[cfg(test)]
mod tests;
