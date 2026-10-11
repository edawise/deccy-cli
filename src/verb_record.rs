//! The record verb: place the named working-copy edits, or land one named draft, into the
//! named node's change.
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
//! per oqnsuzqu) — answered before the store is taken, so a root-less context still answers it
//! (H1, qtqnuqxo), as do the form's refusals. I1-serde (orrvsxqq) fixes the JSON layer the module is written against.
//!
//! `--from <handle>` adds the source form uvrltuvo settles: the same recording, reached
//! through the draft the handle names, so it reads `.deccy/work/<handle>.json` through
//! M-draft-registry (rroqqrzq, whose read refuses a non-conforming document per ntypuupx),
//! refuses an absent or refused handle (uvrltuvo), requires `--to` to be the record's node
//! (uvrltuvo), refuses a source that is the target change (uvrltuvo), and records the changed
//! paths of the landable set (uvrltuvo, vmwvnnzv): the target change's current owned set
//! (uvrltuvo, xzxtxymq) together with the ref paths the draft's node document adds, read from
//! the draft's workspace (vmwvnnzv). The draft's content for each path of that set is compared
//! with the content at the owning change (qrqsxyzz); the target's current set stays the
//! baseline the staleness check reads (qrqsxyzz), and the set the journal covers is the union,
//! so an added path is kept with a null before side and undo removes it (okkvnnvm, qvkuluyw).
//! A dropped ref still in the current set is a change only when the draft's content differs, so
//! a carried deletion lands and an untouched file is not placed (vmwvnnzv). The landing refuses
//! when any open record naming the node is stale, naming each stale handle with its differing
//! paths (ptlmptoy, qrqsxyzz), and turns the record `landed` with the same M-draft-registry
//! write inside the one capture bracket (qvkuluyw, ttkuyvxp). The landing is non-interactive:
//! when the draft's working copy and the node's change both carry a description it keeps the
//! destination's rather than asking a person to combine the two, so the node's change is left
//! with the description it had (rvyrosmp, zylowqkr, uvrltuvo). The `[path...]` arguments stay
//! optional in this form and, when given, must state the changed landable set exactly
//! (uvrltuvo, vmwvnnzv). This module renders nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::context::Context;
use crate::draft_identity;
use crate::draft_registry::{self, Error as RegistryError};
use crate::jj::{Jj, JjError, Output};
use crate::jj_place;
use crate::markers_capture::{self, CaptureError};
use crate::markers_persist::PersistError;
use crate::node_load::LoadError;
use crate::node_own::owned;
use crate::node_resolve::{resolve, Resolved};
use crate::oplog::OplogError;
use crate::refusal::Refusal;
use crate::status::{self, StatusError};
use crate::store::Store;
use crate::store_path::{node_path, Id};
use crate::store_read::{self, Document, ReadError};

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str = "deccy record <path>... --to <id> | deccy record --from <handle> --to <id> \
                        [<path>...]: place the named edits into <id>'s change.";

/// The work-state registry, relative to the repository root (M-draft-store, twpvqstu).
const REGISTRY: &str = ".deccy/work";

/// A record's state before its draft lands (vkkwusty): only an open draft is landed.
const OPEN: &str = "open";

/// A record's state from its landing onward (vkkwusty): the record is retained `landed`.
const LANDED: &str = "landed";

/// The record handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    record(context, args)
}

/// The record operation (wtxymztk): answer the argument surface before the store is taken
/// (H1, qtqnuqxo), then parse the form, resolve the named purpose, and go down the path form
/// or the source form. Success is silent (O7-record).
fn record(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    let invocation = parse(args)?;
    let store = context.store()?;
    let jj = context.jj();
    let id = Id::parse(&invocation.to).ok_or_else(|| unknown(&invocation.to))?;
    let document = match resolve(jj, &id) {
        Ok(Resolved::Node(document)) => document,
        Ok(Resolved::Root) | Err(LoadError::Missing) => return Err(no_change(&invocation.to)),
        Err(_) => return Err(unknown(&invocation.to)),
    };
    let owned = owned(&id, &document);
    match invocation.from.as_deref() {
        None => place_named(store, jj, &id, &owned, &invocation.paths),
        Some(handle) => land(store, jj, &id, &owned, handle, &invocation.paths),
    }
}

