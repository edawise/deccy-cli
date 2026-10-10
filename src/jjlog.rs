//! Log: list changes with their descriptions.
//!
//! S3-log fixes the source — changes and their descriptions come from `jj log` (metadata),
//! not the file store; M-jj fixes the crossing and its evidence-carrying runner. This maps
//! the command to a machine-readable form and parses it into `(change id, description)`
//! pairs, so the verb reads a listing rather than jj's output format.

use crate::jj::{Jj, JjError, Output};

/// One line per visible change: `<short-change-id><TAB><description-first-line>`.
const ENTRY: &str = "change_id.shortest(8) ++ \"\\t\" ++ description.first_line() ++ \"\\n\"";

/// Why a listing could not be produced (S3-err: the evidence is carried, never collapsed).
#[derive(Debug)]
pub enum LogError {
    /// The crossing itself could not run.
    Spawn(String),
    /// jj ran but failed; its captured output is the evidence.
    Failed(Output),
}

impl From<JjError> for LogError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => LogError::Spawn(reason),
        }
    }
}

/// List the visible changes with their descriptions (S3-log).
///
/// Each entry is `(short change id, description's first line)`.
pub fn log(jj: &Jj) -> Result<Vec<(String, String)>, LogError> {
    let out = jj.run(&["log", "--no-graph", "-T", ENTRY])?;
    if out.status != 0 {
        return Err(LogError::Failed(out));
    }
    Ok(entries(&out.stdout))
}

/// Split the fixed listing into `(id, description)` pairs.
fn entries(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(id, description)| (id.to_string(), description.to_string()))
        .collect()
}
