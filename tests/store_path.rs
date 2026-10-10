//! Gate for M-store-path (id parsing and node-document derivation).
//!
//! Self-contained per M-tests: the shim declares the module under test alone — after the split
//! the derivation reads through nothing. No shared test-support module.

#[path = "../src/store_path.rs"]
mod store_path;

use std::path::{Component, PathBuf};
use store_path::{node_path, Id};

#[test]
fn a_valid_id_derives_its_document_path() {
    let id = Id::parse("ppswywtx").expect("a change id parses");
    assert_eq!(node_path(&id), PathBuf::from("nodes/ppswywtx.json"));
}

#[test]
fn every_valid_id_maps_to_its_own_document() {
    for raw in ["uwtupvpw", "zzzzzzzz", "a1b2c3d4", "abcdefghijklmnop"] {
        let id = Id::parse(raw).expect("valid id");
        assert_eq!(node_path(&id), PathBuf::from(format!("nodes/{raw}.json")));
    }
}

#[test]
fn an_unparseable_id_never_yields_a_path() {
    for bad in ["", "short", "ABCDEFGH", "has space", "no/pe", "../etc", "éééééééé", "1111111", "x.json"] {
        assert!(Id::parse(bad).is_none(), "{bad:?} must not parse");
    }
}

#[test]
fn a_derived_path_is_always_nodes_slash_one_file() {
    let id = Id::parse("abcdefgh").unwrap();
    let path = node_path(&id);
    let parts: Vec<_> = path.components().collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], Component::Normal("nodes".as_ref()));
    assert_eq!(parts[1], Component::Normal("abcdefgh.json".as_ref()));
}
