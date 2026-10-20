//! Divergence: read whether any change carries more than one commit.
//!
//! S3-divergence (zplpmpvo) fixes the verdict — the commits the revset `divergent()` yields,
//! over a spawned jj process, with the working copy left alone; M-layout places the module flat
//! at src/divergence.rs. It crosses `jj log --ignore-working-copy --no-graph -r divergent() -T
//! commit_id`, so the read never snapshots the state it is judging (S5-observation, urqytyqn),
//! and carries failures as values with their evidence (S3-err, uzzxlpsl). Taking the verdict and
//! rolling back is the undo and redo verbs' concern (S5-revert-divergent, ppytsptl). This module
//! renders nothing.

use crate::jj::{Jj, JjError, Output};

/// Why the divergence verdict could not be read (S3-err: the evidence is carried).
#[derive(Debug)]
pub enum DivergenceError {
    /// The crossing itself could not run.
    Spawn(String),
    /// jj ran but failed; its captured output is the evidence.
    Failed(Output),
}

impl From<JjError> for DivergenceError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => DivergenceError::Spawn(reason),
        }
    }
}

/// Whether any change carries more than one commit (S3-divergence).
///
/// `divergent()` is jj's own verdict on a change with more than one version. A revert can
/// rewrite a commit below the working copy as readily as the working copy itself, so the read
/// is not scoped to `@`: any divergent change means the revert split the graph. The read
/// observes: `--ignore-working-copy` keeps it from snapshotting the state it is judging. A
/// non-empty answer is the verdict; a failed crossing is a value carrying its evidence.
pub fn divergent(jj: &Jj) -> Result<bool, DivergenceError> {
    let out = jj.run(&[
        "log",
        "--ignore-working-copy",
        "--no-graph",
        "-r",
        "divergent()",
        "-T",
        "commit_id",
    ])?;
    if out.status != 0 {
        return Err(DivergenceError::Failed(out));
    }
    Ok(!out.stdout.trim().is_empty())
}
