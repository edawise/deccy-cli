//! The draft verb: open one draft of each node the call names, in a workspace of its own.
//!
//! xulmunov fixes the opening: the form `draft <node>... [--edit <id>]` opens, for each node
//! named — a node named twice opening two drafts (vnwmpwrx, szvotxpq) — a draft with a jj
//! workspace of its own, whose record is written `open` at `.deccy/work/<handle>.json`, and it
//! reports each draft as its handle together with its workspace path, in the order the nodes
//! were named. Each clause is a prerequisite read as one verb:
//!
//! - the surface (szvotxpq, vnwmpwrx): the node list is not deduplicated, because the draft
//!   model counts drafts, not nodes; `--edit` names the one edit a record carries and an
//!   omitted flag falls back to the edit the invocation is made in; the report is the handle
//!   and the path because that pair alone picks one draft out;
//! - the refusal (xzrkrmmr) and the argument surface (qtqnuqxo): a call naming no node and a
//!   call carrying an argument the form does not name refuse on the surface, before a store is
//!   taken, a workspace made or a record written, and the verb's own `--help` (xlvoxmyl,
//!   rwlywqrk) answers first, so a root-less context still answers it;
//! - the record (kwqnrzoz, owyntkvw): one document per draft, keyed by a minted handle, naming
//!   exactly one node, one edit and the workspace by name and path, and not a node document,
//!   so `nodes/` is untouched;
//! - the state (vkkwusty): a draft that has just opened has landed nothing, so the record is
//!   written `open`;
//! - the base (zsvukpls, ttkuyvxp): one content identity per path of the node's owned set, read
//!   at the change that owns the node through M-store-read across the runner (M-jj) and
//!   attributed by M-node-own;
//! - the workspace (xqzstoyo, lpkssxpp, tyoypkmp, pvnszwum): a working copy outside the
//!   repository, under one per-user root, at the repository's namespace under it plus the record's
//!   handle, created through M-jj-workspace's `add` over the caller's `Jj` seam at the change that
//!   owns the named node, so its working copy holds the owned set the base records;
//! - the order (qxuzyvsr): the workspace is created before the record is written, so a failed
//!   creation leaves no record naming a workspace that was never made;
//! - the handle (upuuwzmr) and the registry (rroqqrzq): the handle is minted fresh against the
//!   records that are there, and the record is stored by M-draft-registry at the path the handle
//!   names;
//! - the state root (mrwtusss, lotlstzr): the store is taken through the context's state
//!   accessor, so it is the store on the repository's trunk root from whichever workspace the
//!   call runs in. The registry under that root is the repository's one registry and the
//!   namespace derived from it is the repository's one namespace, so a draft opened from a
//!   linked workspace is filed with the repository's drafts and lands under the repository's
//!   namespace. Opening writes no graph state, so it is not one of the invocations the
//!   trunk-only door refuses and it keeps working outside the trunk;
//! - the handler shape (ynvwvzrm), the injected context (zttrwlts), the refusal value
//!   (popowxnw) and returned lines (utrzvyyt), with the flat module (szmuyqrq) and self-contained
//!   test (knqkxzns) layouts the shims rely on.
//!
//! Opening writes no graph state and no operation marker (ooquotol): the only file written is
//! the record in the untracked work registry, so undo and redo receive no unit. This module
//! renders nothing; it returns its lines for the envelope to write.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::context::Context;
use crate::draft_handle;
use crate::draft_identity;
use crate::draft_registry::{self, Error as RegistryError};
use crate::jj::{Jj, JjError};
use crate::jj_workspace::{self, WorkspaceError};
use crate::node_load::{self, LoadError};
use crate::node_own;
use crate::refusal::Refusal;
use crate::store_path::Id;
use crate::store_read::{self, Document, ReadError};

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str = "deccy draft <node>... [--edit <id>]: open a draft of each named node in a \
                        workspace of its own.";

/// The work-state registry, relative to the repository root (M-draft-store, twpvqstu).
const REGISTRY: &str = ".deccy/work";

/// The record's state before its landing (vkkwusty): an opened draft has landed nothing.
const OPEN: &str = "open";

