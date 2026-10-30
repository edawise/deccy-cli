//! Gate for the work schema (the closed draft-record document).
//!
//! Self-contained per M-tests: this crate embeds the schema the binary carries and checks it
//! both ways — a conforming record validates in either state, and an unknown field at either
//! level, a third state, a missing field, an empty string and a base that is not a path-keyed
//! map are each rejected. The base's value space is deliberately unfixed by the record's form,
//! so an entry of any JSON shape is left alone.

use serde_json::Value;

/// The schema the binary carries, embedded with `include_str!`.
const WORK_SCHEMA: &str = include_str!("../../../schemas/work.json");

fn compiled() -> jsonschema::JSONSchema {
    let document: Value = serde_json::from_str(WORK_SCHEMA).expect("the schema is JSON");
    jsonschema::JSONSchema::compile(&document).expect("the schema compiles")
}

fn assert_accepted(record: &Value, why: &str) {
    assert!(
        compiled().validate(record).is_ok(),
        "the schema must accept it: {why}"
    );
}

fn assert_rejected(record: &Value, why: &str) {
    assert!(
        compiled().validate(record).is_err(),
        "the schema must reject it: {why}"
    );
}

/// One record of the shape the schema fixes: the handle, the state, the three references and
/// the content-identity base, keyed by repo-relative path.
fn valid() -> Value {
    serde_json::json!({
        "handle": "k4mz7qta",
        "state": "open",
        "node": "rxpkkknl",
        "edit": "uqwnzmtb",
        "workspace": {
            "name": "draft-u4b",
            "path": "drafts/r20261030-132535/c3/gate-U4b",
        },
        "base": {
            "nodes/rxpkkknl.json": "9f2c41d0",
            "schemas/work.json": null,
        },
    })
}

#[test]
fn a_conforming_record_validates() {
    assert_accepted(&valid(), "a conforming open record");

    let mut landed = valid();
    landed["state"] = serde_json::json!("landed");
    assert_accepted(&landed, "a conforming landed record");
}

#[test]
fn a_missing_field_is_rejected() {
    for key in ["handle", "state", "node", "edit", "workspace", "base"] {
        let mut record = valid();
        record.as_object_mut().unwrap().remove(key);
        assert_rejected(&record, &format!("`{key}` is required"));
    }
}

#[test]
fn an_unknown_top_level_field_is_rejected() {
    for key in ["extra", "version"] {
        let mut record = valid();
        record[key] = serde_json::json!(1);
        assert_rejected(&record, &format!("`{key}` is not a field of a record"));
    }
}

#[test]
fn an_unknown_workspace_field_is_rejected() {
    let mut record = valid();
    record["workspace"]["owner"] = serde_json::json!("someone");
    assert_rejected(&record, "the workspace reference is closed too");
}

#[test]
fn a_third_state_is_rejected() {
    for state in ["returned", "closed", "Open"] {
        let mut record = valid();
        record["state"] = serde_json::json!(state);
        assert_rejected(&record, &format!("`{state}` is not one of the two states"));
    }
}

#[test]
fn an_empty_string_is_rejected() {
    for key in ["handle", "node", "edit", "workspace.name", "workspace.path"] {
        let mut record = valid();
        match key.split_once('.') {
            Some((outer, inner)) => record[outer][inner] = serde_json::json!(""),
            None => record[key] = serde_json::json!(""),
        }
        assert_rejected(&record, &format!("`{key}` names something, so it is non-empty"));
    }
}

#[test]
fn a_reference_that_is_a_list_is_rejected() {
    for key in ["node", "edit"] {
        let mut record = valid();
        record[key] = serde_json::json!(["rxpkkknl", "uqwnzmtb"]);
        assert_rejected(&record, &format!("`{key}` names exactly one, not a list"));
    }
}

#[test]
fn a_base_that_is_not_a_path_keyed_map_is_rejected() {
    for base in [
        serde_json::json!([]),
        serde_json::json!(["nodes/rxpkkknl.json"]),
        serde_json::json!("9f2c41d0"),
        Value::Null,
    ] {
        let mut record = valid();
        record["base"] = base.clone();
        assert_rejected(&record, &format!("`base` is a map, not {base}"));
    }

    let mut empty_key = valid();
    empty_key["base"] = serde_json::json!({"": "9f2c41d0"});
    assert_rejected(&empty_key, "a base key is a path, so it is non-empty");
}

#[test]
fn a_base_value_is_unconstrained() {
    let mut record = valid();
    record["base"] = serde_json::json!({
        "nodes/rxpkkknl.json": "9f2c41d0",
        "schemas/work.json": null,
        "tests/work/src/lib.rs": 12,
        "Cargo.toml": {"sha256": "abc", "length": 8},
        "README.md": true,
    });
    assert_accepted(&record, "the encoding of a content identity is not the schema's rule");
}
