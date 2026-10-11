//! The remove verb: abandon the node's change when nothing depends on it.
//!
//! D14 (stnqmzuw) fixes the guard — `deccy remove <id>` abandons the node's change only when
//! nothing depends on it, refusing and naming its dependants otherwise (wnxsoknq, zvtktmpz),
//! the happy path being zrvnlzvw's clause. D7 (myprwuxt) fixes that removal takes the named
//! node's own gates with it, and refuses while a gate still covers beyond it. With no id at
//! all it refuses (ouyrkmlr); RM1's argument surface refuses any argument the form does not
//! name — a flag, or a second id — naming it. D21 (tnqpoqxl, tsspuwzo, xtmprysl) refuses an
//! id that maps to neither a valid node nor the root, naming it — the read is what decides —
//! and removes nothing (qyynqqpx); RM4 refuses an id that names no change — the root, the one
//! id that resolves yet owns none, every other such id being D21's. RM3 refuses a node that
//! did not load, naming it with the store's reason (S2-invalid); RM2 refuses a gate that
//! evaluates the named node among others, naming
//! the gate. The removal brackets through M-markers-capture (okkvnnvm) — the lifecycle
//! table's removal case (S5-lifecycle, vwlrqoxk) — abandoning the named change through
//! M-jj-abandon (urtkpxwv, S3-abandon xllzxxvo) and each orphaned gate after it; the node set
//! is scanned through M-node-set (nykqvuqr). A successful removal writes nothing (O7-remove,
//! loxpnryv). It resolves the id through M-node-resolve (wynwwtur, hence M-node-load and
//! M-store-read, across the runner M-jj), names the file through M-store-path (vkztyrzm),
//! hands the store to the capture bracket (okkvnnvm), builds
//! its refusals through M-refusal (popowxnw), takes the injected context (M-context zttrwlts,
//! M-context-injection ryuswoxz), and returns its lines for the envelope to write
//! (M-output-return, utrzvyyt) under M-route's handler shape (ynvwvzrm), carrying its own
//! --help text (xlvoxmyl/rwlywqrk, per oqnsuzqu) — answered before the store is taken, so a
//! root-less context still answers it (H1, qtqnuqxo), as do the form's refusals. I1-serde (orrvsxqq) fixes the JSON layer the
//! module is written against. This module renders nothing.

use std::path::Path;

use crate::context::Context;
use crate::jj::{Jj, JjError};
use crate::jj_abandon;
use crate::markers_capture::{self, CaptureError};
use crate::markers_persist::PersistError;
use crate::oplog::OplogError;
use crate::refusal::Refusal;
use crate::node_set::{self, NodeRead};
use crate::node_load::LoadError;
use crate::store_path::Id;
use crate::node_resolve::{resolve, Resolved};

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str = "deccy remove <id>: abandon the node's change when nothing depends on it.";

/// The remove handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    remove(context, args)
}

/// The remove operation: answer the argument surface before the store is taken (H1,
/// qtqnuqxo), then parse the form, resolve the id, scan the node set for dependants and the
/// node's own gates, and bracket an abandonment. Success is silent (O7-remove).
fn remove(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    let raw = parse(args)?;
    let store = context.store()?;
    let jj = context.jj();
    let id = Id::parse(&raw).ok_or_else(|| unknown(&raw))?;
    match resolve(jj, &id).map_err(|_| unknown(&raw))? {
        Resolved::Root => return Err(no_change(&raw)),
        Resolved::Node(_) => {}
    }
    let gates = refuse_dependants(jj, context.git().ok(), id.as_str())?;
    let outcome = markers_capture::capture(store, jj, crate::markers::Kind::Remove, || {
        (
            abide(jj, id.as_str(), &gates),
            Some(id.as_str().to_string()),
        )
    })
    .map_err(capture_failed)?;
    outcome?;
    Ok(Vec::new())
}

/// Parse the form's argument surface (RM1): exactly one argument that does not begin with `-`
/// is the id; a missing argument refuses (ouyrkmlr), and any other argument — one beginning
/// with `-`, or a second one — refuses, naming it.
fn parse(args: &[String]) -> Result<String, Refusal> {
    let mut id: Option<String> = None;
    for argument in args {
        if argument.starts_with('-') {
            return Err(Refusal::new("an unnamed argument").with_evidence(argument.clone()));
        }
        if id.is_some() {
            return Err(Refusal::new("a second id").with_evidence(argument.clone()));
        }
        id = Some(argument.clone());
    }
    id.ok_or_else(|| Refusal::new("no id").with_evidence("<id>"))
}

