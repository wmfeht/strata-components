//! The in-process fast path: a drag from this process offers a private MIME type naming the
//! drag, `application/x-local-drag;id=<pid>.<counter>.<nonce>`. A drop in this process that
//! sees the token of the drag it started takes the typed payload instead of reading any
//! type. Seeing the drag on our own data device isn't proof (other clients' drags arrive
//! there too); the token is, and the nonce keeps a token another process copies from a
//! listing from matching a later drag of ours.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// The token type's prefix, before `id=`.
pub const LOCAL_MIME: &str = "application/x-local-drag";

/// A new token type for a drag.
pub(crate) fn new_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    // The address of a stack value differs per run with ASLR: mixed in, the nonce isn't
    // guessable from the counter and the clock alone.
    let here = &n as *const u64 as usize as u64;
    let nonce = (u64::from(nanos) ^ here.rotate_left(17) ^ n.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        & 0xffff_ffff_ffff;
    format!("{LOCAL_MIME};id={}.{n}.{nonce:012x}", std::process::id())
}

/// Whether `mime` is a local-drag token at all (from this process or another).
pub(crate) fn is_token(mime: &str) -> bool {
    mime.strip_prefix(LOCAL_MIME)
        .is_some_and(|rest| rest.starts_with(";id="))
}

/// The token among `mimes`, if any.
pub(crate) fn token_in(mimes: &[String]) -> Option<&str> {
    mimes.iter().map(String::as_str).find(|m| is_token(m))
}

/// `mimes` without local-drag tokens: what an app sees.
pub(crate) fn visible(mimes: &[String]) -> Vec<String> {
    mimes.iter().filter(|m| !is_token(m)).cloned().collect()
}

#[cfg(test)]
mod tests;
