//! The record verb: place the named working-copy edits into the named node's change.
//!
//! D13 (rvyrosmp) fixes the form — `deccy record <path>... --to <id>` places the named file
//! edits into the named node's change, purpose named and never inferred — and D6
//! (roqpxulw) fixes attribution, which change owns which edit. Without `--to` it refuses
//! (yrmxrywl); naming no path it refuses and places nothing (R1, zxrrwrky); an argument the
//! form does not name refuses, named (R2, mlmlqqnx); its arguments are recognized in any
//! order (R3, xznwtssz); a named path it cannot place — no working-copy edit, or not owned
//! by the target — refuses, named, placing nothing (R4, snzrypvt). D21 (tnqpoqxl, tsspuwzo)
//! refuses an id that maps to neither a valid node nor the root, naming it (xtmprysl) and
//! creating, recording, or removing nothing (qyynqqpx); an invocation naming no id at all
//! refuses (ouyrkmlr). A `--to` naming an id that maps to no node refuses, naming the id, and
//! places nothing — an id naming no change and a change carrying no node document both map to
//! no node (uzlvzrpv), while D21 refuses the id that resolves to neither a node nor the root
//! (tsspuwzo). It creates no node (lypvkskv) and
//! lands no edit outside an owning change (xklsyrrv); on success nothing is written to stdout
//! (O7-record, wvqzoqvx). It reads the working copy's
//! changed paths through M-jj-status (uktxyxmq, S3-status vkowzusm), attributes with
//! M-node-own (lwnnssuo, S2-own szqpmtrs) over the resolved document (the ownership rule,
//! D2, xzxtxymq), and moves the named paths into the named change through M-jj-place
//! (vyklrqym, S3-place zylowqkr), bracketing the recording through M-markers-capture
//! (okkvnnvm) — the lifecycle table's recording case (S5-lifecycle, vwlrqoxk). It resolves
//! the id through M-node-resolve (wynwwtur, hence M-node-load and M-store-read, across
//! the runner M-jj), names the file through M-store-path (vkztyrzm), hands the store to the
//! capture bracket (okkvnnvm), builds its refusals through
//! M-refusal (popowxnw), takes the injected context (M-context zttrwlts, M-context-injection
//! ryuswoxz), and returns its lines for the envelope to write (M-output-return, utrzvyyt)
//! under M-route's handler shape (ynvwvzrm), carrying its own --help text (xlvoxmyl/rwlywqrk,
//! per oqnsuzqu). I1-serde (orrvsxqq) fixes the JSON layer the module is written against.
//! This module renders nothing.

use std::path::PathBuf;

use crate::context::Context;
use crate::jj::{Jj, JjError, Output};
use crate::jj_place;
use crate::markers_capture::{self, CaptureError};
use crate::markers_persist::PersistError;
use crate::oplog::OplogError;
use crate::refusal::Refusal;
use crate::status::{self, StatusError};
use crate::store::Store;
use crate::node_own::owned;
use crate::store_path::Id;
use crate::node_load::LoadError;
use crate::node_resolve::{resolve, Resolved};

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy record <path>... --to <id>: place the named working-copy edits into <id>'s change.";

/// The record handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    let store = context.store()?;
    record(store, context.jj(), args)
}

/// The record operation (wtxymztk): parse the form, resolve the named purpose, check every
/// named path before anything moves, then bracket a placed recording. Success is silent
/// (O7-record).
fn record(store: &Store, jj: &Jj, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    let invocation = parse(args)?;
    let id = Id::parse(&invocation.to).ok_or_else(|| unknown(&invocation.to))?;
    let document = match resolve(jj, &id) {
        Ok(Resolved::Node(document)) => document,
        Ok(Resolved::Root) | Err(LoadError::Missing) => return Err(no_change(&invocation.to)),
        Err(_) => return Err(unknown(&invocation.to)),
    };
    refuse_unplaceable(jj, &id, &document, &invocation.paths)?;
    let outcome = markers_capture::capture(store, jj, || {
        jj_place::place(jj, id.as_str(), &invocation.paths)
    })
    .map_err(capture_failed)?;
    report(outcome, &id)?;
    Ok(Vec::new())
}

/// The parsed form: one required `--to` purpose and the named paths, in the order given
/// (D13, R3).
struct Invocation {
    to: String,
    paths: Vec<String>,
}

