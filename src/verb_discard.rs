//! The discard verb: end one open draft per handle named.
//!
//! kxyznwxu fixes the surface — `deccy discard <handle>...`, one open draft per handle, the
//! handle the only key that addresses a draft, a repeated handle naming the same draft and a
//! handle that names no open record refusing the whole invocation. lnpnkvkv states it in the
//! requirement form; qtqnuqxo fixes that the argument surface answers first, `--help` included
//! and from a root-less context, and yumlvwzq that a discard is admitted only in the trunk
//! working copy. What the call then does is the end tvxspkyy settles, carried by three acts:
//! ywsulkzu abandons only the working-copy change the draft's workspace holds, ovmqxwuk forgets
//! the draft's jj workspace and then removes the directory the handle derives, vzposmvm gives
//! that workspace half its order and its tolerance of an absence, nwswzlnw makes the registry
//! removal the commit point after every named draft's acts, and qznqvpyr leaves the completed
//! acts standing and admits a re-run through them. wvltornz fixes the silence of a successful
//! call and utrzvyyt that the handler returns its lines rather than writing a stream.
//!
//! The store is taken through the context's trunk-only accessor (zttrwlts, lotlstzr), so a
//! discard outside the trunk refuses before any draft is discarded. The records are found by
//! scanning the registry `draft_registry` keeps at `.deccy/work/`; each is removed through that
//! module's removal. The jj crossings go through `jj_abandon` and `jj_workspace` over the
//! caller's `Jj` seam, while the directory is derived from the handle and the repository's
//! namespace under the per-user draft root — never read from the record's stored path. The
//! verb writes no node, no change and no marker, and returns an empty line list on success so
//! the envelope writes nothing.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::draft_identity;
use crate::draft_registry::{self, Error as RegistryError};
use crate::jj::{Jj, JjError};
use crate::jj_abandon;
use crate::jj_workspace::{self, WorkspaceError};
use crate::refusal::Refusal;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy discard <handle>...: end each named open draft without landing it.";

/// The work-state registry, relative to the repository root (M-draft-store, twpvqstu).
const REGISTRY: &str = ".deccy/work";

/// The record's state for a draft that has not landed (vkkwusty): the only subject of a discard.
const OPEN: &str = "open";

/// The discard handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    discard(context, args)
}

/// End the open drafts the call names (kxyznwxu, lnpnkvkv).
///
/// The argument surface answers first (qtqnuqxo): the verb's own help, then a missing handle or
/// an argument the form does not name. Only then is the trunk-only store taken (yumlvwzq): a
/// call outside the trunk refuses there, before any record is read or any draft discarded.
fn discard(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    let handles = parse(args)?;
    let store = context.store()?;
    let registry = store.root().join(REGISTRY);
    let records = draft_registry::scan(&registry).map_err(record_failed)?;

    // The whole call refuses when any handle names no open record (kxyznwxu, lnpnkvkv): the
    // offending handles are the evidence and nothing is discarded.
    let mut subjects: Vec<(String, String)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for handle in &handles {
        match records.iter().find(|(stored, _)| stored == handle) {
            Some((_, document))
                if document.get("state").and_then(|state| state.as_str()) == Some(OPEN) =>
            {
                let name = document
                    .get("workspace")
                    .and_then(|workspace| workspace.get("name"))
                    .and_then(|name| name.as_str())
                    .unwrap_or_default()
                    .to_string();
                subjects.push((handle.clone(), name));
            }
            _ => missing.push(handle.clone()),
        }
    }
    if !missing.is_empty() {
        let mut refusal = Refusal::new("a handle names no open draft");
        for handle in &missing {
            refusal = refusal.with_evidence(handle.clone());
        }
        return Err(refusal);
    }

    let jj = context.jj();
    let draft_root = draft_root()?;
    let namespace = namespace_of(store.root());

    // Every named draft's three acts complete before any record leaves the store (nwswzlnw):
    // the change the workspace holds is abandoned, the workspace is forgotten, and the
    // directory the handle derives is removed, in that order (ywsulkzu, ovmqxwuk, vzposmvm).
    for (handle, name) in &subjects {
        abandon_workspace_change(jj, name)?;
        forget_workspace(jj, name)?;
        let path = draft_root.join(&namespace).join(handle);
        remove_directory(&path)?;
    }

    // The registry removal is the commit: it is taken only after every act above completed, and
    // a failure before it has left every record at its handle (nwswzlnw).
    for (handle, _) in &subjects {
        draft_registry::remove(&registry, handle).map_err(record_failed)?;
    }

    // A successful discard writes nothing (wvltornz); the handler returns its lines for the
    // envelope to write (utrzvyyt).
    Ok(Vec::new())
}

