//! The node set: every node's id, its document and edges, or why it failed to load.
//!
//! S2 makes one store under one repo root the boundary every verb and guard crosses; S2-path
//! fixes that a node is `nodes/<id>.json`, so the set of nodes is the set of changes that own
//! such a document; S2-set (ryzlzrkk) fixes that the set is the ids jj reports, never a
//! directory listing; S2-invalid fixes the failure — invalid with the reason attached; D3
//! fixes the edges to report, D4 that targets are sibling ids with the root excepted. Reading
//! is total: every node and every load failure are surfaced as values, so the caller — log's
//! all-or-nothing read, validate's per-node report — decides what a failure means.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::jj::{Jj, JjError};
use crate::node_load::{load, LoadError};
use crate::store_path::Id;

/// The change listing, scoped to the changes that touch a node document (S2-set): the changes
/// touching `nodes/` are exactly the changes that own one (S2-path).
const NODE_SET: &str = "files(\"nodes/\")";

/// What one listed change reports: its change id, which names the node it owns (D1b).
const ID: &str = "change_id.shortest(8) ++ \"\\n\"";

/// A node's outgoing edges, as its document records them (D3).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Edges {
    /// Prerequisite ids.
    pub depends_on: Vec<String>,
    /// Evaluated ids.
    pub evaluates: Vec<String>,
}

/// One node's read: its document with its edges, or the reason it did not load (S2-invalid).
#[derive(Debug)]
pub enum NodeRead {
    /// The node loaded: its checked document and its outgoing edges.
    Loaded { document: Value, edges: Edges },
    /// The document did not load; the reason is carried (S2-invalid).
    Failed(LoadError),
}

/// Every node in the set, keyed by id, with its document and edges — or its load failure
/// (S2, S2-path, S2-set, S2-invalid, D3, D4).
///
/// The set is the changes jj reports for `nodes/`; a change that owns no node document is not
/// one. A per-document load failure is data, never an abort; only the listing itself failing
/// is an `Err`.
pub fn list(jj: &Jj) -> Result<BTreeMap<String, NodeRead>, LoadError> {
    let mut nodes = BTreeMap::new();
    for id in ids(jj)? {
        let read = match load(jj, &id) {
            Ok(document) => NodeRead::Loaded {
                edges: edges(&document),
                document,
            },
            Err(error) => NodeRead::Failed(error),
        };
        nodes.insert(id.as_str().to_string(), read);
    }
    Ok(nodes)
}

/// The node ids jj reports: one change id per change touching `nodes/` (S2-set, D1b).
fn ids(jj: &Jj) -> Result<Vec<Id>, LoadError> {
    let output = jj
        .run(&["log", "--no-graph", "-r", NODE_SET, "-T", ID])
        .map_err(listing_failed)?;
    if output.status != 0 {
        return Err(LoadError::Unreadable(output.stderr.trim().to_string()));
    }
    Ok(output
        .stdout
        .lines()
        .filter_map(|line| Id::parse(line.trim()))
        .collect())
}

/// A listing that could not spawn: the reason is carried (S3-err).
fn listing_failed(error: JjError) -> LoadError {
    match error {
        JjError::Spawn(reason) => LoadError::Unreadable(reason),
    }
}

/// The edges a document records: `depends_on` and `evaluates` (D3).
fn edges(node: &Value) -> Edges {
    Edges {
        depends_on: id_list(node.pointer("/depends_on")),
        evaluates: id_list(node.pointer("/evaluates")),
    }
}

fn id_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