/// Parse the form's arguments in any order (R3): one `--to <id>` and the positional
/// `<path>` values. An argument the form does not name — a flag, or `--to` without its
/// value or a second `--to` — refuses, naming it (R2). No `--to` refuses (yrmxrywl); no
/// path refuses (R1).
fn parse(args: &[String]) -> Result<Invocation, Refusal> {
    let mut to: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        if argument == "--to" {
            if to.is_some() {
                return Err(Refusal::new("a second --to").with_evidence("--to"));
            }
            to = Some(value(args, index, "--to")?);
            index += 2;
        } else if argument.starts_with('-') {
            return Err(Refusal::new("an unnamed argument").with_evidence(argument));
        } else {
            paths.push(argument.to_string());
            index += 1;
        }
    }
    let to = to.ok_or_else(|| Refusal::new("no --to").with_evidence("--to"))?;
    if paths.is_empty() {
        return Err(Refusal::new("no path").with_evidence("<path>"));
    }
    Ok(Invocation { to, paths })
}

/// The value that follows a flag, or a refusal naming the flag that lacks one (R2).
fn value(args: &[String], index: usize, flag: &str) -> Result<String, Refusal> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| Refusal::new(format!("{flag} without its value")).with_evidence(flag))
}

/// Refuse the whole invocation when a named path cannot be placed: it must name a
/// working-copy edit (S3-status) and belong to the named target (S2-own), or nothing moves
/// (R4, xklsyrrv). The refusal names the path.
fn refuse_unplaceable(
    jj: &Jj,
    id: &Id,
    document: &serde_json::Value,
    paths: &[String],
) -> Result<(), Refusal> {
    let owned = owned(id, document);
    let changed = status::status(jj).map_err(status_failed)?;
    for path in paths {
        let changed_here = changed.iter().any(|entry| entry == path);
        let owned_here = owned.contains(&PathBuf::from(path));
        if !changed_here || !owned_here {
            return Err(Refusal::new("a path that cannot be placed").with_evidence(path));
        }
    }
    Ok(())
}

/// Interpret the placed recording: a crossing that could not spawn, or a `jj squash` that
/// failed, refuses naming the target id. Success is the caller's silence (O7-record).
fn report(outcome: Result<Output, JjError>, id: &Id) -> Result<(), Refusal> {
    let output = outcome.map_err(|error| spawn_failed(error).with_evidence(id.as_str()))?;
    if output.status != 0 {
        return Err(Refusal::new(output.stderr.trim().to_string()).with_evidence(id.as_str()));
    }
    Ok(())
}

/// D21's refusal: the id that names neither a valid node nor the root, named back
/// (tsspuwzo, xtmprysl).
fn unknown(raw: &str) -> Refusal {
    Refusal::new("unknown id").with_evidence(raw)
}

/// uzlvzrpv's refusal: the id maps to no node — it names no change, or the change it names
/// carries no node document — named back.
fn no_change(raw: &str) -> Refusal {
    Refusal::new("no node for the id").with_evidence(raw)
}

/// A placement that could not spawn jj: the reason is carried (S3-err).
fn spawn_failed(error: JjError) -> Refusal {
    match error {
        JjError::Spawn(reason) => Refusal::new(reason),
    }
}

/// A changed-path read that failed: the reason is carried (S3-err).
fn status_failed(error: StatusError) -> Refusal {
    match error {
        StatusError::Spawn(reason) => Refusal::new(reason),
        StatusError::Failed(output) => Refusal::new(output.stderr.trim().to_string()),
    }
}

/// A bracket that did not close: its reason is carried, both variants (okkvnnvm).
fn capture_failed(error: CaptureError) -> Refusal {
    match error {
        CaptureError::Oplog(error) => Refusal::new(oplog_reason(&error)),
        CaptureError::Append(error) => Refusal::new(persist_reason(&error)),
    }
}

/// Why the op head could not be read (S3-oplog, mvwnksou).
fn oplog_reason(error: &OplogError) -> String {
    match error {
        OplogError::Spawn(reason) => reason.clone(),
        OplogError::Failed(output) => output.stderr.trim().to_string(),
    }
}

/// Why the pair could not be appended (S5-marker-corrupt, vqqmwsml).
fn persist_reason(error: &PersistError) -> String {
    match error {
        PersistError::Unreadable(reason)
        | PersistError::NotJson(reason)
        | PersistError::Invalid(reason)
        | PersistError::Schema(reason)
        | PersistError::Unwritable(reason) => reason.clone(),
    }
}
