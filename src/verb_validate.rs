//! The validate verb: exercise validity, resolution, coverage, topology, and custody; report
//! failures.
//!
//! D16c (xtuswonn) fixes the read — exercise the three checks over the graph, report
//! failures, change nothing, record no verdicts; its EARS state the exercising (qprkpnso),
//! the no-change and verdict-freedom (sspsomkw), and the reporting (pyvmoluu). D9 (orwrynyn)
//! fixes the checks — node validity against the schema, edge target resolution, and sandwich
//! coverage of every decision edge (kwwsoosw) — and O10 (rzlyozlp) the one-line-per-failure
//! rendering; C (mztqtrsz) refuses an argument. The set comes through M-node-set, each node's
//! document and edges with it (read through M-store-read across the runner, M-jj), and every
//! target is resolved from the set already read through M-node-resolve's set-backed entry
//! point (S2-resolve-in-set). The fourth check is S-topology's rule (zrkzppqo) applied to the
//! same reading: the order the set was listed in already gives each node its position, so the
//! check walks the loaded edges and costs no crossing of its own — never a per-node or per-edge
//! jj call, and never a second reading of the order. The verb changes nothing and returns its
//! lines for the envelope to write (M-output-return) under M-route's handler shape, carrying its
//! own --help text (xlvoxmyl/rwlywqrk) and the injected context (M-context-injection).
//!
//! The fifth check is content custody: what history recorded must stay inside the set each
//! change's own node owns (xotxspwp). The paths come from S-read-custody's one crossing
//! (kywmkqlm, `jj_custody::read`), and each change's owning set is S2-own's derivation
//! (lwnnssuo, `node_own::owned`) from the document the set already loaded — so the pass applies
//! the ownership rule as given rather than restating it, and prints one `custody <id> <path>`
//! line per path a change's diff strayed onto. Nothing is repaired: no edit is moved, no change
//! rewritten and no document amended, and no verdict is recorded. A read that could not itself
//! run is one check-level line — `custody - <jj evidence>`, the check's name, O10's stand-in in
//! the id position and the evidence it produced — rather than a refusal, so the other four
//! checks' findings still reach the caller.
//!
//! The pass accounts for every change that touches the node store: the set read reports the
//! candidates it dropped for want of a document of their own (S2-set, `list_with_drops`), so
//! validity names each of them with the same `<check> <id> <reason>` line a document that failed
//! to load gets, and custody attributes every candidate the set does hold. Nothing the
//! enumeration saw disappears silently, and no report of a drop puts the change back in the set:
//! a dropped candidate is not resolved, not placed in history and given no owning set.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::Value;

use crate::context::Context;
use crate::jj_custody::{self, CustodyError};
use crate::node_own::owned;
use crate::refusal::Refusal;
use crate::node_set::{self, Edges, NodeRead};
use crate::node_load::LoadError;
use crate::store_path::Id;
use crate::node_resolve::{resolve_in, Lookup};

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str = "deccy validate: report validity, resolution, coverage, topology, and \
     content custody failures as `<check> <id> <reason>` lines.";

/// The validate handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    validate(context, args)
}

