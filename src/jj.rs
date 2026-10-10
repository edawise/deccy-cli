//! The jj seam: spawn jj, capture what it said, interpret nothing.
//!
//! S3 fixes that every external-tool interaction crosses a spawned process — no library
//! linkage;
//! S3-explicit that the binary and root are supplied by the caller with nothing searching
//! ambient PATH inside this boundary; S3-stateless that this layer holds the binary and root
//! and nothing else; S3-err that failures surface as values carrying their evidence, never as
//! refusals or panics. The runner names no command: the per-command nodes do.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What one crossing produced: the exit status and both streams, verbatim (S3-err).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The child's exit status, or -1 if it was killed by a signal.
    pub status: i32,
    /// Everything the child wrote to stdout.
    pub stdout: String,
    /// Everything the child wrote to stderr.
    pub stderr: String,
}

/// Why a crossing could not run at all: the evidence is carried, not collapsed (S3-err).
#[derive(Debug)]
pub enum JjError {
    /// The process could not be spawned (binary absent, not executable, …).
    Spawn(String),
}

/// One external-tool crossing over one caller-supplied binary and root (S3-explicit, S3-stateless).
pub struct Jj {
    binary: PathBuf,
    root: PathBuf,
}

impl Jj {
    /// Open a runner on an explicit binary and root; nothing is searched (S3-explicit).
    pub fn new(binary: impl Into<PathBuf>, root: impl Into<PathBuf>) -> Self {
        Jj {
            binary: binary.into(),
            root: root.into(),
        }
    }

    /// The binary this runner crosses with.
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// The root this runner crosses in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Run one crossing: `<binary> <args>` in the root, status and both streams captured.
    pub fn run(&self, args: &[&str]) -> Result<Output, JjError> {
        let output = Command::new(&self.binary)
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|error| JjError::Spawn(error.to_string()))?;
        Ok(Output {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
