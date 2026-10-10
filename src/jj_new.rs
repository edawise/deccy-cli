//! Creation: insert a change after the newest named parent, with its summary.
//!
//! S3-new fixes the mechanism — a change is opened at a named position change via
//! `jj new -A <position> -m <summary>`, its position and summary given, never defaulted.
//! S3-new-position (N1) fixes which position: the dependency furthest up in jj history, the
//! unique head of the given set; S3-new-position refusal (N2) fixes that a set with more than
//! one head refuses rather than forking. M-jj fixes the crossing and its evidence-carrying
//! runner. This layer reads the set's head from jj, builds the call, and crosses; interpreting
//! the outcome — extracting the new change id — is the create operation's concern, not this
//! layer's.

use crate::jj::{Jj, JjError, Output};

/// Why a change could not be opened: the evidence is carried, not collapsed (S3-err).
#[derive(Debug)]
pub enum OpenError {
    /// The process could not be spawned (binary absent, not executable, …).
    Spawn(String),
    /// The given dependency set has more than one head in jj history, so no position is
    /// unique (N2); the tied ids are carried.
    NonLinear(Vec<String>),
}

/// The template each head id is read with: the 8-character change-id prefix.
const HEAD_TEMPLATE: &str = "change_id.shortest(8) ++ \"\\n\"";

/// Insert a change after the newest of `parents`, described by `summary` (S3-new, N1).
///
/// The position is the unique head of `parents` in jj history, read from jj; the other
/// dependencies become its ancestors. A set with more than one head refuses (N2). The position
/// is never defaulted to the working copy.
pub fn open(jj: &Jj, summary: &str, parents: &[&str]) -> Result<Output, OpenError> {
    let heads = jj
        .run(&[
            "log",
            "--no-graph",
            "-r",
            &format!("heads({})", parents.join("|")),
            "-T",
            HEAD_TEMPLATE,
        ])
        .map_err(spawn)?;
    if heads.status != 0 {
        // The read could not name a position; its own outcome is the crossing's result.
        return Ok(heads);
    }
    let ids = head_ids(&heads.stdout);
    let position = match ids.as_slice() {
        [only] => only.clone(),
        _ => return Err(OpenError::NonLinear(ids)),
    };
    jj.run(&["new", "-A", &position, "-m", summary]).map_err(spawn)
}

/// The head ids jj reported, one per non-empty line (N1).
fn head_ids(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// A crossing that could not spawn jj: the reason is carried (S3-err).
fn spawn(error: JjError) -> OpenError {
    match error {
        JjError::Spawn(reason) => OpenError::Spawn(reason),
    }
}
