//! The drafts verb: list every draft record with its handle, state and workspace, and report
//! the open records whose workspace jj no longer has.
//!
//! nzrnrlky settles the surface: one verb, `drafts`, declaring no arguments in v1, which lists
//! exactly the registry's records and reports drift per record; qspsyqlp states the same
//! listing as a requirement (every record by handle, with its state and the workspace it
//! names, and the missing-workspace report only for a record that still asserts a draft);
//! vuopwuyo and yozqornv fix drift — jj owns workspace existence, the registry owns the
//! record's meaning, an open record whose named workspace jj lacks is drift and a landed one
//! is not, and the report changes nothing. vkkwusty supplies the two states, ntypuupx and
//! rroqqrzq supply the registry's scan (which refuses a non-conforming document rather than
//! hiding it), and tyoypkmp supplies jj's workspace names. The verb reads the registry and
//! jj only: it creates, rewrites, drops and forgets nothing.
//!
//! The registry is the repository's one registry, in the trunk's `.deccy/work/`, and the
//! verb reads it through the context's store rather than through the invocation workspace
//! (mrwtusss, M-TWO-ROOTS): a linked workspace is another checkout of the same repository,
//! not a second registry, and a listing is not a graph write, so an invocation there lists
//! the trunk's records instead of refusing. The drift check is unchanged by that — it is
//! still jj's workspace names against each record's own name.
//!
//! The handler returns its lines for the envelope to write (utrzvyyt) under the route table's
//! shape (ynvwvzrm) and the injected context (zttrwlts), a failure is one `Refusal` value
//! carrying its evidence (popowxnw), and the argument surface answers before the workspace is
//! read (qtqnuqxo), with `--help` answered from any context, root-less included (xlvoxmyl,
//! rwlywqrk). The module is one flat file under `src/` (szmuyqrq) whose gate is a
//! self-contained test crate (knqkxzns).

use serde_json::Value;

use crate::context::Context;
use crate::draft_registry;
use crate::jj_workspace::{self, WorkspaceError};
use crate::refusal::Refusal;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy drafts: list every draft record with its state and workspace, and report an open \
     record whose workspace jj no longer has.";

/// The registry directory under the store's root (twpvqstu, nolspvpw, M-TWO-ROOTS): the
/// records live in `.deccy/work/` under the trunk state root, which is the root the caller
/// names — never the invocation workspace's own tree (mrwtusss).
const REGISTRY_DIR: &str = ".deccy/work";

/// The verb's state word for a draft that has not landed (vkkwusty).
const OPEN: &str = "open";

/// The drafts handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    drafts(context, args)
}

/// List the registry's records and report drift (nzrnrlky, qspsyqlp); an invocation carrying
/// an argument refuses, naming it, before any workspace is read (qtqnuqxo), and `--help`
/// answers from the same place (xlvoxmyl).
fn drafts(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(lines_of(HELP));
    }
    if let Some(argument) = args.first() {
        return Err(Refusal::new("unexpected argument").with_evidence(argument.clone()));
    }

    let store = context.state()?;
    let records = draft_registry::scan(&store.root().join(REGISTRY_DIR)).map_err(registry_failed)?;
    let workspaces = jj_workspace::list(context.jj()).map_err(workspace_failed)?;

    Ok(records
        .iter()
        .map(|(handle, document)| row(handle, document, &workspaces))
        .collect())
}

/// One record's line: its handle, state and workspace by name and path (nzrnrlky,
/// owyntkvw), with the drift report appended only when an open record's workspace is not
/// among jj's (vuopwuyo, vkkwusty). A landed record carries no drift, so its line does not
/// depend on whether its workspace still exists (vuopwuyo).
fn row(handle: &str, document: &Value, workspaces: &[String]) -> String {
    let state = document.get("state").and_then(Value::as_str).unwrap_or("");
    let workspace = document.get("workspace");
    let name = workspace
        .and_then(|workspace| workspace.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let path = workspace
        .and_then(|workspace| workspace.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let drifted = state == OPEN && !workspaces.iter().any(|known| known == name);
    if drifted {
        format!("{handle} {state} {name} {path} drifted")
    } else {
        format!("{handle} {state} {name} {path}")
    }
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

/// The registry's refusal, read as its reason: the scan refuses a malformed document rather
/// than hiding it, and the refusal carries the path it read (ntypuupx, rroqqrzq).
fn registry_failed(error: draft_registry::Error) -> Refusal {
    Refusal::new(error.to_string())
}

/// Why jj's workspaces could not be read (tyoypkmp, popowxnw): the crossing's evidence is
/// carried, never collapsed into an empty listing.
fn workspace_failed(error: WorkspaceError) -> Refusal {
    match error {
        WorkspaceError::Spawn(reason) => Refusal::new(reason),
        WorkspaceError::Refused(output) => {
            let message = output.stderr.trim();
            if message.is_empty() {
                Refusal::new(format!("jj workspace list exited {}", output.status))
            } else {
                Refusal::new(message.to_string())
            }
        }
        WorkspaceError::Unknown(name) => Refusal::new("unknown workspace").with_evidence(name),
    }
}
