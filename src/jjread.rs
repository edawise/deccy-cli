//! Read a file's contents at a named revision via `jj file show`.
//!
//! S3-read (qrwtzxpn) fixes the command — a file's contents at a named revision via
//! `jj file show`; M-jj (zruxtxrv) fixes the crossing and its evidence-carrying runner. This
//! maps the command to a machine-readable form and returns the file's bytes at that revision,
//! so the store's read reads a document rather than jj's output; a failed crossing is carried
//! as a value, never rendered here (S3-err, uzzxlpsl). Which revision and which path the read
//! names is S2-read's concern, not this layer's.

use crate::jj::{Jj, JjError, Output};

/// Why a file read could not be produced (S3-err: the evidence is carried, never collapsed).
#[derive(Debug)]
pub enum ReadError {
    /// The crossing itself could not run.
    Spawn(String),
    /// jj ran but failed; its captured output is the evidence.
    Failed(Output),
}

impl From<JjError> for ReadError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => ReadError::Spawn(reason),
        }
    }
}

/// Read `path`'s contents at `revision` via `jj file show` (S3-read).
pub fn read(jj: &Jj, path: &str, revision: &str) -> Result<String, ReadError> {
    let out = jj.run(&["file", "show", "-r", revision, path])?;
    if out.status != 0 {
        return Err(ReadError::Failed(out));
    }
    Ok(out.stdout)
}
