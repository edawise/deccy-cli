//! Ownership attribution: the files a change owns, derived, not scanned.
//!
//! S2-own fixes that a change's files are attributed through the ownership rule — its node
//! file plus its ref-listed paths, nothing else, never by scanning; S2-own-inline fixes the
//! degenerate case (inline ref yields the node file alone). D2 states the rule. The node
//! file's path comes from store_path, so there is one layout; the document arrives already
//! loaded.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;

use crate::store_path::{node_path, Id};

/// The files `id`'s change owns, given its loaded document: its node file plus its
/// ref-listed paths, nothing else (S2-own, S2-own-inline, D2).
pub fn owned(id: &Id, node: &Value) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    files.insert(node_path(id));
    if let Some(paths) = node.pointer("/content/ref").and_then(Value::as_array) {
        for path in paths.iter().filter_map(Value::as_str) {
            files.insert(PathBuf::from(path));
        }
    }
    files
}
