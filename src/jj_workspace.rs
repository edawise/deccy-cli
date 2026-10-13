//! Draft jj workspaces: create, forget and enumerate the working copies under the per-user
//! draft-workspace root.
//!
//! xqzstoyo fixes where a draft's working copy lives — under one per-user root outside the
//! repository, at the repository's namespace under that root plus the record's handle — and
//! lpkssxpp fixes that a creation lands at exactly that path. This module is the derivation and
//! the three jj crossings over it: `path_for` computes the place, `add` lands a working copy
//! there at the revision its caller names, `forget` drops one and `list` enumerates them.
//! pvnszwum fixes that start: the working copy is created at the change that owns the draft's
//! node, so the caller supplies that revision and `add` forwards it to jj. Every crossing goes
//! through the caller-supplied `Jj` seam (zruxtxrv): the binary and root are the caller's,
//! nothing is searched here, and a spawn failure or a jj refusal surfaces as a value carrying
//! its evidence (S3-err). The module is one flat file beside `src/jj.rs` (szmuyqrq), and the
//! gate that drives it is a self-contained test crate (knqkxzns).

use std::fs;
use std::path::{Path, PathBuf};

use crate::jj::{Jj, JjError, Output};

/// Why a workspace crossing did not complete: the evidence is carried, never collapsed.
#[derive(Debug)]
pub enum WorkspaceError {
    /// jj could not be spawned, or the destination could not be prepared; the reason is
    /// carried (S3-err).
    Spawn(String),
    /// jj ran and refused; its status and both streams are carried (S3-err).
    Refused(Output),
    /// The name is not among the workspaces jj reports; the name is carried.
    Unknown(String),
}

impl From<JjError> for WorkspaceError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => WorkspaceError::Spawn(reason),
        }
    }
}

/// The derivation fixed by xqzstoyo and lpkssxpp: the draft root, then the repository's own
/// namespace under it, then the record's handle — nothing appended.
pub fn path_for(root: &Path, namespace: &str, handle: &str) -> PathBuf {
    root.join(namespace).join(handle)
}

/// Create a working copy at `path`, name it `name` in jj, and start it at `revision`.
///
/// The destination's parent (the repository's namespace directory) is made first because jj
/// requires it; the leaf itself is left to `jj workspace add`, which owns the working copy at
/// the path the derivation yields (lpkssxpp). The working copy is created at `revision`, the
/// change the caller names — the change that owns the draft's node — so it starts from that
/// content rather than from whatever `@` happens to hold (pvnszwum); the revision is forwarded
/// to jj. The jj name is the caller's, unrelated to the handle the path carries.
pub fn add(jj: &Jj, path: &Path, name: &str, revision: &str) -> Result<(), WorkspaceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            WorkspaceError::Spawn(format!(
                "could not create the draft root {}: {error}",
                parent.display()
            ))
        })?;
    }
    let destination = path.to_str().ok_or_else(|| {
        WorkspaceError::Spawn(format!(
            "the workspace path is not valid UTF-8: {}",
            path.display()
        ))
    })?;
    let output = jj.run(&[
        "workspace",
        "add",
        destination,
        "--name",
        name,
        "--revision",
        revision,
    ])?;
    if output.status == 0 {
        Ok(())
    } else {
        Err(WorkspaceError::Refused(output))
    }
}

/// Forget the jj workspace `name`, which must be one jj already reports.
///
/// jj 0.46 exits zero and only warns when asked to forget a name it does not know, so membership
/// is checked against `list` first: an absent name comes back as `Unknown(name)` with nothing
/// removed. A name jj knows is dropped through the seam.
pub fn forget(jj: &Jj, name: &str) -> Result<(), WorkspaceError> {
    if !list(jj)?.iter().any(|known| known == name) {
        return Err(WorkspaceError::Unknown(name.to_string()));
    }
    let output = jj.run(&["workspace", "forget", name])?;
    if output.status == 0 {
        Ok(())
    } else {
        Err(WorkspaceError::Refused(output))
    }
}

/// The names of the workspaces jj reports.
///
/// The name template keeps jj's default line — which also carries a path and a change id — out
/// of the result, so the enumeration is exactly the names (zruxtxrv's seam, read for names).
pub fn list(jj: &Jj) -> Result<Vec<String>, WorkspaceError> {
    let output = jj.run(&["workspace", "list", "-T", "name ++ \"\\n\""])?;
    if output.status != 0 {
        return Err(WorkspaceError::Refused(output));
    }
    Ok(output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}
