//! Gate for M-draft-identity (a draft's content-identity base).
//!
//! Self-contained per M-tests: the `#[path]` shims pull in the module under test and the
//! modules it may build on, exactly as the binary's module tree is shaped. `store.rs` is
//! std-only and `journal.rs` needs only `std` + `serde` + `serde_json` + `crate::store`
//! (plus the embedded schema), so the module may either be a std-only leaf or reuse
//! `crate::journal::sha256_hex` — both must compile and pass. The digest is still pinned
//! from first principles against published SHA-256 vectors, so neither reuse nor duplication
//! is trusted to itself.

#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/journal.rs"]
mod journal;
#[path = "../../../src/draft_identity.rs"]
mod draft_identity;

use std::collections::{BTreeMap, BTreeSet};

use draft_identity::{base, identity};

/// SHA-256 of the empty input (FIPS 180-4; NIST CAVP vector).
const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// SHA-256 of `abc` (NIST CAVP vector).
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
/// SHA-256 of `abc` followed by a newline: `printf 'abc\n' | shasum -a 256`.
const ABC_NEWLINE: &str = "edeaaff3f1774ad2888673770c6d64097e391bc362d7d6fb34982ddf0efd18cb";
/// SHA-256 of a space, `abc`, and a newline: `printf ' abc\n' | shasum -a 256`.
const LEADING_SPACE_NEWLINE: &str = "0e876e95f5e8a2d805f13659cf5edeb8dcf6addc50dee5f84ef6f0df42cf1b8e";
/// SHA-256 of a tab followed by `abc`: `printf '\tabc' | shasum -a 256`.
const LEADING_TAB_ABC: &str = "0feab523d881a68c54faf47f5e89fcdc59df001cc4921196d8a5292def2d7383";
/// SHA-256 of the 448-bit NIST message `abcdbcdec...nopq`.
const TWO_BLOCK: &str = "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1";
/// SHA-256 of the 256-byte sequence `0x00, 0x01, ..., 0xff`.
const BINARY: &str = "40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880";

/// A content identity is exactly 64 lowercase hexadecimal characters.
fn is_lowercase_hex64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[test]
fn identity_of_the_empty_input_is_pinned() {
    assert_eq!(identity(b""), EMPTY, "the empty input digests to the empty vector");
}

#[test]
fn identity_of_abc_is_pinned() {
    assert_eq!(identity(b"abc"), ABC, "abc digests to the published vector");
}

#[test]
fn identity_of_the_nist_two_block_vector_is_pinned() {
    let message = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(
        identity(message),
        TWO_BLOCK,
        "the NIST 448-bit message digests to its published vector"
    );
}

#[test]
fn identity_of_a_long_binary_vector_is_pinned() {
    let message: Vec<u8> = (0..=255u8).collect();
    assert_eq!(
        identity(&message),
        BINARY,
        "a 256-byte binary message, including NUL and high bytes, digests to its published vector"
    );
}

#[test]
fn every_identity_is_64_lowercase_hex_characters() {
    let binary = [0u8, 1, 2, 0xff];
    let messages: [&[u8]; 4] = [
        b"",
        b"abc",
        b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
        &binary,
    ];
    for message in messages {
        let rendered = identity(message);
        assert!(
            is_lowercase_hex64(&rendered),
            "an identity is 64 characters of [0-9a-f], got {:?} (length {})",
            rendered,
            rendered.len()
        );
    }
}

#[test]
fn equal_bytes_give_equal_identities() {
    let first = b"content that occurs at two paths".to_vec();
    let second = first.clone();
    assert_eq!(identity(&first), identity(&second), "equal bytes, equal identity");
    assert_eq!(identity(b"abc"), identity(b"abc"), "identity is deterministic");
}

#[test]
fn one_changed_byte_gives_a_different_identity() {
    let first: Vec<u8> = (0..=255u8).collect();
    let second = {
        let mut changed = first.clone();
        changed[127] ^= 0x01;
        changed
    };
    assert_ne!(identity(&first), identity(&second), "one changed byte changes the identity");
    assert_ne!(identity(b"abc"), identity(b"abd"), "the same holds for a short message");
}

