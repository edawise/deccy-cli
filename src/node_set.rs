//! The node set: every node's id, its document and edges, or why it failed to load.
//!
//! S2 makes one store under one repo root the boundary every verb and guard crosses; S2-path
//! fixes that a node is `nodes/<id>.json`, so a node is a change that carries its own
//! document; S2-set (ryzlzrkk) fixes that the set is the ids jj reports, narrowed to the
//! changes whose own document is there — the changes touching `nodes/` are only the
//! candidates — never a directory listing; S2-invalid fixes the failure — invalid with the
//! reason attached; D3 fixes the edges to report, D4 that targets are sibling ids with the
//! root excepted. Reading is total: every node and every load failure are surfaced as values,
//! so the caller — log's all-or-nothing read, validate's per-node report — decides what a
//! failure means. The documents are read through M-store-bulk (xlvtxsky): the enumeration
//! supplies the ids, the set read maps and batches them, and this module maps each outcome.
//! The set is reported in the order jj listed the candidates (`list_ordered`), the same order
//! the enumeration is read in; `list` is the same set keyed by id for the callers that need to
//! look a node up rather than walk the listing. The drop of a candidate for want of a document
//! is itself a fact worth reporting, so the read also yields the dropped ids
//! (`list_with_drops`): the same set as `list_ordered`, plus the ids of exactly the candidates
//! that owned no `nodes/<its own id>.json`, so nothing the enumeration saw disappears silently.
//! There is one read — `read` — and all three entry points are views of it, so the enumeration,
//! the batching and the mapping exist once.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::jj::{Jj, JjError};
use crate::node_load::LoadError;
use crate::store_bulk;
use crate::store_path::Id;

/// The candidate change listing (S2-set): the changes that touch a path under `nodes/`. This
/// is a superset of the nodes — a change that only edits a sibling's document, or adds a
/// stray file there, touches the path without carrying its own (S2-path); `list` keeps a
/// candidate only when its own document is present.
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
/// This is `list_ordered` keyed by id, for a caller that looks nodes up by id (`log`'s
/// all-or-nothing read, `validate`'s per-node report); its iteration order is the ids' own, not
/// the listing's. A caller that reports the set as listed reads `list_ordered` directly.
pub fn list(jj: &Jj, git: Option<&Path>) -> Result<BTreeMap<String, NodeRead>, LoadError> {
    Ok(read(jj, git)?.0.into_iter().collect())
}

/// Every node in the set, in the order jj listed its change, with its document and edges — or
/// its load failure (S2, S2-path, S2-set, S2-invalid, D3, D4).
///
/// The candidates are the changes jj reports for `nodes/`, in the order jj reports them; a
/// candidate whose own document is absent owns no node and is dropped (S2-set), while any other
/// load failure is data, never an abort; only the listing itself failing is an `Err`. The order
/// is the listing's — the ids are read in that order and reported in it — so a caller that walks
/// the set walks what jj said, and `list`'s id keying never reorders this.
pub fn list_ordered(jj: &Jj, git: Option<&Path>) -> Result<Vec<(String, NodeRead)>, LoadError> {
    Ok(read(jj, git)?.0)
}

/// The set read with its drops: every node as `list_ordered` reports it — entry for entry and in
/// the order jj listed them — together with the ids of the candidates dropped for want of their
/// own document (S2-set).
///
/// The first element is exactly what `list_ordered` returns; the second is exactly the
/// candidate ids that own no `nodes/<their own id>.json`, and nothing else — a candidate whose
/// document is present but does not load stays a node carrying its failure (S2-invalid), never a
/// drop. The drops are reported as a set: their order carries no meaning. Nothing is added to
/// the set by reporting a drop, so the set and the drops are disjoint and together account for
/// every candidate the enumeration listed, once each.
pub fn list_with_drops(
    jj: &Jj,
    git: Option<&Path>,
) -> Result<(Vec<(String, NodeRead)>, Vec<String>), LoadError> {
    read(jj, git)
}

/// The one read every entry point is a view of: the enumerated ids in jj's order, each mapped to
/// its document and edges or its failure, plus the ids dropped for want of a document.
fn read(
    jj: &Jj,
    git: Option<&Path>,
) -> Result<(Vec<(String, NodeRead)>, Vec<String>), LoadError> {
    let ids = ids(jj)?;
    let git = git.map(|binary| Jj::new(binary, jj.root()));
    // The set read is keyed by id and total: every id given has an outcome.
    let mut outcomes = store_bulk::read(jj, git.as_ref(), &ids);
    let mut nodes = Vec::with_capacity(ids.len());
    let mut dropped = Vec::new();
    for id in &ids {
        let Some(outcome) = outcomes.remove(id.as_str()) else {
            continue;
        };
        match outcome {
            Ok(document) => nodes.push((
                id.as_str().to_string(),
                NodeRead::Loaded {
                    edges: edges(&document),
                    document,
                },
            )),
            // No `nodes/<its own id>.json` at the change: it only touches the path (a buffer
            // editing a sibling's document, a stray file), so it is not a node (S2-set). The
            // listing keeps its place, so the rest of the ids stay in jj's order; the id is
            // remembered so the drop can be reported rather than vanish.
            Err(LoadError::Missing) => dropped.push(id.as_str().to_string()),
            Err(error) => nodes.push((id.as_str().to_string(), NodeRead::Failed(error))),
        }
    }
    Ok((nodes, dropped))
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
