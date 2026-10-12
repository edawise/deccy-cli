//! The ordered bootstrap: create the git repository, record the state directory, colocate.
//!
//! IN-mechanism (lmomkluy) fixes the sequence — create the git repository, append the state
//! directory to `.git/info/exclude`, then create the jj workspace colocated with it — and
//! this is that sequence as one module. The order is load-bearing: `.git` must exist before
//! its `info/exclude` can be recorded, and the workspace must adopt that repository rather
//! than make its own. The state directory is recorded once, so git's own template is recorded
//! without being clobbered and a re-run adds nothing. Both crossings run through the one
//! runner (M-jj, zruxtxrv) under caller-supplied binaries and root (S3-explicit, vyquqmoo),
//! and `.git/info/exclude` is read and appended through the exclude module (M-exclude,
//! nrvtvmln), never `std::fs`. M-layout (szmuyqrq) places the module flat at
//! src/jj_init.rs. It returns failures as values and renders nothing.

use crate::exclude;
use crate::jj::{Jj, JjError, Output};
use crate::store::Store;

/// The state directory's entry: appended to git's own exclude template (IN-ignore, xxwtnpzz).
const STATE_DIR: &str = ".deccy/";

/// Why the bootstrap could not complete: the reason is carried, never rendered (S3-err).
#[derive(Debug)]
pub enum BootstrapError {
    /// A crossing could not be spawned (binary absent, not executable, …).
    Spawn(String),
    /// A crossing ran and exited non-zero.
    Failed(Output),
    /// The exclude file could not be read or written.
    Exclude(exclude::ExcludeError),
}

/// Run the ordered bootstrap on the repository rooted at the runners' root (IN-mechanism).
///
/// `git` and `jj` are the two caller-supplied crossings, both anchored at the target; `store`
/// reads and appends `.git/info/exclude` under the same root. The git repository is created
/// first, the state directory is recorded once in its exclude file, and only then is the jj
/// workspace created colocated with it. Nothing is rendered; every failure is a value.
pub fn bootstrap(git: &Jj, jj: &Jj, store: &Store) -> Result<(), BootstrapError> {
    let created = git.run(&["init"]).map_err(spawn)?;
    if created.status != 0 {
        return Err(BootstrapError::Failed(created));
    }

    let contents = exclude::read(store).map_err(BootstrapError::Exclude)?;
    if !contents.lines().any(|line| line == STATE_DIR) {
        exclude::append(store, STATE_DIR).map_err(BootstrapError::Exclude)?;
    }

    let workspace = jj.run(&["git", "init", "--colocate"]).map_err(spawn)?;
    if workspace.status != 0 {
        return Err(BootstrapError::Failed(workspace));
    }
    Ok(())
}

fn spawn(error: JjError) -> BootstrapError {
    match error {
        JjError::Spawn(reason) => BootstrapError::Spawn(reason),
    }
}