/// The path form (D13, R1–R4): check every named path before anything moves, then bracket the
/// placed recording. Unchanged by the source form.
fn place_named(
    store: &Store,
    jj: &Jj,
    id: &Id,
    owned: &BTreeSet<PathBuf>,
    paths: &[String],
) -> Result<Vec<String>, Refusal> {
    refuse_unplaceable(jj, owned, paths)?;
    let outcome = markers_capture::capture_recording(
        store,
        jj,
        id.as_str(),
        owned,
        || {
            let outcome = jj_place::place(jj, id.as_str(), paths);
            (outcome, Some(id.as_str().to_string()))
        },
    )
    .map_err(capture_failed)?;
    report(outcome, id)?;
    Ok(Vec::new())
}

/// The source form (uvrltuvo): land one open draft named by its handle into the node `--to`
/// names. Every refusal precedes the one capture bracket, so a refused landing places nothing;
/// inside the bracket the changed paths of the landable set move into the node's change and the
/// record turns `landed` (qvkuluyw, vmwvnnzv).
fn land(
    store: &Store,
    jj: &Jj,
    id: &Id,
    owned: &BTreeSet<PathBuf>,
    handle: &str,
    named: &[String],
) -> Result<Vec<String>, Refusal> {
    let registry = store.root().join(REGISTRY);
    let mut record = draft_registry::read(&registry, handle)
        .map_err(|error| record_refused(handle, error))?;
    if record.get("state").and_then(Value::as_str) != Some(OPEN) {
        return Err(Refusal::new("the draft has already landed").with_evidence(handle));
    }
    let node = record.get("node").and_then(Value::as_str).unwrap_or_default();
    if node != id.as_str() {
        return Err(Refusal::new("the record names another node")
            .with_evidence(handle)
            .with_evidence(node)
            .with_evidence(id.as_str()));
    }
    if record.get("edit").and_then(Value::as_str) == Some(id.as_str()) {
        return Err(Refusal::new("the source is the target change").with_evidence(handle));
    }
    let workspace = workspace_path(handle, &record)?;
    let name = record
        .pointer("/workspace/name")
        .and_then(Value::as_str)
        .ok_or_else(|| Refusal::new("the record names no workspace").with_evidence(handle))?;
    let target = target_content(jj, id, owned)?;
    let current = base_of(&target);
    refuse_stale(&registry, id.as_str(), &current)?;
    // The landable set is wider than the target's current set: the draft's own document may
    // list refs the target does not own yet (vmwvnnzv). The staleness check above stays on the
    // current set, which is the baseline the record was opened against (qrqsxyzz); the
    // comparison, the disagreement check, the placement and the journal all read the union.
    let landable = landable_set(&workspace, id, owned)?;
    let draft = draft_content(&workspace, &landable)?;
    let changed = changed_paths(&target, &draft);
    if changed.is_empty() {
        return Err(Refusal::new("the draft has no changed path to land").with_evidence(handle));
    }
    refuse_disagreeing(named, &changed)?;
    let moved: Vec<String> = changed.into_iter().collect();

    // The draft's working copy is the squash source, so it is settled first: the landing moves
    // the content the draft carries, not a stale copy of it. The snapshot is taken before the
    // bracket opens, so it is not part of the recording's window.
    let draft_jj = Jj::new(jj.binary(), &workspace);
    let settled = draft_jj
        .run(&["util", "snapshot"])
        .map_err(|error| spawn_failed(error).with_evidence(handle))?;
    if settled.status != 0 {
        return Err(Refusal::new(settled.stderr.trim().to_string()).with_evidence(handle));
    }
    let source = format!("{name}@");
    // The landing is non-interactive and the node's change keeps its own description
    // (rvyrosmp, zylowqkr, uvrltuvo): when the source and the destination both carry a
    // description, the default squash asks a person to combine them, and a landing has no
    // person. The destination's description is the node's - the summary the graph lists - so
    // it is the one kept and the source's is discarded, never merged or edited, riding inside
    // the one recording (qvkuluyw).
    let mut command: Vec<&str> = vec![
        "squash",
        "--use-destination-message",
        "--from",
        source.as_str(),
        "--into",
        id.as_str(),
        "--",
    ];
    command.extend(moved.iter().map(String::as_str));

    // The landing is the one recording: the placement and the record's `landed` write ride
    // inside the same bracket, so the registry write is not an operation of its own
    // (qvkuluyw). It is invoked from the recording's own workspace with the draft's working
    // copy named as the source, so the workspace that makes the recording is the one jj
    // updates as the target's change is rewritten. The bracket snapshots the landable set, so
    // the path the draft's document adds is kept with a null before side and undo removes it
    // (okkvnnvm, vmwvnnzv).
    let outcome = markers_capture::capture_recording(
        store,
        jj,
        id.as_str(),
        &landable,
        || {
            let placed = match jj.run(&command) {
                Ok(output) if output.status == 0 => {
                    record["state"] = json!(LANDED);
                    draft_registry::write(&registry, &record)
                        .map_err(|error| record_refused(handle, error))
                }
                Ok(output) => {
                    Err(Refusal::new(output.stderr.trim().to_string()).with_evidence(handle))
                }
                Err(error) => Err(spawn_failed(error).with_evidence(handle)),
            };
            (placed, Some(id.as_str().to_string()))
        },
    )
    .map_err(capture_failed)?;
    outcome?;
    Ok(Vec::new())
}

