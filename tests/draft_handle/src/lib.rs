//! Gate for the draft's record handle (the mint's form and its freshness).
//!
//! Self-contained per M-tests: this crate embeds the flat module `src/draft_handle.rs` through a
//! `#[path]` shim and checks the mint's observable contract against the handle rule — exactly
//! sixteen characters over `a-z0-9`, a function of the bytes it is given by the stated mapping,
//! never a counter or stored state, and redrawn while the handle names a record already held
//! (a `<handle>.json` file under `.deccy/work/`).
//!
//! The module is pulled in as a leaf — `std` and nothing of the CLI's own — which is what lets a
//! fixture crate compile it on its own.

#[path = "../../../src/draft_handle.rs"]
mod draft_handle;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// The alphabet the mapping fixes, in order: `0`–`9` then `a`–`z`.
const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// The handle `from_bytes` must return: character `i` is `ALPHABET[bytes[i] % 36]`.
fn mapped(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(16)
        .map(|&byte| ALPHABET[(byte % 36) as usize] as char)
        .collect()
}

fn assert_handle(handle: &str, why: &str) {
    assert_eq!(
        handle.chars().count(),
        16,
        "a handle is exactly sixteen characters, got {handle:?}: {why}"
    );
    for c in handle.chars() {
        assert!(
            c.is_ascii_lowercase() || c.is_ascii_digit(),
            "`{c}` in {handle:?} is outside the alphabet `a-z0-9`: {why}"
        );
    }
}

/// A process-unique fixture root holding the record directory `.deccy/work/`.
fn work_dir(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "deccy-draft-handle-gate-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let work = root.join(".deccy").join("work");
    std::fs::create_dir_all(&work).expect("the fixture record directory is created");
    work
}

/// `taken` in the record's sense: the handle names a `*.json` file under `.deccy/work/`.
fn held_by_a_record(work: &Path, handle: &str) -> bool {
    work.join(format!("{handle}.json")).is_file()
}

fn sample_free() -> String {
    "k4mz7qta9vx2bndt".to_string()
}

/// The handle as a base-36 number, so two handles can be compared as successive values.
fn numeric(handle: &str) -> u128 {
    handle.bytes().fold(0u128, |acc, byte| {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'z' => byte - b'a' + 10,
            _ => panic!("`{handle}` holds a character outside the alphabet"),
        };
        acc * 36 + u128::from(digit)
    })
}

#[test]
fn a_handle_is_sixteen_lowercase_alphanumerics() {
    for bytes in [[0u8; 16], [7u8; 16], [255u8; 16]] {
        assert_handle(
            &draft_handle::from_bytes(&bytes),
            "every handle the mint constructs",
        );
    }
}

#[test]
fn the_stated_mapping_is_pinned() {
    assert_eq!(
        draft_handle::from_bytes(&[0u8; 16]),
        "0000000000000000",
        "index 0 of the alphabet for every byte"
    );

    let counting: Vec<u8> = (0..16).collect();
    assert_eq!(
        draft_handle::from_bytes(&counting),
        "0123456789abcdef",
        "bytes 0..16 are alphabet indices 0..16"
    );

    assert_eq!(
        draft_handle::from_bytes(&[255u8; 16]),
        "3333333333333333",
        "255 % 36 = 3"
    );
}

#[test]
fn different_bytes_give_different_handles() {
    let zero = draft_handle::from_bytes(&[0u8; 16]);
    let max = draft_handle::from_bytes(&[255u8; 16]);
    assert_ne!(zero, max, "the handle is a function of the bytes, not a constant");
}

#[test]
fn the_same_bytes_give_the_same_handle_twice() {
    let bytes = [9u8; 16];
    let first = draft_handle::from_bytes(&bytes);
    let second = draft_handle::from_bytes(&bytes);
    assert_eq!(
        first, second,
        "the mint keeps no counter or other state between calls"
    );
    assert_eq!(first, mapped(&bytes), "and it is the stated mapping, not a sequence");
}

#[test]
fn a_free_draw_is_returned_as_drawn() {
    let got = draft_handle::first_free(sample_free, |_| false);
    assert_eq!(got, sample_free(), "a free candidate is kept, not replaced or incremented");
    assert_handle(&got, "the kept draw");
}

