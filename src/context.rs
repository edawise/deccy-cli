//! The caller side of the seam: find jj and git, find the repo, open the store, hand all across.
//!
//! S3-explicit fixes that every crossing runs under a caller-supplied binary and root;
//! S3-caller fixes that the seam's caller resolves the jj binary via PATH once and the repo
//! root via `jj workspace root` at the current working directory, handing both across
//! explicitly. It searches PATH for jj and for git; the first crossing — asking jj for the
//! root — runs with root = cwd, because no root is known yet. Nothing inside the boundary
//! (M-jj) searches PATH; the search lives here.
//!
//! Two roots, not one (M-TWO-ROOTS, C-TWO-ROOTS): the invocation workspace root is what
//! `root()` lends and what the runner is anchored on, while the store is opened on the
//! **trunk state root** the resolver derives from that workspace's repository entry
//! (M-state-root, lotlstzr). The substrate's handles — the invocation root, the store on the
//! trunk root, the runner, and the git binary — are resolved once and handed across as one
//! context. A directory with no workspace has no root to resolve, so the context carries none
//! (S3-root-optional): the root query running and reporting none is not a failure. A
//! workspace whose repository entry exists but cannot be resolved is a failure instead
//! (M-TWO-ROOTS): `resolve` carries the resolver's typed cause rather than falling back to
//! the invocation root. The accessors split accordingly: `state()` lends the store in any
//! workspace, `store()` lends it only from the trunk working copy and otherwise refuses,
//! naming the workspace it ran in, and a root-less context refuses both with the absent
//! workspace — the value M-refusal renders — while the runner is always present and git is
//! reached through its own fallible accessor.

use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::jj::{Jj, JjError};
use crate::refusal::Refusal;
use crate::state_root;
use crate::store::Store;

/// Why context could not be resolved. Evidence-carrying, never a refusal (S3-err).
#[derive(Debug)]
pub enum ContextError {
    /// No `jj` executable on PATH.
    BinaryNotFound(String),
    /// The root query could not run at all.
    Spawn(String),
    /// The invocation workspace's repository entry could not be resolved into a trunk state
    /// root (M-TWO-ROOTS): the typed cause travels with the failure and resolution stops
    /// rather than falling back to the invocation root.
    StateRoot(state_root::StateRootError),
    /// The invocation directory holds a workspace, but jj would not report its root.
    RootQuery(String),
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContextError::BinaryNotFound(name) => write!(f, "no {name} executable on PATH"),
            ContextError::Spawn(reason) => write!(f, "the jj root query could not run: {reason}"),
            ContextError::StateRoot(error) => {
                write!(f, "the trunk state root could not be resolved: {error}")
            }
            ContextError::RootQuery(reason) => {
                write!(f, "the workspace root could not be resolved: {reason}")
            }
        }
    }
}

impl std::error::Error for ContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ContextError::StateRoot(error) => Some(error),
            _ => None,
        }
    }
}

/// The substrate's resolved handles: the invocation workspace root and the store on the trunk
/// state root when a workspace resolves, the jj runner, and the git binary.
pub struct Context {
    root: Option<PathBuf>,
    store: Option<Store>,
    jj: Jj,
    git: Option<PathBuf>,
}

impl Context {
    /// The invocation workspace root, or a refusal naming the absent workspace
    /// (S3-root-required).
    pub fn root(&self) -> Result<&Path, Refusal> {
        self.root.as_deref().ok_or_else(no_workspace)
    }

    /// The store, opened on the trunk state root (M-TWO-ROOTS), in any workspace — the trunk
    /// working copy or a linked one — or a refusal naming the absent workspace
    /// (S3-root-required).
    pub fn state(&self) -> Result<&Store, Refusal> {
        self.store.as_ref().ok_or_else(no_workspace)
    }

    /// The store, opened on the trunk state root, handed only from the trunk working copy
    /// (M-TWO-ROOTS). Outside it the store belongs to another workspace, so this refuses,
    /// naming the workspace the invocation ran in; a root-less context refuses with the
    /// absent workspace (S3-root-required).
    pub fn store(&self) -> Result<&Store, Refusal> {
        let store = self.store.as_ref().ok_or_else(no_workspace)?;
        let root = self.root.as_deref().ok_or_else(no_workspace)?;
        if is_same_workspace(root, store.root()) {
            return Ok(store);
        }
        Err(
            Refusal::new("the store is rooted at the trunk, not at this workspace")
                .with_evidence(root.to_string_lossy().into_owned()),
        )
    }

    /// The jj runner, anchored to the invocation workspace root, or to the working directory
    /// when none resolved (S3-caller). Always present.
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

/// Whether two roots name the same working copy. The store's root is resolved by the trunk
/// resolver; the invocation root is whatever jj printed, so compare them resolved.
fn is_same_workspace(invocation: &Path, store_root: &Path) -> bool {
    match fs::canonicalize(invocation) {
        Ok(resolved) => resolved == store_root,
        Err(_) => invocation == store_root,
    }
}

/// Resolve jj on PATH and the workspace root at `cwd`, open the store on the trunk state root,
/// and hand the substrate's handles across (S3-caller, S3-root-optional, M-TWO-ROOTS).
///
/// `cwd` is passed explicitly so a test can point at a fixture repo; assembly passes the
/// process working directory. A directory with no workspace resolves root-less: the root
/// query running and reporting none is not a failure, so only a missing binary, a query that
/// cannot spawn, or a workspace whose repository entry cannot be resolved is an `Err`.
pub fn resolve(cwd: &Path) -> Result<Context, ContextError> {
    let binary = find_on_path("jj").ok_or_else(|| ContextError::BinaryNotFound("jj".to_string()))?;
    let git = find_on_path("git");
    let probe = Jj::new(binary.as_path(), cwd);
    let output = probe
        .run(&["workspace", "root"])
        .map_err(|error| spawn_error(&error))?;
    let printed = output.stdout.trim();
    let root = match output.status {
        0 if !printed.is_empty() => Some(PathBuf::from(printed)),
        // jj answered with nothing: no workspace to resolve.
        0 => None,
        // The query failed. A directory with no workspace at all stays root-less
        // (S3-root-optional); a directory that holds a workspace entry names the cause
        // instead of quietly becoming root-less (M-TWO-ROOTS).
        _ => {
            if !cwd.join(".jj").exists() {
                None
            } else {
                match state_root::trunk_root(cwd) {
                    Err(error) => return Err(ContextError::StateRoot(error)),
                    Ok(_) => {
                        return Err(ContextError::RootQuery(output.stderr.trim().to_string()));
                    }
                }
            }
        }
    };
    // The store is opened on the trunk state root the resolver derives from the invocation
    // workspace's repository entry — never on the invocation root by presumption.
    let store = match root.as_deref() {
        Some(root) => Some(Store::new(
            state_root::trunk_root(root).map_err(ContextError::StateRoot)?,
        )),
        None => None,
    };
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
