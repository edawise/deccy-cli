//! Node addressing: parse an id, then derive its document path.
//!
//! S2-id fixes the boundary — only a validated id derives a path, so an unparseable id never
//! becomes a filesystem probe. S2-path fixes the layout — a node document is `nodes/<id>.json`,
//! derived from the id alone, with no index or second layout. This parses and derives; it
//! neither reads nor validates the document, so it needs neither the runner nor the read, and
//! it is the one derivation both the read and the write call.

use std::path::PathBuf;

/// The directory that holds node documents, relative to the store root (S2-path).
pub const NODE_DIR: &str = "nodes";

/// A node id: the jj change id that names the change owning the node (D1b).
///
/// Constructed only by `parse`, so a string that is not a well-formed id can never be
/// used to derive a path (S2-id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Id(String);

impl Id {
    /// Parse a node id: lowercase ASCII letters and digits, at least eight of them — the
    /// graph's id grammar, the same the schema uses for edge targets.
    pub fn parse(text: &str) -> Option<Id> {
        let well_formed = text.len() >= 8
            && text
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        well_formed.then(|| Id(text.to_string()))
    }

    /// The id as it appears in filenames and edge targets.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The node document's path, derived from the id alone: `nodes/<id>.json` (S2-path).
pub fn node_path(id: &Id) -> PathBuf {
    PathBuf::from(NODE_DIR).join(format!("{}.json", id.as_str()))
}