/// The parsed form: one required `--to` purpose, the optional `--from <handle>` source, and
/// the named paths, in the order given (D13, R3; the source form is uvrltuvo).
struct Invocation {
    from: Option<String>,
    to: String,
    paths: Vec<String>,
}

/// Parse the form's arguments in any order (R3): one `--to <id>`, at most one
/// `--from <handle>`, and the positional `<path>` values. An argument the form does not name —
/// a flag, or `--to`/`--from` without its value or named twice — refuses, naming it (R2). No
/// `--to` refuses (yrmxrywl). The path form naming no path refuses (R1); the source form's
/// paths are optional because the draft and the owned set name the edits (uvrltuvo).
fn parse(args: &[String]) -> Result<Invocation, Refusal> {
    let mut from: Option<String> = None;
    let mut to: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        if argument == "--from" {
            if from.is_some() {
                return Err(Refusal::new("a second --from").with_evidence("--from"));
            }
            from = Some(value(args, index, "--from")?);
            index += 2;
        } else if argument == "--to" {
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
    if from.is_none() && paths.is_empty() {
        return Err(Refusal::new("no path").with_evidence("<path>"));
    }
    Ok(Invocation { from, to, paths })
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
    owned: &BTreeSet<PathBuf>,
    paths: &[String],
) -> Result<(), Refusal> {
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

/// The content each owned path holds at the target change (S2-read, nqktpzuu). Absence at the
/// change is `None`, distinct from an empty document; the read is the one `src/store_read.rs`
/// provides, so the landing compares content exactly as the opening hashed it.
fn target_content(
    jj: &Jj,
    id: &Id,
    owned: &BTreeSet<PathBuf>,
) -> Result<BTreeMap<String, Option<String>>, Refusal> {
    let mut contents = BTreeMap::new();
    for path in owned {
        let relative = relative(path)?;
        let content = match store_read::read(jj, &relative, id.as_str()) {
            Ok(Document::Whole(text)) => Some(text),
            Ok(Document::Absent) => None,
            Err(error) => return Err(read_failed(&relative, error)),
        };
        contents.insert(relative, content);
    }
    Ok(contents)
}

/// The content-identity base of the owned set as it stands at the landing (qrqsxyzz): one
/// entry per owned path, each the identity of that path's content, or `None` when the path is
/// not there.
fn base_of(content: &BTreeMap<String, Option<String>>) -> BTreeMap<String, Option<String>> {
    content
        .iter()
        .map(|(path, text)| {
            (
                path.clone(),
                text.as_ref()
                    .map(|text| draft_identity::identity(text.as_bytes())),
            )
        })
        .collect()
}

/// The landable set of a landing (uvrltuvo, vmwvnnzv): the target change's current owned set
/// together with the ref paths the draft's node document adds.
///
/// The draft's document is `nodes/<id>.json` in the draft's workspace, where the draft writes
/// it; it is read there and parsed, and the paths the ownership rule derives from it (M-node-own
/// over its own `/content/ref`) join the set. The document itself is already in the current set
/// (S2-path), so it lands like any changed path. A ref the draft's document no longer lists
/// stays in the set through the target's current set: the draft's content then decides whether
/// it is a carried deletion (a change) or untouched (not a change). A draft that carries no
/// document names no added ref, so the current set stands alone; a document that cannot be read
/// or parsed names no refs, and the landing refuses rather than guessing which paths it owns.
fn landable_set(
    workspace: &Path,
    id: &Id,
    current: &BTreeSet<PathBuf>,
) -> Result<BTreeSet<PathBuf>, Refusal> {
    let mut landable = current.clone();
    let path = node_path(id);
    let full = workspace.join(&path);
    let bytes = match std::fs::read(&full) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(landable),
        Err(error) => {
            return Err(Refusal::new(format!("{}: {error}", full.display()))
                .with_evidence(path.to_string_lossy().into_owned()))
        }
    };
    let document: Value = serde_json::from_slice(&bytes).map_err(|error| {
        Refusal::new(format!(
            "{}: the node document cannot be read: {error}",
            full.display()
        ))
        .with_evidence(path.to_string_lossy().into_owned())
    })?;
    landable.extend(owned(id, &document));
    Ok(landable)
}

/// The content the draft's workspace carries for each path of the landable set (uvrltuvo,
/// vmwvnnzv): the draft's own files are the source of the edits, so they are read from its
/// workspace, not the target's change. Absence in the workspace is `None` — a deletion the
/// draft carries.
fn draft_content(
    workspace: &Path,
    landable: &BTreeSet<PathBuf>,
) -> Result<BTreeMap<String, Option<Vec<u8>>>, Refusal> {
    if !workspace.is_dir() {
        return Err(Refusal::new("the draft's workspace is not there")
            .with_evidence(workspace.display().to_string()));
    }
    let mut contents = BTreeMap::new();
    for path in landable {
        let relative = relative(path)?;
        let full = workspace.join(path);
        let content = match std::fs::read(&full) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(Refusal::new(format!("{}: {error}", full.display()))
                    .with_evidence(relative))
            }
        };
        contents.insert(relative, content);
    }
    Ok(contents)
}

