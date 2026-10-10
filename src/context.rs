//! The caller side of the seam: find jj and git, find the repo, open the store, hand all across.
//!
//! S3-explicit fixes that every crossing runs under a caller-supplied binary and root;
//! S3-caller fixes that the seam's caller resolves the jj binary via PATH once and the repo
//! root via `jj workspace root` at the current working directory, handing both across
//! explicitly. It searches PATH for jj and for git; the first crossing — asking jj for the
//! root — runs with root = cwd, because no root is known yet. Nothing inside the boundary
//! (M-jj) searches PATH; the search lives here. The store is opened on that same resolved
//! root (S2), so the substrate's handles — the root, the store, the runner, and the git
//! binary — are resolved once and handed across as one context. A directory with no
//! workspace has no root to resolve, so the context carries none (S3-root-optional): the
//! root query running and reporting none is not a failure, only a binary that cannot be
//! found or a query that cannot spawn is. The root and the store are reached through
//! fallible accessors that refuse when the context is root-less (S3-root-required), naming
//! the absence — the value M-refusal renders — while the runner is always present and git is
//! reached through its own fallible accessor.

use std::env;
use std::path::{Path, PathBuf};

use crate::jj::{Jj, JjError};
use crate::refusal::Refusal;
use crate::store::Store;

/// Why context could not be resolved. Evidence-carrying, never a refusal (S3-err).
#[derive(Debug)]
pub enum ContextError {
    /// No `jj` executable on PATH.
    BinaryNotFound(String),
    /// The root query could not run at all.
    Spawn(String),
}

/// The substrate's resolved handles: the repo root and the store on it when a workspace
/// resolves, the jj runner, and the git binary.
pub struct Context {
    root: Option<PathBuf>,
    store: Option<Store>,
    jj: Jj,
    git: Option<PathBuf>,
}

impl Context {
    /// The repo root, or a refusal naming the absent workspace (S3-root-required).
    pub fn root(&self) -> Result<&Path, Refusal> {
        self.root.as_deref().ok_or_else(no_workspace)
    }

    /// The store, opened on the resolved root (S2), or a refusal naming the absent workspace
    /// (S3-root-required).
    pub fn store(&self) -> Result<&Store, Refusal> {
        self.store.as_ref().ok_or_else(no_workspace)
    }

    /// The jj runner, anchored to the resolved root, or to the working directory when none
    /// resolved (S3-caller). Always present.
    pub fn jj(&self) -> &Jj {
        &self.jj
    }

    /// The git binary, or a refusal naming the absent binary.
    pub fn git(&self) -> Result<&Path, Refusal> {
        self.git.as_deref().ok_or_else(no_git)
    }
}

/// The one refusal a root-less context hands a workspace-needing verb (S3-root-required).
fn no_workspace() -> Refusal {
    Refusal::new("no workspace")
}

/// The one refusal a context with no git binary hands a caller that needs it.
fn no_git() -> Refusal {
    Refusal::new("no git binary")
}

/// Resolve jj on PATH and the workspace root at `cwd`, open the store on that root, and hand
/// the substrate's handles across (S3-caller, S3-root-optional).
///
/// `cwd` is passed explicitly so a test can point at a fixture repo; assembly passes the
/// process working directory. A directory with no workspace resolves root-less: the root
/// query running and reporting none is not a failure, so only a missing binary or a query
/// that cannot spawn is an `Err`.
pub fn resolve(cwd: &Path) -> Result<Context, ContextError> {
    let binary = find_on_path("jj").ok_or_else(|| ContextError::BinaryNotFound("jj".to_string()))?;
    let git = find_on_path("git");
    let probe = Jj::new(binary.as_path(), cwd);
    let output = probe
        .run(&["workspace", "root"])
        .map_err(|error| spawn_error(&error))?;
    let root = match output.status {
        0 => {
            let printed = output.stdout.trim();
            (!printed.is_empty()).then(|| PathBuf::from(printed))
        }
        _ => None,
    };
    let store = root.clone().map(Store::new);
    let anchored = root.clone().unwrap_or_else(|| cwd.to_path_buf());
    Ok(Context {
        root,
        store,
        jj: Jj::new(binary.as_path(), anchored),
        git,
    })
}

/// The first `jj`/`git` on PATH (S3-caller).
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn spawn_error(error: &JjError) -> ContextError {
    match error {
        JjError::Spawn(reason) => ContextError::Spawn(reason.clone()),
    }
}
