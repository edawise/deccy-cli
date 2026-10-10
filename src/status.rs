//! Status: read the working-copy changed-path set.
//!
//! S3-status fixes the source and its width — the working-copy changed-path set, paths only,
//! for recording's attribution; M-jj fixes the crossing and its evidence-carrying runner.
//! `jj diff --summary` is the machine-readable form of that set, so this returns the paths
//! and nothing wider; deciding an edit's ownership is the record verb's concern.

use crate::jj::{Jj, JjError, Output};

/// Why the changed-path set could not be read (S3-err: the evidence is carried).
#[derive(Debug)]
pub enum StatusError {
    /// The crossing itself could not run.
    Spawn(String),
    /// jj ran but failed; its captured output is the evidence.
    Failed(Output),
}

impl From<JjError> for StatusError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => StatusError::Spawn(reason),
        }
    }
}

/// The working-copy changed paths (S3-status).
pub fn status(jj: &Jj) -> Result<Vec<String>, StatusError> {
    let out = jj.run(&["diff", "--summary", "-r", "@"])?;
    if out.status != 0 {
        return Err(StatusError::Failed(out));
    }
    Ok(paths(&out.stdout))
}

/// The paths in `jj diff --summary` output; each entry line is `<code> <path>`, and a rename
/// names **two** paths on one line.
fn paths(stdout: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in stdout.lines() {
        let Some((_code, field)) = line.split_once(' ') else {
            continue;
        };
        match rename(field) {
            Some((old, new)) => {
                paths.push(old);
                paths.push(new);
            }
            None => paths.push(field.to_string()),
        }
    }
    paths
}

/// The two paths of a rename's field, or `None` when the field names a single path.
///
/// jj writes a rename as `R <prefix>{<old> => <new>}<suffix>`, where the prefix and suffix are
/// the parts the two paths share — empty for unrelated paths (a plain file move), and the moved
/// directory for a whole-tree move (`{src/v1 => src/v2}/lib.rs`). The
/// brace-less `R <old> => <new>` is the same shape with an empty prefix and suffix.
fn rename(field: &str) -> Option<(String, String)> {
    if let (Some(open), Some(close)) = (field.find('{'), field.rfind('}')) {
        if open < close {
            let prefix = &field[..open];
            let suffix = &field[close + 1..];
            if let Some((old, new)) = field[open + 1..close].split_once(" => ") {
                return Some((
                    format!("{prefix}{old}{suffix}"),
                    format!("{prefix}{new}{suffix}"),
                ));
            }
        }
    }
    field
        .split_once(" => ")
        .map(|(old, new)| (old.to_string(), new.to_string()))
}
