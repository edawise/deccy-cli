//! The trunk state root, resolved from the invocation workspace's repository entry.
//!
//! C-TWO-ROOTS (lotlstzr) roots the store at the trunk, not at the working copy the
//! invocation runs in; EARS lssslxwo states that same rule and adds that an unresolvable
//! trunk refuses, naming the cause, instead of falling back to the invocation root; and
//! T-TWO-ROOTS (wxsynqwm) models the resolution as `resolve` over `Own`, `Leads`,
//! `Missing`, `Unreadable` and `Dangling`. This module is `resolve` on real workspaces:
//! it reads the invocation workspace's repository entry and yields the root of the working
//! copy that entry belongs to — the invocation's own root, or another working copy's — or
//! a typed refusal. It never yields the invocation root for an unresolved entry, and it
//! does not touch the jj runner: the runner stays anchored on the invocation workspace, so
//! only the store's root is answered here.
//!
//! A jj workspace spells its repository entry at `.jj/repo`, in one of two forms:
//!
//! - a **directory** — the workspace holds its own repository, so the workspace is the
//!   working copy the repository belongs to and its root is the answer;
//! - a **file** — the entry names another working copy's repository entry. The name is
//!   either absolute or relative; a relative name is anchored at the directory containing
//!   the entry, the workspace's `.jj` directory, which is the form `jj workspace add`
//!   writes (`../../<trunk>/.jj/repo`). The answer is the root of the working copy that
//!   holds the named entry, normalized so no `..` component survives.
//!
//! The entry's representation is all this module settles. It is std-only (no `crate::`
//! imports) so the gate compiles it standalone through a `#[path]` shim.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The repository entry's name inside a workspace's `.jj` directory.
const ENTRY_NAME: &str = "repo";

/// The typed refusal: the trunk root could not be resolved from the invocation workspace.
///
/// Every variant carries the path it is about, so the caller can name the cause rather
/// than fall back to the invocation workspace root (EARS lssslxwo). `Display` names that
/// cause; the variants stay comparable by shape, so a caller chooses between them without
/// parsing the message.
#[derive(Debug)]
pub enum StateRootError {
    /// The invocation workspace has no repository entry at `.jj/repo`.
    MissingEntry { path: PathBuf },
    /// The repository entry exists but names no target.
    EmptyEntry { path: PathBuf },
    /// The repository entry, or the path it names, could not be read.
    UnreadableEntry { path: PathBuf, source: io::Error },
    /// The repository entry names a path that does not exist.
    DanglingEntry { path: PathBuf, target: PathBuf },
    /// The repository entry names a path that exists but is not a `.jj/repo` directory.
    NotARepoEntry { path: PathBuf, target: PathBuf },
}

impl fmt::Display for StateRootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateRootError::MissingEntry { path } => write!(
                f,
                "no repository entry at {}: the invocation workspace has no .jj/repo",
                path.display()
            ),
            StateRootError::EmptyEntry { path } => write!(
                f,
                "the repository entry at {} is empty and names no target",
                path.display()
            ),
            StateRootError::UnreadableEntry { path, source } => write!(
                f,
                "the repository entry at {} cannot be read: {}",
                path.display(),
                source
            ),
            StateRootError::DanglingEntry { path, target } => write!(
                f,
                "the repository entry at {} names {}, which does not exist",
                path.display(),
                target.display()
            ),
            StateRootError::NotARepoEntry { path, target } => write!(
                f,
                "the repository entry at {} leads to {}, which is not a .jj/repo directory",
                path.display(),
                target.display()
            ),
        }
    }
}

impl Error for StateRootError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StateRootError::UnreadableEntry { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Resolve the trunk state root from the invocation workspace's repository entry.
///
/// `workspace_root` is the root of the working copy the invocation runs in. The answer is
/// the root of the working copy the repository belongs to — that same workspace when its
/// `.jj/repo` is its own directory, and otherwise the workspace holding the entry the
/// invocation's `.jj/repo` file names, however that name is written. An entry that is
/// missing, empty, unreadable, dangling or not a repository entry is a typed refusal;
/// the invocation root is never returned in its place (EARS lssslxwo).
pub fn trunk_root(workspace_root: &Path) -> Result<PathBuf, StateRootError> {
    let entry = workspace_root.join(".jj").join(ENTRY_NAME);

    let metadata = match fs::metadata(&entry) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(StateRootError::MissingEntry { path: entry });
        }
        Err(source) => return Err(StateRootError::UnreadableEntry { path: entry, source }),
    };

    if metadata.is_dir() {
        // The workspace holds its own repository entry: it is the working copy the
        // repository belongs to, so its own root is the trunk root.
        return fs::canonicalize(workspace_root).map_err(|source| {
            StateRootError::UnreadableEntry {
                path: workspace_root.to_path_buf(),
                source,
            }
        });
    }

    if metadata.is_file() {
        return follow_entry(entry);
    }

    // Neither a directory nor a file: an entry of another kind is not a repository entry.
    Err(StateRootError::NotARepoEntry {
        path: entry.clone(),
        target: entry,
    })
}

/// Follow a `.jj/repo` **file** entry to the root of the working copy that holds the entry
/// it names.
///
/// The target is used as written when it is absolute. A relative target is joined onto the
/// directory that contains the entry — the workspace's `.jj` directory — never onto the
/// workspace root and never onto the process working directory, because that is the base
/// `jj workspace add` writes against.
fn follow_entry(entry: PathBuf) -> Result<PathBuf, StateRootError> {
    let named = fs::read_to_string(&entry).map_err(|source| StateRootError::UnreadableEntry {
        path: entry.clone(),
        source,
    })?;
    let named = named.trim();
    if named.is_empty() {
        return Err(StateRootError::EmptyEntry { path: entry });
    }

    let named = Path::new(named);
    let target = if named.is_absolute() {
        named.to_path_buf()
    } else {
        match entry.parent() {
            // The entry always lives at `<workspace>/.jj/repo`; anchoring on `.jj` is the
            // whole point of the relative form.
            Some(anchor) => anchor.join(named),
            None => named.to_path_buf(),
        }
    };

    // Resolution also normalizes: a canonical path has no `.` or `..` component left, so
    // the root derived from it is the working copy's root, not a path spelling of the walk.
    let resolved = match fs::canonicalize(&target) {
        Ok(resolved) => resolved,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(StateRootError::DanglingEntry {
                path: entry,
                target,
            });
        }
        Err(source) => return Err(StateRootError::UnreadableEntry { path: target, source }),
    };

    // The named path has to be a repository entry — a `.jj/repo` directory — for the
    // working copy that contains it to be the answer. A path that exists but is something
    // else is exactly the case that must refuse rather than name a root.
    let is_repository_entry = resolved.is_dir()
        && resolved
            .file_name()
            .map_or(false, |name| name == ENTRY_NAME)
        && resolved
            .parent()
            .and_then(Path::file_name)
            .map_or(false, |name| name == ".jj");
    if !is_repository_entry {
        return Err(StateRootError::NotARepoEntry {
            path: entry,
            target: resolved,
        });
    }

    // `<root>/.jj/repo` → `<root>`.
    match resolved.parent().and_then(Path::parent) {
        Some(root) => Ok(root.to_path_buf()),
        None => Err(StateRootError::NotARepoEntry {
            path: entry,
            target: resolved,
        }),
    }
}
