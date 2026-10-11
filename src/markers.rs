//! Marker state types: the Rust face of the marker schema.
//!
//! S5-marker-schema (zwurlynr) fixes the file's contents — markers, u, b, and at, closed — and
//! T-state (owolyrmp) the four variables with empty/unset/zero/zero init; these types are that
//! file, so the JSON mapping lives here. G-marker (kkqzyxly) and S5-ids-only (okqmlwmp) fix
//! that a marker is the kept pre/post pair of operation ids and nothing else. D8b (txqmnvup)
//! fixes where those ids come from: the pre- and post-operation operation ids over the op log.
//! M-layout (szmuyqrq) places the module flat at src/markers.rs.
//!
//! What this fixes: the pair, the ordered list, the undone count, the redo bound, and the
//! restore-target position, with the JSON mapping that makes the types the file's contents —
//! closed objects and hex-string ids, with "0" the position's unset sentinel. Reading and
//! writing the file is M-markers-persist's; the stack and window guards are M-markers-guards'.

use serde::{Deserialize, Serialize};

/// The position's unset sentinel: the model's position 0, realized as the hex string "0"
/// (T-state, S5-marker-schema). A real position is an op-log id, never this.
pub const UNSET: &str = "0";

/// One marker: the kept pre- and post-boundary pair of operation ids (G-marker, S5-ids-only,
/// D8b). Closed — ids alone, no graph state attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    /// The pre-operation operation id captured at the pair's start (D8b).
    pub pre: String,
    /// The post-operation operation id captured at the pair's close (D8b).
    pub post: String,
}

/// The marker file's contents: the ordered pair list, the undone count, the redo bound, and the
/// restore-target position (S5-marker-schema, T-state). Closed — these fields alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Markers {
    /// Bracketed pairs in run order (S5-marker-schema).
    pub markers: Vec<Marker>,
    /// 0 means nothing undone, else the number of operations undone (S5-marker-schema).
    pub u: u64,
    /// 0 at init, else the redo bound — the pair count when the undone count was last set
    /// from nothing-undone (T-state, S5-marker-schema).
    pub b: u64,
    /// Current position; `UNSET` is the unset sentinel (S5-marker-schema).
    pub at: String,
}

impl Markers {
    /// The init value: no pairs, nothing undone, the unset position (T-state).
    pub fn new() -> Self {
        Markers {
            markers: Vec::new(),
            u: 0,
            b: 0,
            at: UNSET.to_string(),
        }
    }
}
