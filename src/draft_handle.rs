//! Draft record handles: the mint's form, its draw and its freshness.
//!
//! `wvmtrxlv` fixes the rule this module implements: when a draft is opened, its record
//! handle is a fresh token of one fixed form — sixteen characters, each a lowercase ASCII
//! letter or digit — drawn at random independently of the draft and the graph, and redrawn
//! while a record already holds it. `rqtowtqn` fixes what "a record holds it" means: a
//! record is exactly one `<handle>.json` file, so the caller answers that question with the
//! `taken` predicate this module is handed; the module itself reads no registry and holds no
//! state between draws.
//!
//! The draw and the redraw are separate on purpose. `from_bytes` is the whole construction
//! from bytes to token, `first_free` is the redraw loop, and `mint` joins them to the OS
//! source. `szmuyqrq` keeps it one flat file under `src/`; it uses `std` only, so `knqkxzns`'
//! self-contained test tree can compile it as a leaf.

/// The handle alphabet, in order: `0`–`9` then `a`–`z` (`wvmtrxlv`).
const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// A handle is exactly sixteen characters (`wvmtrxlv`).
const LENGTH: usize = 16;

/// Build the handle from entropy bytes: character `i` is `ALPHABET[bytes[i] % 36]`.
///
/// The token is a function of the bytes alone — no counter, no clock, no stored state — so
/// the same bytes always give the same handle. At least [`LENGTH`] bytes are required.
///
/// # Panics
///
/// Panics if fewer than sixteen bytes are given; a short draw has no handle to build.
pub fn from_bytes(bytes: &[u8]) -> String {
    assert!(
        bytes.len() >= LENGTH,
        "a handle takes at least {LENGTH} entropy bytes, got {}",
        bytes.len()
    );
    bytes[..LENGTH]
        .iter()
        .map(|&byte| ALPHABET[usize::from(byte % 36)] as char)
        .collect()
}

/// Draw until a candidate is free, and return it.
///
/// Each attempt is one call to `sample`; a candidate that `taken` refuses is discarded and a
/// fresh one is sampled. Nothing else is consulted and nothing is remembered across calls, so
/// the same samples always produce the same result (`wvmtrxlv`).
pub fn first_free(mut sample: impl FnMut() -> String, taken: impl Fn(&str) -> bool) -> String {
    loop {
        let candidate = sample();
        if !taken(&candidate) {
            return candidate;
        }
    }
}

/// Mint a fresh handle no record already holds (`wvmtrxlv`).
///
/// Sixteen bytes are drawn from the OS source for each attempt and turned into a candidate by
/// [`from_bytes`]; [`first_free`] redraws while `taken` reports that a record holds it. The
/// function takes no draft and no graph entry, so no field of the record can reach the draw.
pub fn mint(taken: impl Fn(&str) -> bool) -> String {
    first_free(draw_from_os, taken)
}

/// One draw of sixteen OS entropy bytes, mapped to a handle.
fn draw_from_os() -> String {
    use std::io::Read;

    let mut bytes = [0u8; LENGTH];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .expect("the OS entropy source /dev/urandom is readable");
    from_bytes(&bytes)
}
