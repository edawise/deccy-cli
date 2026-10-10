//! Revert: take a named op-log entry's work out of the log.
//!
//! S3-revert (kttmwrsv) fixes the write half — the selective revert of a named operation's
//! entries, whole operations only, no partial, file-level or snapshot revert; M-jj
//! (zruxtxrv) fixes the crossing and its evidence-carrying runner. This names the entries
//! and crosses; which operation's entries those are, and in what order, is the undo/redo
//! layer's concern.

use crate::jj::{Jj, JjError, Output};

/// Revert the named op-log entries, in the order given (S3-revert).
///
/// Only whole deccy operations are reverted, and the crossing is merge-based, so concurrent
/// work inside the window survives. Each crossing's outcome is returned; a crossing that
/// cannot spawn is a failure value of the runner's.
pub fn revert(jj: &Jj, entries: &[String]) -> Result<Vec<Output>, JjError> {
    let mut outcomes = Vec::with_capacity(entries.len());
    for entry in entries {
        outcomes.push(jj.run(&["op", "revert", entry])?);
    }
    Ok(outcomes)
}