/// Parse the form: a positional list of one or more handles (kxyznwxu).
///
/// A call naming no handle refuses (kxyznwxu) and an argument the form does not name refuses at
/// the same surface, naming it (qtqnuqxo). The handles themselves are not judged here: whether
/// one picks out an open record is answered from the store afterwards. Parsing reaches nothing,
/// so a refusal here leaves every draft as it stood.
fn parse(args: &[String]) -> Result<Vec<String>, Refusal> {
    let mut handles: Vec<String> = Vec::new();
    for argument in args {
        if argument.starts_with('-') {
            return Err(Refusal::new("an unnamed argument").with_evidence(argument.clone()));
        }
        // A repeated handle names the same draft, which the call discards once (owyntkvw).
        if !handles.iter().any(|known| known == argument) {
            handles.push(argument.clone());
        }
    }
    if handles.is_empty() {
        return Err(Refusal::new("no handle given"));
    }
    Ok(handles)
}

/// Abandon the working-copy change the draft's workspace holds (ywsulkzu).
///
/// "Gone" is jj not listing the workspace, and nothing else (tyoypkmp, vuopwuyo): the workspace
/// seam's enumeration is asked first, and only a name it does not carry means there is no change
/// to reach, so the act is skipped and jj's state is left alone (ywsulkzu).
///
/// A workspace jj still lists is a workspace whose change the act must reach. The change is read
/// at `<workspace-name>@` through the caller's `Jj` seam, so it is the change one step above the
/// workspace's start and never the node's owning change (vxltqruo). A failed read is not an
/// absence: a spawn failure or a non-zero status refuses the invocation, carrying the reason,
/// because the workspace is there and the change could not be identified — a failure before the
/// commit (nwswzlnw). Only a zero status yields the change id, which is then abandoned; a refused
/// abandon refuses the invocation the same way.
fn abandon_workspace_change(jj: &Jj, name: &str) -> Result<(), Refusal> {
    if name.is_empty() {
        return Ok(());
    }
    let known = jj_workspace::list(jj).map_err(workspace_failed)?;
    if !known.iter().any(|listed| listed == name) {
        // The workspace jj no longer holds: nothing is reached (ywsulkzu, vzposmvm).
        return Ok(());
    }
    let revision = format!("{name}@");
    let output = jj
        .run(&["log", "-r", &revision, "--no-graph", "-T", "change_id"])
        .map_err(|error| spawn_failed(&error))?;
    if output.status != 0 {
        // The workspace exists, so a failed read is not its absence: refuse before the commit.
        return Err(Refusal::new(output.stderr.trim().to_string()).with_evidence(revision));
    }
    let change = output.stdout.trim();
    if change.is_empty() {
        return Err(Refusal::new("the workspace's change id is empty").with_evidence(revision));
    }
    let abandoned = jj_abandon::abandon(jj, change).map_err(|error| spawn_failed(&error))?;
    if abandoned.status != 0 {
        return Err(
            Refusal::new(abandoned.stderr.trim().to_string()).with_evidence(change.to_string())
        );
    }
    Ok(())
}

/// Forget the draft's jj workspace through the workspace seam (ovmqxwuk, vzposmvm).
///
/// The seam checks the name against jj's enumeration and carries an absent name as `Unknown`
/// with nothing removed (tyoypkmp); an absence is the same discard carried through, not a
/// failure (vzposmvm). Every other crossing failure refuses the invocation.
fn forget_workspace(jj: &Jj, name: &str) -> Result<(), Refusal> {
    if name.is_empty() {
        return Ok(());
    }
    match jj_workspace::forget(jj, name) {
        Ok(()) | Err(WorkspaceError::Unknown(_)) => Ok(()),
        Err(error) => Err(workspace_failed(error)),
    }
}

/// Remove the directory the handle derives under the per-user root and the namespace
/// (ovmqxwuk, vzposmvm): that directory and no wider path.
///
/// An absent directory is the same discard carried through rather than a failure; any other io
/// failure is a failure before the commit and refuses the invocation.
fn remove_directory(path: &Path) -> Result<(), Refusal> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(
            Refusal::new(format!("the draft's directory could not be removed: {error}"))
                .with_evidence(path.to_string_lossy().into_owned()),
        ),
    }
}

/// The per-user root drafts are written under (xqzstoyo): one directory per user, outside every
/// repository, holding one namespace per repository.
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
/// same repository always gets the same one. The content identity the crate carries is reused
/// rather than a second digest.
fn namespace_of(root: &Path) -> String {
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let digest = draft_identity::identity(canonical.to_string_lossy().as_bytes());
    digest.chars().take(16).collect()
}

/// A failed registry scan or removal (M-draft-store, twpvqstu): the error names the path it
/// refused.
fn record_failed(error: RegistryError) -> Refusal {
    Refusal::new(error.to_string())
}

/// A failed workspace crossing (S3-err): the evidence is carried, never collapsed.
fn workspace_failed(error: WorkspaceError) -> Refusal {
    match error {
        WorkspaceError::Spawn(reason) => Refusal::new(reason),
        WorkspaceError::Refused(output) => Refusal::new(output.stderr.trim().to_string()),
        WorkspaceError::Unknown(name) => {
            Refusal::new("the workspace is not listed").with_evidence(name)
        }
    }
}

/// A crossing that could not run at all (S3-err): the reason is carried.
fn spawn_failed(error: &JjError) -> Refusal {
    match error {
        JjError::Spawn(reason) => Refusal::new(reason.clone()),
    }
}