/// The paths of the landable set whose draft content differs from the content the target change
/// holds (uvrltuvo, vmwvnnzv): only the changed paths are recorded.
///
/// Every path of either side is compared, so a path the draft's document adds — absent at the
/// target, present in the draft — is a change even though the target's map does not hold it.
/// Absence on one side and presence on the other differs; absence on both does not, so a newly
/// listed ref that neither the target nor the draft carries is not a change, and the document
/// that lists it still lands.
fn changed_paths(
    target: &BTreeMap<String, Option<String>>,
    draft: &BTreeMap<String, Option<Vec<u8>>>,
) -> BTreeSet<String> {
    let mut paths: BTreeSet<&String> = target.keys().collect();
    paths.extend(draft.keys());
    paths
        .into_iter()
        .filter(|path| {
            let before: Option<&[u8]> = target
                .get(*path)
                .and_then(|text| text.as_deref())
                .map(str::as_bytes);
            let after: Option<&[u8]> = draft.get(*path).and_then(|bytes| bytes.as_deref());
            before != after
        })
        .cloned()
        .collect()
}

/// The source form's `[path...]` arguments state the changed landable set, they do not select a
/// partial landing (uvrltuvo, vmwvnnzv): every named path must be in the set and changed, and
/// every changed path of the set must be named, or nothing is placed.
fn refuse_disagreeing(named: &[String], changed: &BTreeSet<String>) -> Result<(), Refusal> {
    if named.is_empty() {
        return Ok(());
    }
    let stated: BTreeSet<&str> = named.iter().map(String::as_str).collect();
    for path in named {
        if !changed.contains(path.as_str()) {
            return Err(Refusal::new("a named path is not a changed path of the landable set")
                .with_evidence(path));
        }
    }
    for path in changed {
        if !stated.contains(path.as_str()) {
            return Err(
                Refusal::new("a changed path of the landable set is not named").with_evidence(path)
            );
        }
    }
    Ok(())
}

