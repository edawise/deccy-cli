//! The validate verb: exercise validity, resolution, and coverage; report failures.
//!
//! D16c (xtuswonn) fixes the read — exercise the three checks over the graph, report
//! failures, change nothing, record no verdicts; its EARS state the exercising (qprkpnso),
//! the no-change and verdict-freedom (sspsomkw), and the reporting (pyvmoluu). D9 (orwrynyn)
//! fixes the checks — node validity against the schema, edge target resolution, and sandwich
//! coverage of every decision edge (kwwsoosw) — and O10 (rzlyozlp) the one-line-per-failure
//! rendering; C (mztqtrsz) refuses an argument. The set comes through M-node-set, each node's
//! document and edges with it (read through M-store-read across the runner, M-jj), and every
//! target is resolved as
//! the graph resolves it (M-node-resolve). The verb changes nothing and returns its lines
//! for the envelope to write (M-output-return) under M-route's handler shape, carrying its
//! own --help text (xlvoxmyl/rwlywqrk) and the injected context (M-context-injection).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::context::Context;
use crate::jj::Jj;
use crate::refusal::Refusal;
use crate::node_set::{self, Edges, NodeRead};
use crate::node_load::LoadError;
use crate::store_path::Id;
use crate::node_resolve::resolve;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy validate: report validity, resolution, and coverage failures as `<check> <id> <reason>` lines.";

/// The validate handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    validate(context, args)
}

/// Exercise the three checks over the graph and report each failure as one
/// `<check> <id> <reason>` line (D16c, D9, O10); an argument refuses, naming it (C).
fn validate(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(lines_of(HELP));
    }
    if let Some(argument) = args.first() {
        return Err(Refusal::new("unexpected argument").with_evidence(argument.clone()));
    }
    let jj = context.jj();
    let nodes = node_set::list(jj).map_err(list_failed)?;
    let mut lines = Vec::new();

    // Validity: each node's load outcome (S2-validate, S2-invalid).
    for (id, read) in &nodes {
        if let NodeRead::Failed(error) = read {
            lines.push(line("validity", id, &reason(error)));
        }
    }

    // Resolution: each loaded node's edge targets, resolved as the graph resolves them (D9).
    for (id, read) in &nodes {
        if let NodeRead::Loaded { edges, .. } = read {
            for target in edges.depends_on.iter().chain(&edges.evaluates) {
                if let Err(reason) = resolution(jj, target) {
                    lines.push(line("resolution", id, &reason));
                }
            }
        }
    }

    // Coverage: every decision's depends_on edge to another decision (D9, kwwsoosw).
    let kinds = kinds(&nodes);
    let evaluations = evaluations(&nodes);
    for (id, read) in &nodes {
        let NodeRead::Loaded { edges, .. } = read else {
            continue;
        };
        if !is_decision(&kinds, id) {
            continue;
        }
        for target in &edges.depends_on {
            if !is_decision(&kinds, target) {
                continue; // the root and non-decisions are out of scope
            }
            if !covers(&evaluations, id, target) {
                lines.push(line("coverage", id, target));
            }
        }
    }

    Ok(lines)
}

fn line(check: &str, id: &str, reason: &str) -> String {
    format!("{check} {id} {reason}")
}

/// Every loaded node's kind, by id (a node that did not load has none).
fn kinds(nodes: &BTreeMap<String, NodeRead>) -> BTreeMap<String, String> {
    nodes
        .iter()
        .filter_map(|(id, read)| match read {
            NodeRead::Loaded { document, .. } => Some((
                id.clone(),
                document
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            )),
            NodeRead::Failed(_) => None,
        })
        .collect()
}

fn is_decision(kinds: &BTreeMap<String, String>, id: &str) -> bool {
    kinds.get(id).map(String::as_str) == Some("decision")
}

/// Every loaded evaluation's edges, keyed by nothing: `evaluates` and `depends_on` (D9).
fn evaluations(nodes: &BTreeMap<String, NodeRead>) -> Vec<(&str, &Edges)> {
    nodes
        .iter()
        .filter_map(|(id, read)| match read {
            NodeRead::Loaded { document, edges }
                if document.get("kind").and_then(Value::as_str) == Some("evaluation") =>
            {
                Some((id.as_str(), edges))
            }
            _ => None,
        })
        .collect()
}

/// Whether some evaluation covers the decision edge `dependent -> prerequisite` (kwwsoosw).
fn covers(evaluations: &[(&str, &Edges)], dependent: &str, prerequisite: &str) -> bool {
    evaluations.iter().any(|(_, edges)| {
        edges.evaluates.iter().any(|e| e == dependent)
            && edges.depends_on.iter().any(|d| d == prerequisite)
    })
}

/// Resolve one edge target the way the graph resolves it: an id that names the root or a
/// sibling document, else the reason it does not (M-node-resolve).
fn resolution(jj: &Jj, target: &str) -> Result<(), String> {
    let Some(id) = Id::parse(target) else {
        return Err(format!("{target} is not a node id"));
    };
    match resolve(jj, &id) {
        Ok(_) => Ok(()),
        Err(error) => Err(format!("{target} does not resolve: {}", reason(&error))),
    }
}

/// S2-invalid's failure, read as its reason.
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

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}
