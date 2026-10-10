//! Removal: abandon a change.
//!
//! S3-abandon fixes the mechanism — a change is abandoned via `jj abandon`; M-jj fixes the
//! crossing and its evidence-carrying runner. This names the change and crosses; whether it
//! *may* be removed (no dependants) is decided elsewhere (myprwuxt), and interpreting the
//! outcome is not this layer's concern.

use crate::jj::{Jj, JjError, Output};

/// Abandon the change `id` (S3-abandon).
///
/// The change is named; the default-to-working-copy form is never relied on.
pub fn abandon(jj: &Jj, id: &str) -> Result<Output, JjError> {
    jj.run(&["abandon", id])
}
