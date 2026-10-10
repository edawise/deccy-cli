//! Restore: restore a whole deccy operation.
//!
//! S3-restore fixes the write half — restore a whole deccy operation via `op restore` given the
//! boundary's operation id; M-jj fixes the crossing and its evidence-carrying runner. This
//! names the operation and crosses; which boundary to restore is the undo/redo layer's
//! concern.

use crate::jj::{Jj, JjError, Output};

/// Restore the repo to the operation `id` (S3-restore).
///
/// The operation id is given; only whole deccy operations are restored.
pub fn restore(jj: &Jj, id: &str) -> Result<Output, JjError> {
    jj.run(&["op", "restore", id])
}