/// Exercise the five checks over the graph and report each failure as one
/// `<check> <id> <reason>` line (D16c, D9, O10, xotxspwp); an argument refuses, naming it (C).
fn validate(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(lines_of(HELP));
    }
    if let Some(argument) = args.first() {
        return Err(Refusal::new("unexpected argument").with_evidence(argument.clone()));
    }
    let jj = context.jj();
    // One reading of the set, in the order jj listed it: the id keying the checks look nodes up
    // by and the order the topology check places them in come from that one listing. The same
    // read reports the candidates it dropped for want of a document of their own (S2-set), so
    // accounting for them costs no second crossing and adds nothing to the set.
    let (listed, dropped) =
        node_set::list_with_drops(jj, context.git().ok()).map_err(list_failed)?;
    let nodes: BTreeMap<&str, &NodeRead> = listed
        .iter()
        .map(|(id, read)| (id.as_str(), read))
        .collect();
    // One position per node: its index in the listing (S-topology).
    let positions: BTreeMap<&str, usize> = listed
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (id.as_str(), index))
        .collect();
    let mut lines = Vec::new();

    // Validity: each node's load outcome (S2-validate, S2-invalid), and — with the same line
    // shape — every candidate the set dropped because it carries no document of its own
    // (S2-set). A change that touches the node store and owns no document there is named, not
    // skipped: the read reports it, and this is the line it gets.
    for (id, read) in &nodes {
        if let NodeRead::Failed(error) = *read {
            lines.push(line("validity", id, &reason(error)));
        }
    }
    for id in &dropped {
        lines.push(line("validity", id, &reason(&LoadError::Missing)));
    }

    // Resolution: each loaded node's edge targets, resolved from the set already read
    // (D9, S2-resolve-in-set).
    for (id, read) in &nodes {
        if let NodeRead::Loaded { edges, .. } = *read {
            for target in edges.depends_on.iter().chain(&edges.evaluates) {
                if let Err(reason) = resolution(&nodes, target) {
                    lines.push(line("resolution", id, &reason));
                }
            }
        }
    }

    // Coverage: every decision's depends_on edge to another decision (D9, kwwsoosw).
    let kinds = kinds(&nodes);
    let evaluations = evaluations(&nodes);
    for (id, read) in &nodes {
        let NodeRead::Loaded { edges, .. } = *read else {
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

    // Topology: each edge whose target the listing does not place before the dependent
    // (S-topology, zrkzppqo), reported against the two positions already in hand. A target the
    // set cannot place has no position to compare, so it is silent here — resolution's line is
    // the only report it gets.
    for (id, read) in &listed {
        let NodeRead::Loaded { edges, .. } = read else {
            continue;
        };
        let Some(&here) = positions.get(id.as_str()) else {
            continue;
        };
        for target in edges.depends_on.iter().chain(&edges.evaluates) {
            let Some(&there) = positions.get(target.as_str()) else {
                continue;
            };
            if there < here {
                lines.push(line("topology", id, target));
            }
        }
    }

    // Custody: every path a change's recorded diff touched beyond the set its own node owns
    // (xotxspwp), read as one crossing (kywmkqlm) and compared with S2-own's derivation
    // (lwnnssuo). The findings are lines like the other four; nothing is repaired. A check that
    // could not itself run is O10's line for it — its name, the stand-in and the evidence jj
    // produced — so the other four checks' findings still reach the caller.
    match jj_custody::read(jj) {
        Ok(changes) => lines.extend(custody(&nodes, &changes)),
        Err(error) => lines.push(line("custody", STAND_IN, &custody_evidence(&error))),
    }

    Ok(lines)
}

/// The stand-in an O10 line carries in the id position when the check could not itself run: no
/// single node or change is implicated (rzlyozlp).
const STAND_IN: &str = "-";

fn line(check: &str, id: &str, reason: &str) -> String {
    format!("{check} {id} {reason}")
}

/// The fifth check: one `custody <id> <path>` line per path a change's recorded diff touched
/// that its own node does not own (xotxspwp), under O10's shape.
///
/// The paths are S-read-custody's pairing (kywmkqlm), never recomputed here; the owning set is
/// `node_own::owned`'s derivation from the document the set already loaded (S2-own, lwnnssuo) —
/// the node file plus the paths `content.ref` lists, nothing else. Ownership is compared, never
/// repaired: no document is amended and no change rewritten so the diff would fit. A change
/// whose node document did not load has no owning set to derive, so it is silent here: the
/// validity check already reports the node it could not read, and inventing a set for it would
/// be a second ownership rule.
fn custody(nodes: &BTreeMap<&str, &NodeRead>, changes: &[(String, Vec<String>)]) -> Vec<String> {
    // The loaded documents, with the id the set lists each under: the owning set comes from
    // these, and from nothing else.
    let mut documents: Vec<(&str, &Value)> = Vec::new();
    for (id, read) in nodes {
        if let NodeRead::Loaded { document, .. } = read {
            documents.push((id, document));
        }
    }
    let mut lines = Vec::new();
    for (change, paths) in changes {
        // A node id is the change's own short id, so the change the read names is the node
        // whose id prefixes its full change id (D1b, S2-set).
        let Some((id, document)) = documents
            .iter()
            .find(|(id, _)| change.starts_with(id))
            .copied()
        else {
            continue;
        };
        let Some(id) = Id::parse(id) else {
            continue;
        };
        let owning: BTreeSet<PathBuf> = owned(&id, document);
        for path in paths {
            if !owning.contains(&PathBuf::from(path)) {
                lines.push(line("custody", id.as_str(), path));
            }
        }
    }
    lines
}

/// The evidence a custody read that could not itself run produced (S3-err, xotxspwp): jj's own
/// words, carried rather than collapsed, and reported as the line's third position (O10, S4-own).
/// No change is called clean by a check that never ran, and no node or change is named.
fn custody_evidence(error: &CustodyError) -> String {
    match error {
        CustodyError::Spawn(reason) => reason.clone(),
        CustodyError::Failed(output) => output.stderr.trim().to_string(),
    }
}

/// Every loaded node's kind, by id (a node that did not load has none).
fn kinds(nodes: &BTreeMap<&str, &NodeRead>) -> BTreeMap<String, String> {
    nodes
        .iter()
        .filter_map(|(id, read)| match *read {
            NodeRead::Loaded { document, .. } => Some((
                id.to_string(),
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
fn evaluations<'a>(nodes: &'a BTreeMap<&str, &NodeRead>) -> Vec<(&'a str, &'a Edges)> {
    nodes
        .iter()
        .filter_map(|(id, read)| match *read {
            NodeRead::Loaded { document, edges }
                if document.get("kind").and_then(Value::as_str) == Some("evaluation") =>
            {
                Some((*id, edges))
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

/// Resolve one edge target from the set already read: an id that names the root or a sibling
/// document, else the reason it does not. The rule stays in M-node-resolve's set-backed entry
/// point (S2-resolve-in-set); no target is read again.
fn resolution(nodes: &BTreeMap<&str, &NodeRead>, target: &str) -> Result<(), String> {
    let Some(id) = Id::parse(target) else {
        return Err(format!("{target} is not a node id"));
    };
    let lookup = |target: &str| match nodes.get(target).copied() {
        Some(NodeRead::Loaded { document, .. }) => Lookup::Loaded(document),
        Some(NodeRead::Failed(LoadError::Missing)) | None => Lookup::Absent,
        Some(NodeRead::Failed(error)) => Lookup::Failed(reason_ref(error)),
    };
    match resolve_in(&id, lookup) {
        Ok(()) => Ok(()),
        Err(error) => Err(format!("{target} does not resolve: {}", reason(&error))),
    }
}

/// S2-invalid's failure, as its reason.
fn reason(error: &LoadError) -> String {
    match error {
        LoadError::Missing => "missing node document".to_string(),
        LoadError::Unreadable(reason)
        | LoadError::NotJson(reason)
        | LoadError::Invalid(reason)
        | LoadError::Schema(reason) => reason.clone(),
    }
}

/// S2-invalid's failure, borrowed from the carried reason.
fn reason_ref(error: &LoadError) -> &str {
    match error {
        LoadError::Missing => "missing node document",
        LoadError::Unreadable(reason)
        | LoadError::NotJson(reason)
        | LoadError::Invalid(reason)
        | LoadError::Schema(reason) => reason,
    }
}

/// The directory-level read failure: the store could not be listed at all (S2).
fn list_failed(error: LoadError) -> Refusal {
    Refusal::new(reason(&error))
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}