/// The new handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    draft(context, args)
}

/// Open one draft per named node (xulmunov). The argument surface answers first
/// (qtqnuqxo): the verb's own help, then the form's refusal of a call naming no node
/// (xzrkrmmr) or an argument it does not name. Only then is the state taken, so a refused call
/// creates no workspace and writes no record.
fn draft(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    let invocation = parse(args)?;
    // The store on the trunk state root (mrwtusss, lotlstzr), in any workspace: the
    // repository's one registry and its one namespace follow from this root, so a call made
    // from a linked workspace files where a trunk call files. Opening writes no graph state,
    // so the trunk-only door does not reach it.
    let store = context.state()?;
    let jj = context.jj();
    let registry = store.root().join(REGISTRY);
    let edit = match invocation.edit {
        Some(edit) => edit,
        None => current_edit(jj)?,
    };
    let root = draft_root()?;
    let namespace = namespace_of(store.root());

    let mut lines = Vec::with_capacity(invocation.nodes.len());
    for raw in &invocation.nodes {
        let id = Id::parse(raw).ok_or_else(|| unknown_node(raw))?;
        let node = node_load::load(jj, &id).map_err(|error| load_failed(raw, &error))?;
        let base = base_of(jj, &id, &node)?;
        let handle = draft_handle::mint(|candidate| {
            draft_registry::path_for(&registry, candidate).exists()
        });
        let path = jj_workspace::path_for(&root, &namespace, &handle);
        let path_text = path
            .to_str()
            .ok_or_else(|| Refusal::new("the workspace path is not valid UTF-8"))?
            .to_string();
        let name = format!("draft-{handle}");
        // The workspace first, the record last (qxuzyvsr): the record is a claim about the
        // working copy, so a creation that fails leaves no record naming it. The workspace starts
        // at the node's own change (pvnszwum), so the working copy holds the owned set the base
        // records rather than whatever the invocation's change holds.
        jj_workspace::add(jj, &path, &name, id.as_str()).map_err(workspace_failed)?;
        let document = json!({
            "handle": handle,
            "state": OPEN,
            "node": id.as_str(),
            "edit": edit,
            "workspace": { "name": name, "path": path_text },
            "base": base,
        });
        draft_registry::write(&registry, &document).map_err(record_failed)?;
        lines.push(format!("{handle} {path_text}"));
    }
    Ok(lines)
}

/// The parsed form (szvotxpq): one or more node ids in the order given, and the one edit the
/// drafts belong to — the flag's value, or the invocation's own edit when it is omitted.
struct Invocation {
    nodes: Vec<String>,
    edit: Option<String>,
}

/// Parse the form's arguments. A call naming no node refuses (xzrkrmmr) and an argument the
/// form does not name refuses, naming it (qtqnuqxo). Parsing reaches nothing, so a refusal
/// here leaves no workspace and no record behind.
fn parse(args: &[String]) -> Result<Invocation, Refusal> {
    let mut nodes: Vec<String> = Vec::new();
    let mut edit: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--edit" => {
                if edit.is_some() {
                    return Err(Refusal::new("a second --edit").with_evidence("--edit"));
                }
                let value = args
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| Refusal::new("--edit without its value").with_evidence("--edit"))?;
                edit = Some(value);
                index += 2;
            }
            argument if argument.starts_with('-') => {
                return Err(Refusal::new("an unnamed argument").with_evidence(argument));
            }
            node => {
                nodes.push(node.to_string());
                index += 1;
            }
        }
    }
    if nodes.is_empty() {
        return Err(Refusal::new("no node given").with_evidence("draft"));
    }
    Ok(Invocation { nodes, edit })
}

/// The edit the invocation is made in (szvotxpq): the current working copy's change id,
/// read through the caller's `Jj` seam (M-jj).
fn current_edit(jj: &Jj) -> Result<String, Refusal> {
    let output = jj
        .run(&["log", "-r", "@", "--no-graph", "-T", "change_id"])
        .map_err(|error| spawn_failed(&error))?;
    if output.status != 0 {
        return Err(Refusal::new(output.stderr.trim().to_string()));
    }
    let id = output.stdout.trim();
    if id.is_empty() {
        return Err(Refusal::new("the current edit has no change id"));
    }
    Ok(id.to_string())
}

