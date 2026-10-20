//! Gate for S5-journal-schema (the closed journal entry document).
//!
//! Self-contained per M-tests: this crate embeds the schema the binary carries and checks it
//! both ways — a valid entry passes, and an unknown field, a malformed hash, a bad version and
//! a missing side are each rejected. It grows the store module's tests when M-journal lands.

#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/journal.rs"]
mod journal;

use serde_json::Value;

/// The schema the binary carries, embedded with `include_str!` (S5-journal-schema).
const JOURNAL_SCHEMA: &str = include_str!("../../../schemas/journal.json");

fn compiled() -> jsonschema::JSONSchema {
    let document: Value = serde_json::from_str(JOURNAL_SCHEMA).expect("the schema is JSON");
    jsonschema::JSONSchema::compile(&document).expect("the schema compiles")
}

fn hex(c: char) -> String {
    std::iter::repeat(c).take(64).collect()
}

fn valid() -> Value {
    serde_json::json!({
        "id": hex('a'),
        "version": 1,
        "kind": "record",
        "target": "kqxyzmrn",
        "before": {"nodes/kqxyzmrn.json": hex('b'), "notes.txt": null},
        "after": {"nodes/kqxyzmrn.json": hex('c'), "notes.txt": hex('d')},
    })
}

#[test]
fn a_valid_entry_passes() {
    let schema = compiled();
    assert!(
        schema.validate(&valid()).is_ok(),
        "a valid journal entry validates"
    );
}

#[test]
fn an_unknown_field_is_rejected() {
    let mut entry = valid();
    entry["extra"] = serde_json::json!(true);
    assert!(
        compiled().validate(&entry).is_err(),
        "the entry object is closed"
    );
}

#[test]
fn a_bad_hash_is_rejected() {
    let mut entry = valid();
    entry["after"]["notes.txt"] = serde_json::json!("zz");
    assert!(
        compiled().validate(&entry).is_err(),
        "a content value is 64 hex or null"
    );
}

#[test]
fn a_bad_version_is_rejected() {
    let mut entry = valid();
    entry["version"] = serde_json::json!(2);
    assert!(
        compiled().validate(&entry).is_err(),
        "the entry version is fixed"
    );
}

#[test]
fn a_missing_side_is_rejected() {
    let mut entry = valid();
    entry.as_object_mut().unwrap().remove("before");
    assert!(
        compiled().validate(&entry).is_err(),
        "both sides are required"
    );
}

#[cfg(test)]
mod store_tests;
