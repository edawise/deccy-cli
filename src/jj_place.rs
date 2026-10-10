//! Placement: move working-copy edits into an older change.
//!
//! S3-place fixes the mechanism — working-copy edits move into an older change via
//! `jj squash --into <id>`, the working copy as the source; M-jj fixes the crossing and its
//! evidence-carrying runner. This names the target and the paths to move, and crosses; which edit
//! belongs in which change (roqpxulw's attribution) is decided elsewhere, and interpreting the
//! outcome is not this layer's concern.

use crate::jj::{Jj, JjError, Output};

/// Move the working-copy edits into the change `into` (S3-place).
///
/// The working copy is the source; the target is named, and the pathset narrows that source to the
/// named files.
pub fn place(jj: &Jj, into: &str, paths: &[String]) -> Result<Output, JjError> {
    let mut args = vec!["squash", "--into", into, "--"];
    args.extend(paths.iter().map(String::as_str));
    jj.run(&args)
}