/// The per-user root drafts are written under (xqzstoyo): one directory per user, outside
/// every repository, holding one namespace per repository.
fn draft_root() -> Result<PathBuf, Refusal> {
    let home = env::var_os("HOME")
        .ok_or_else(|| Refusal::new("no home directory to place draft workspaces under"))
        .map(PathBuf::from)?;
    Ok(home
        .join(".local")
        .join("share")
        .join("deccy")
        .join("workspaces"))
}

/// The repository's own namespace under the per-user root (xqzstoyo): a directory derived from
/// the repository's canonical root alone, so two repositories never share a namespace and the
/// same repository always gets the same one. How the namespace is spelled is not fixed by the
/// prerequisites; the content identity the crate already carries is reused rather than a second
/// digest.
fn namespace_of(root: &Path) -> String {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let digest = draft_identity::identity(canonical.to_string_lossy().as_bytes());
    digest.chars().take(16).collect()
}

/// The base of the node's owned set (zsvukpls, ttkuyvxp): one content identity for every path
/// the set holds, each read at the change that owns the node through M-store-read (S2-read).
fn base_of(jj: &Jj, id: &Id, node: &Value) -> Result<BTreeMap<String, String>, Refusal> {
    let mut contents: Vec<(String, String)> = Vec::new();
    for path in node_own::owned(id, node) {
        let relative = path
            .to_str()
            .ok_or_else(|| Refusal::new("an owned path is not valid UTF-8"))?
            .to_string();
        match store_read::read(jj, &relative, id.as_str()) {
            Ok(Document::Whole(text)) => contents.push((relative, text)),
            Ok(Document::Absent) => {
                return Err(Refusal::new("an owned path is not at the node's change")
                    .with_evidence(relative))
            }
            Err(error) => return Err(read_failed(&relative, &error)),
        }
    }
    Ok(draft_identity::base(
        contents
            .iter()
            .map(|(path, content)| (path.as_str(), content.as_bytes())),
    ))
}

/// The refusal for an id that does not parse or does not name a node document.
fn unknown_node(raw: &str) -> Refusal {
    Refusal::new("unknown node").with_evidence(raw)
}

/// A node document that did not load (S2-invalid): the reason, with the id named as evidence.
fn load_failed(raw: &str, error: &LoadError) -> Refusal {
    let reason = match error {
        LoadError::Missing => "missing node document".to_string(),
        LoadError::Unreadable(reason)
        | LoadError::NotJson(reason)
        | LoadError::Invalid(reason)
        | LoadError::Schema(reason) => reason.clone(),
    };
    Refusal::new(reason).with_evidence(raw)
}

/// A failed read of an owned path (S2-read-fail): the evidence is carried, never collapsed.
fn read_failed(path: &str, error: &ReadError) -> Refusal {
    let cause = match error {
        ReadError::Spawn(reason) => reason.clone(),
        ReadError::Failed(output) => output.stderr.trim().to_string(),
    };
    Refusal::new(cause).with_evidence(path)
}

/// A failed workspace crossing (lxqk, S3-err): the evidence is carried, and because the record
/// is written last (qxuzyvsr) the failure leaves no record behind.
fn workspace_failed(error: WorkspaceError) -> Refusal {
    match error {
        WorkspaceError::Spawn(reason) => Refusal::new(reason),
        WorkspaceError::Refused(output) => Refusal::new(output.stderr.trim().to_string()),
        WorkspaceError::Unknown(name) => {
            Refusal::new("the workspace is not listed").with_evidence(name)
        }
    }
}

/// A failed record write (M-draft-store, twpvqstu): the error names the path it refused.
fn record_failed(error: RegistryError) -> Refusal {
    Refusal::new(error.to_string())
}

/// A crossing that could not run at all (S3-err): the reason is carried.
fn spawn_failed(error: &JjError) -> Refusal {
    match error {
        JjError::Spawn(reason) => Refusal::new(reason.clone()),
    }
}
