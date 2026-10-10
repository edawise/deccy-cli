//! The jj seam: spawn jj, capture what it said, interpret nothing.
//!
//! S3 fixes that every external-tool interaction crosses a spawned process — no library
//! linkage;
//! S3-explicit that the binary and root are supplied by the caller with nothing searching
//! ambient PATH inside this boundary; S3-stateless that this layer holds the binary and root
//! and nothing else; S3-err that failures surface as values carrying their evidence, never as
//! refusals or panics. The runner names no command: the per-command nodes do.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// What one byte-preserving crossing produced: the exit status, raw stdout bytes, and stderr
/// verbatim (S3-err). Used where a stream carries file content rather than text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputBytes {
    /// The child's exit status, or -1 if it was killed by a signal.
    pub status: i32,
    /// Everything the child wrote to stdout, byte for byte.
    pub stdout: Vec<u8>,
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

    /// Run one crossing that carries an input stream: the caller's bytes are written to the
    /// child's stdin before the crossing completes, and the status, raw stdout bytes and stderr
    /// are captured as always (S3-stdin, S3-err).
    pub fn run_with_input(&self, args: &[&str], input: &[u8]) -> Result<OutputBytes, JjError> {
        let mut child = Command::new(&self.binary)
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| JjError::Spawn(error.to_string()))?;
        let mut stdin = child.stdin.take().expect("stdin is piped");
        let input = input.to_vec();
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
        let output = child
            .wait_with_output()
            .map_err(|error| JjError::Spawn(error.to_string()))?;
        let _ = writer.join();
        Ok(OutputBytes {
            status: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Run one crossing whose stdout is content rather than text: status, raw stdout bytes and
    /// stderr captured (S3-err). The bytes are not decoded, so a binary file survives.
    pub fn run_bytes(&self, args: &[&str]) -> Result<OutputBytes, JjError> {
        let output = Command::new(&self.binary)
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|error| JjError::Spawn(error.to_string()))?;
        Ok(OutputBytes {
            status: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
