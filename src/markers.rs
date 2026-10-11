//! Marker state types: the Rust face of the marker schema.
//!
//! S5-marker-schema (zwurlynr) fixes the file's contents — markers and pos, closed — and
//! T-state (owolyrmp) those two variables with empty/zero init; these types are that
//! file, so the JSON mapping lives here. G-marker (kkzyxly) and S5-ids-only (okqmlwmp) fix
//! that a marker is one operation's own record — the kind it was, the change it targeted,
//! the entry ids it kept and the reverts recorded against it — and nothing else. D8b
//! (txqmnvup) fixes where those entries come from: the entries within an operation's run
//! window, attributed to the invoking workspace. M-layout (szmuyqrq) places the module flat
//! at src/markers.rs.
//!
//! What this fixes: the marker list and the position, with the JSON mapping that makes the
//! types the file's contents — closed objects and hex-string ids. The linear history
//! (kpptprql) fixes the position: markers before it are applied, markers from it onward
//! are the undone tail. Reading and writing the file is M-markers-persist's (wzyorrny);
//! the cursor guards are M-markers-guards' (lqtqzznn).

use serde::{Deserialize, Serialize};

/// The kind of operation a marker records (G-operation, lmwpuqlx): a creation, a
/// recording, or a removal. Undo takes each operation's own inverse, so the kind is what
/// it dispatches on (S5-ids-only, okqmlwmp; D8a, prsoxsuo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Create,
    Record,
    Remove,
}

/// One marker: one operation's own record (G-marker, S5-ids-only, D8b). Closed — the
/// kind, the targeted change id and the ids alone, no graph state attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    /// The operation's kind (G-operation).
    pub kind: Kind,
    /// The change the operation acted on: the created change, the recorded-into node or the
    /// removed change. A marker always names one: the schema refuses an empty target
    /// (zwurlynr).
    pub target: String,
    /// The operation's kept entry ids, in run order (D8b).
    pub entries: Vec<String>,
    /// The reverts undo recorded against the operation while it is undone (S5-undo).
    pub reverts: Vec<String>,
}

/// The marker file's contents: the marker list and the position (S5-marker-schema,
/// T-state). Closed — these fields alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Markers {
    /// Markers in run order (S5-marker-schema).
    pub markers: Vec<Marker>,
    /// How many markers are applied: markers before it are applied, markers from it onward
    /// are the undone tail (T-state, S5-marker-schema).
    pub pos: u64,
}

impl Markers {
    /// The init value: no markers, the position at zero (T-state).
    pub fn new() -> Self {
        Markers {
            markers: Vec::new(),
            pos: 0,
        }
    }
}