#[test]
fn the_draw_loop_keeps_no_state_between_calls() {
    let first = draft_handle::first_free(sample_free, |_| false);
    let second = draft_handle::first_free(sample_free, |_| false);
    assert_eq!(
        first, second,
        "the same draw is returned again: the loop holds nothing across calls"
    );
}

#[test]
fn a_handle_a_record_holds_is_redrawn() {
    let work = work_dir("redraw");
    let held = "aaaaaaaaaaaaaaaa";
    std::fs::write(work.join(format!("{held}.json")), "{}").expect("the held record is written");

    let draws = [held, "bbbbbbbbbbbbbbbb"];
    let mut next = draws.iter();
    let sample = move || {
        next.next()
            .expect("two draws are enough: the first is held, the second is free")
            .to_string()
    };
    let taken = |handle: &str| held_by_a_record(&work, handle);

    let got = draft_handle::first_free(sample, taken);

    assert_eq!(
        got, "bbbbbbbbbbbbbbbb",
        "a candidate a record holds is rejected and the next free draw is kept"
    );
    assert!(
        held_by_a_record(&work, held),
        "the record already stored keeps its handle"
    );
}

#[test]
fn the_mint_consults_taken_before_returning() {
    let log = RefCell::new(Vec::<String>::new());
    let taken = |candidate: &str| {
        let mut log = log.borrow_mut();
        log.push(candidate.to_string());
        log.len() == 1
    };

    let handle = draft_handle::mint(taken);

    let seen = log.into_inner();
    assert_handle(&handle, "a handle minted while `taken` rejects the first draw");
    assert!(
        !seen.is_empty(),
        "the mint asks `taken` about the candidates it draws"
    );
    assert_ne!(
        handle, seen[0],
        "the mint does not return a candidate `taken` rejected"
    );
}

#[test]
fn the_mint_redraws_while_candidates_are_taken() {
    let calls = RefCell::new(0usize);
    let rejected = RefCell::new(Vec::<String>::new());
    let taken = |candidate: &str| {
        let mut calls = calls.borrow_mut();
        *calls += 1;
        if *calls <= 3 {
            rejected.borrow_mut().push(candidate.to_string());
            true
        } else {
            false
        }
    };

    let handle = draft_handle::mint(taken);

    let calls = calls.into_inner();
    let rejected = rejected.into_inner();
    assert_handle(&handle, "a handle minted after three rejected draws");
    assert!(
        calls >= 4,
        "the mint consults `taken` until a candidate is free: {calls} consultations"
    );
    assert!(
        !rejected.is_empty(),
        "the mint asks `taken` about the candidates it draws"
    );
    for candidate in &rejected {
        assert_ne!(
            handle, *candidate,
            "the mint does not return a candidate `taken` rejected"
        );
    }
}

#[test]
fn the_mint_returns_the_first_candidate_taken_accepts() {
    let log = RefCell::new(Vec::<String>::new());
    let taken = |candidate: &str| {
        let mut log = log.borrow_mut();
        log.push(candidate.to_string());
        log.len() == 1
    };

    let handle = draft_handle::mint(taken);

    let seen = log.into_inner();
    assert_handle(&handle, "a handle minted after `taken` rejects the first draw");
    assert!(
        seen.len() >= 2,
        "the mint redraws after `taken` rejects the first candidate: saw {seen:?}"
    );
    assert_eq!(
        handle, seen[1],
        "the handle is the first candidate `taken` accepted, not a later draw"
    );
}

#[test]
fn the_mint_returns_a_free_handle_from_the_system() {
    let work = work_dir("mint");
    let handle = draft_handle::mint(|candidate| held_by_a_record(&work, candidate));
    assert_handle(&handle, "a handle minted from OS entropy");
    assert!(
        !held_by_a_record(&work, &handle),
        "no record holds the handle the mint returns"
    );
}

#[test]
fn two_mints_are_fresh_draws() {
    let first = draft_handle::mint(|_| false);
    let second = draft_handle::mint(|_| false);
    assert_handle(&first, "a handle minted from OS entropy");
    assert_handle(&second, "a second handle minted from OS entropy");
    assert_ne!(
        first, second,
        "each mint draws again instead of returning one stored handle"
    );

    let space = 36u128.pow(16);
    let step = (numeric(&second) + space - numeric(&first)) % space;
    assert_ne!(
        step, 1,
        "the second handle is not the first one incremented: {first} then {second}"
    );
}