/// Refuse the landing when any open record naming the target node is stale (ptlmptoy):
/// compare each record's base with the owned set as it stands now and, for every record whose
/// base and owned content disagree somewhere, name its handle and its differing paths.
fn refuse_stale(
    registry: &Path,
    target: &str,
    current: &BTreeMap<String, Option<String>>,
) -> Result<(), Refusal> {
    let records = draft_registry::scan(registry).map_err(|error| Refusal::new(error.to_string()))?;
    let mut stale: Vec<(String, Vec<String>)> = Vec::new();
    for (handle, record) in records {
        if record.get("state").and_then(Value::as_str) != Some(OPEN) {
            continue;
        }
        if record.get("node").and_then(Value::as_str) != Some(target) {
            continue;
        }
        let differing = differing_paths(current, &record_base(&record));
        if !differing.is_empty() {
            stale.push((handle, differing));
        }
    }
    if stale.is_empty() {
        return Ok(());
    }
    let mut refusal = Refusal::new("a stale draft refuses the landing");
    for (handle, differing) in stale {
        refusal = refusal.with_evidence(handle);
        for path in differing {
            refusal = refusal.with_evidence(path);
        }
    }
    Err(refusal)
}

/// The base a record carries, read as one entry per path (qrqsxyzz): a value that is not a
/// string cannot equal a content identity, so it compares as a difference.
fn record_base(record: &Value) -> BTreeMap<String, Option<String>> {
    record
        .get("base")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .map(|(path, value)| (path.clone(), value.as_str().map(str::to_string)))
                .collect()
        })
        .unwrap_or_default()
}

/// The paths on which a record's base and the owned set disagree (qrqsxyzz): a value that no
/// longer matches, a path the base holds that the owned set has lost, and a path the owned set
/// holds that the base does not are each a difference.
fn differing_paths(
    current: &BTreeMap<String, Option<String>>,
    base: &BTreeMap<String, Option<String>>,
) -> Vec<String> {
    let mut paths: BTreeSet<&String> = current.keys().collect();
    paths.extend(base.keys());
    paths
        .into_iter()
        .filter(|path| current.get(*path) != base.get(*path))
        .cloned()
        .collect()
}

/// The workspace path the record names (owyntkvw), where the draft's edits live.
fn workspace_path(handle: &str, record: &Value) -> Result<PathBuf, Refusal> {
    record
        .pointer("/workspace/path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| Refusal::new("the record names no workspace path").with_evidence(handle))
}

/// One owned path as a repo-relative string; every path the ownership rule derives is UTF-8.
fn relative(path: &PathBuf) -> Result<String, Refusal> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| Refusal::new("an owned path is not valid UTF-8"))
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

/// A record the registry could not read or write: the registry's reason names the path it
/// refused (ntypuupx, rroqqrzq), and the handle is named back.
fn record_refused(handle: &str, error: RegistryError) -> Refusal {
    Refusal::new(error.to_string()).with_evidence(handle)
}

/// A failed read of an owned path (S2-read-fail): the evidence is carried, never collapsed.
fn read_failed(path: &str, error: ReadError) -> Refusal {
    match error {
        ReadError::Spawn(reason) => Refusal::new(reason).with_evidence(path),
        ReadError::Failed(output) => {
            Refusal::new(output.stderr.trim().to_string()).with_evidence(path)
        }
    }
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