#[test]
fn a_trailing_newline_is_hashed_not_normalised_away() {
    assert_ne!(
        identity(b"abc\n"),
        identity(b"abc"),
        "a trailing newline is part of the content and changes the identity"
    );
    assert_eq!(
        identity(b"abc\n"),
        ABC_NEWLINE,
        "abc\\n digests to its own vector, not to the vector of abc"
    );
}

#[test]
fn a_trailing_space_is_hashed_not_trimmed() {
    assert_ne!(
        identity(b"abc "),
        identity(b"abc"),
        "trailing whitespace is part of the content and changes the identity"
    );
}

#[test]
fn crlf_is_hashed_not_normalised_to_lf() {
    assert_ne!(
        identity(b"abc\r\n"),
        identity(b"abc\n"),
        "CRLF is not folded to LF"
    );
}

#[test]
fn a_leading_space_is_pinned_not_trimmed_away() {
    assert_eq!(
        identity(b" abc\n"),
        LEADING_SPACE_NEWLINE,
        "a leading space and a trailing newline both digest as content"
    );
    assert_ne!(
        identity(b" abc\n"),
        identity(b"abc\n"),
        "a leading space is part of the content and changes the identity"
    );
}

#[test]
fn a_leading_tab_is_pinned_not_trimmed_away() {
    assert_eq!(
        identity(b"\tabc"),
        LEADING_TAB_ABC,
        "a leading tab digests as content"
    );
    assert_ne!(
        identity(b"\tabc"),
        identity(b"abc"),
        "a leading tab is part of the content and changes the identity"
    );
}

#[test]
fn leading_spaces_are_hashed_not_trimmed() {
    assert_ne!(
        identity(b"  abc"),
        identity(b"abc"),
        "leading whitespace is part of the content and changes the identity"
    );
}

#[test]
fn the_base_has_one_entry_per_named_path_and_nothing_else() {
    let contents: [(&str, &[u8]); 2] = [
        ("nodes/aaaaaaaa.json", b"{\"kind\":\"decision\"}"),
        ("src/a.rs", b"fn a() {}"),
    ];
    let entries: BTreeMap<String, String> = base(contents);
    let keys: BTreeSet<&str> = entries.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = ["nodes/aaaaaaaa.json", "src/a.rs"].into_iter().collect();
    assert_eq!(keys, expected, "one entry per owned path and no other entry");
    assert_eq!(entries.len(), 2, "the base has no extra entries");
}

#[test]
fn every_base_entry_is_the_identity_of_that_paths_content() {
    let contents: [(&str, &[u8]); 3] = [
        ("nodes/aaaaaaaa.json", b"node document bytes"),
        ("src/a.rs", b"fn a() {}"),
        ("tests/b.rs", b"\x00\xff"),
    ];
    let entries = base(contents);
    for (path, bytes) in contents {
        assert_eq!(
            entries[path],
            identity(bytes),
            "{path} carries the identity of its own content"
        );
    }
}

#[test]
fn the_path_name_is_not_folded_in() {
    let contents: [(&str, &[u8]); 2] = [
        ("src/a.rs", b"identical bytes under two names"),
        ("src/renamed.rs", b"identical bytes under two names"),
    ];
    let entries = base(contents);
    assert_eq!(
        entries["src/a.rs"], entries["src/renamed.rs"],
        "the same content at two paths yields the same identity value"
    );
    assert_eq!(
        entries["src/a.rs"],
        identity(b"identical bytes under two names"),
        "the value is the digest of the content alone"
    );
}

#[test]
fn an_empty_path_has_the_empty_input_identity() {
    let contents: [(&str, &[u8]); 1] = [("notes/.keep", b"")];
    let entries = base(contents);
    assert_eq!(
        entries["notes/.keep"], EMPTY,
        "an empty file's identity is the empty-input vector"
    );
}

#[test]
fn an_owned_set_with_no_paths_has_no_entries() {
    let entries = base(Vec::<(&str, &[u8])>::new());
    assert!(entries.is_empty(), "an empty owned set yields an empty base");
}
