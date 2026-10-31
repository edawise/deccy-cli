//! The draft's content identity: one digest per owned path.
//!
//! S5-identity (skooqvul) fixes the encoding the record's base was left to the model to name:
//! the SHA-256 digest of a path's content, rendered as exactly 64 lowercase hexadecimal
//! characters, high nibble first, with no prefix, separator or decoration, and with no path
//! name, node id, change id or other metadata folded in. What is hashed is the content byte
//! for byte, not framed, re-encoded, newline-normalised or trimmed. S5-base (qrqsxyzz) fixes
//! that a base is a value per path of the owned set — one entry for every path the set holds,
//! determined by that path's content and by nothing else — and S4-base (zsvukpls) states it as
//! the requirement the opening applies. M-layout (szmuyqrq) places the module flat at
//! `src/draft_identity.rs`.
//!
//! This module is that encoding and nothing above it: it takes bytes and returns their
//! identity, and it takes an owned set's paths with their content and returns the base. It
//! makes no jj call and reads no working copy, because reading each owned path at the change
//! that owns it is the reach of `src/jjread.rs` (osplmlvq) and the attribution of
//! `src/node_own.rs` (lwnnssuo); it does not write the record, decide when the base is taken,
//! or compare a base against a later owned set, which belong to the record model above it.
//!
//! The digest itself is the journal's (zutqsqtu): `sha256_hex` is the journal store's FIPS
//! 180-4 SHA-256 and its lowercase hex rendering, reused here rather than copied, so the base
//! and the journal's content-addressed blobs are one function and there is no second digest in
//! the crate to drift from the first.

use std::collections::BTreeMap;

use crate::journal::sha256_hex;

/// The content identity of `bytes` (S5-identity, skooqvul): the SHA-256 of exactly these bytes,
/// rendered as 64 lowercase hexadecimal characters.
///
/// The bytes are the digest input as they are. Equal bytes at any path and in any change give
/// an equal identity, a one-byte difference gives a different one, and the path the content
/// sits at is not part of the input.
pub fn identity(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

/// The base of an owned set (S5-base, qrqsxyzz; S4-base, zsvukpls): an entry for every path in
/// `contents`, each the identity of that path's content and nothing else.
///
/// The owned set is the paths the caller hands over — the node document and the paths its
/// `content.ref` lists, when the call above it has resolved them — with the content read from
/// the change that owns it. A path's name keys its entry and is not folded into the value, so
/// the same bytes under two names yield the same identity value; an empty set yields an empty
/// base.
pub fn base<'a>(
    contents: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> BTreeMap<String, String> {
    contents
        .into_iter()
        .map(|(path, bytes)| (path.to_owned(), identity(bytes)))
        .collect()
}
