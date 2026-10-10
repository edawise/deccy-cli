//! The log verb: list every node as `<id> <kind> <summary>`, in dependency order, changing
//! nothing.
//!
//! D16b (zqkqvlyo) fixes the read — the whole set of nodes with their edges, no filtering —
//! and its EARS state the read (txookpuq), the no-change restraint (mrulpsoq), and the
//! dependency-order rule (ztpwplsn); C (mztqtrsz) refuses an argument
//! and L (wnuqqlwr) refuses a node that did not load; O9 (mlszsqrr) fixes the
//! one-line-per-node rendering. The set comes through M-node-set, each node's document and
//! kind with it (read through M-store-read across the runner, M-jj), and each summary from the
//! change descriptions `jj log` supplies (M-jj-log). The order is computed from the edges the
//! set already carries (ztpwplsn): each node follows everything it depends on or evaluates;
//! when several are ready, the smallest change id goes next; a cycle cannot be ordered, so its
//! leftovers follow in ascending change id. The verb reads through the runner (M-jj)
//! and returns its lines
//! for the envelope to write (M-output-return) under M-route's handler shape, carrying its
//! own --help text (xlvoxmyl/rwlywqrk, per M-help's division of labour) and the injected
//! context (M-context-injection).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::context::Context;
use crate::jjlog::{self, LogError};
use crate::refusal::Refusal;
use crate::node_set::{self, NodeRead};
use crate::node_load::LoadError;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str = "deccy log: list every node in dependency order as `<id> <kind> <summary>`.";

/// The log handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    log(context, args)
}

/// Read the whole node set, order it by its edges, and render one `<id> <kind> <summary>` line
/// per node (D16b, O9, ztpwplsn); an invocation carrying an argument refuses, naming it (C),
/// and a node that did not load refuses by name (L).
fn log(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(lines_of(HELP));
    }
    if let Some(argument) = args.first() {
        return Err(Refusal::new("unexpected argument").with_evidence(argument.clone()));
    }
    let nodes = node_set::list(context.jj()).map_err(list_failed)?;
    for (id, read) in &nodes {
        if let NodeRead::Failed(error) = read {
            return Err(Refusal::new(reason(error)).with_evidence(id.clone()));
        }
    }
    let descriptions = jjlog::log(context.jj()).map_err(log_failed)?;
    let mut lines = Vec::new();
    for id in ordered(&nodes) {
        let read = nodes
            .get(&id)
            .expect("an ordered id is one of the listed nodes");
        let NodeRead::Loaded { document, .. } = read else {
            unreachable!("a node that did not load refused above")
        };
        let kind = document.get("kind").and_then(Value::as_str).unwrap_or("");
        let summary = descriptions
            .iter()
            .find(|(change, _)| change.as_str() == id.as_str())
            .map(|(_, description)| description.as_str())
            .unwrap_or("");
        lines.push(format!("{} {} {}", id, kind, summary));
    }
    Ok(lines)
}

/// The nodes in dependency order (ztpwplsn): each node after everything it depends on or
/// evaluates, the smallest change id first among the ready nodes, and any node a cycle keeps
/// from being listed last in ascending change id. Edge targets outside the set (`zzzzzzzz`,
/// dangling ids) impose no order — the listing does not validate edges.
fn ordered(nodes: &BTreeMap<String, NodeRead>) -> Vec<String> {
    let mut listed: BTreeSet<String> = BTreeSet::new();
    let mut order = Vec::with_capacity(nodes.len());
    while order.len() < nodes.len() {
        let next = nodes
            .iter()
            .find(|(id, read)| {
                !listed.contains(*id)
                    && match read {
                        NodeRead::Loaded { edges, .. } => edges
                            .depends_on
                            .iter()
                            .chain(&edges.evaluates)
                            .filter(|target| nodes.contains_key(*target))
                            .all(|target| listed.contains(target)),
                        NodeRead::Failed(_) => false,
                    }
            })
            .map(|(id, _)| id.clone());
        match next {
            Some(id) => {
                listed.insert(id.clone());
                order.push(id);
            }
            None => {
                order.extend(nodes.keys().filter(|id| !listed.contains(*id)).cloned());
                break;
            }
        }
    }
    order
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

/// S2-invalid's failure, read as its reason — the reason L names alongside the node.
fn reason(error: &LoadError) -> String {
    match error {
        LoadError::Missing => "missing node document".to_string(),
        LoadError::Unreadable(reason)
        | LoadError::NotJson(reason)
        | LoadError::Invalid(reason)
        | LoadError::Schema(reason) => reason.clone(),
    }
}

/// The directory-level read failure: the store could not be listed at all (S2).
fn list_failed(error: LoadError) -> Refusal {
    Refusal::new(reason(&error))
}

/// S3-err's failure, carried as a refusal: jj's captured output is the cause.
fn log_failed(error: LogError) -> Refusal {
    match error {
        LogError::Spawn(reason) => Refusal::new(reason),
        LogError::Failed(out) => Refusal::new(out.stderr.trim().to_string()),
    }
}