/// Scan the node set (M-node-set) and refuse the whole invocation if anything still reaches
/// the named node: a node whose `depends_on` names it, or a gate whose `evaluates` names it
/// among others, refuses, naming every dependant (wnxsoknq, zvtktmpz, RM2); a node that did
/// not load refuses, naming it with the store's reason (RM3, S2-invalid). The named node's own
/// gates — evaluations whose `evaluates` names only it — are returned to be abandoned with it
/// (D7). Nothing is abandoned before this clears.
fn refuse_dependants(jj: &Jj, git: Option<&Path>, id: &str) -> Result<Vec<String>, Refusal> {
    let nodes = node_set::list(jj, git).map_err(list_failed)?;
    let mut dependants: Vec<String> = Vec::new();
    let mut gates: Vec<String> = Vec::new();
    for (other, read) in &nodes {
        let edges = match read {
            NodeRead::Loaded { edges, .. } => edges,
            NodeRead::Failed(error) => {
                return Err(Refusal::new(reason(error)).with_evidence(other.clone()));
            }
        };
        if edges.depends_on.iter().any(|d| d == id) {
            dependants.push(other.clone());
        }
        if edges.evaluates.iter().any(|e| e == id) {
            if edges.evaluates.len() > 1 {
                dependants.push(other.clone());
            } else {
                gates.push(other.clone());
            }
        }
    }
    if dependants.is_empty() {
        return Ok(gates);
    }
    let mut refusal = Refusal::new("a node with dependants");
    for dependant in dependants {
        refusal = refusal.with_evidence(dependant);
    }
    Err(refusal)
}

/// Abandon the named change first, then each of the named node's own gates (D7); the first
/// crossing that fails refuses, naming the id it could not abandon (RM4).
fn abide(jj: &Jj, id: &str, gates: &[String]) -> Result<(), Refusal> {
    abandon(jj, id)?;
    for gate in gates {
        abandon(jj, gate)?;
    }
    Ok(())
}

/// Abandon one change through M-jj-abandon (S3-abandon); a crossing that failed refuses,
/// naming the change (RM4).
fn abandon(jj: &Jj, id: &str) -> Result<(), Refusal> {
    let output = jj_abandon::abandon(jj, id).map_err(spawn_failed)?;
    if output.status != 0 {
        return Err(Refusal::new(output.stderr.trim().to_string()).with_evidence(id));
    }
    Ok(())
}

/// D21's refusal: the id that names neither a valid node nor the root, named back
/// (tsspuwzo, xtmprysl).
fn unknown(raw: &str) -> Refusal {
    Refusal::new("unknown id").with_evidence(raw)
}

/// RM4's refusal: the id resolves, but names no change — the root — named back.
fn no_change(raw: &str) -> Refusal {
    Refusal::new("no change for the id").with_evidence(raw)
}

/// S2-invalid's failure, read as its reason — the reason RM3 names alongside the node.
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

/// A crossing that could not spawn jj: the reason is carried (S3-err).
fn spawn_failed(error: JjError) -> Refusal {
    match error {
        JjError::Spawn(reason) => Refusal::new(reason),
    }
}

/// A bracket that did not close: its reason is carried, both variants (okkvnnvm).
fn capture_failed(error: CaptureError) -> Refusal {
    match error {
        CaptureError::Oplog(error) => Refusal::new(oplog_reason(&error)),
        CaptureError::Append(error) => Refusal::new(persist_reason(&error)),
        CaptureError::Journal(error) => Refusal::new(format!("{error:?}")),
    }
}

/// Why the op head could not be read (S3-oplog, mvwnksou).
fn oplog_reason(error: &OplogError) -> String {
    match error {
        OplogError::Spawn(reason) => reason.clone(),
        OplogError::Failed(output) => output.stderr.trim().to_string(),
    }
}

/// Why the marker could not be appended (S5-marker-corrupt, vqqmwsml).
fn persist_reason(error: &PersistError) -> String {
    match error {
        PersistError::Unreadable(reason)
        | PersistError::NotJson(reason)
        | PersistError::Invalid(reason)
        | PersistError::Schema(reason)
        | PersistError::Unwritable(reason) => reason.clone(),
    }
}
