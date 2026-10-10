//! Op log: read the current operation id.
//!
//! S3-oplog fixes the read — the current operation id from the jj op log, feeding markers'
//! read-run-read bracketing; M-jj fixes the crossing and its evidence-carrying runner. `jj op
//! log` carries a template, so this fixes the machine-readable form and returns the head
//! operation id; bracketing around an operation is the markers layer's concern.

use crate::jj::{Jj, JjError, Output};

/// Why the current operation id could not be read (S3-err: the evidence is carried).
#[derive(Debug)]
pub enum OplogError {
    /// The crossing itself could not run.
    Spawn(String),
    /// jj ran but failed; its captured output is the evidence.
    Failed(Output),
}

impl From<JjError> for OplogError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => OplogError::Spawn(reason),
        }
    }
}

/// The current (head) operation id (S3-oplog).
pub fn oplog(jj: &Jj) -> Result<String, OplogError> {
    let out = jj.run(&["op", "log", "--no-graph", "--limit", "1", "-T", "id"])?;
    if out.status != 0 {
        return Err(OplogError::Failed(out));
    }
    Ok(out.stdout.trim().to_string())
}
