//! Appending a bracketed pair to the live marker file: read, push, advance, write.
//!
//! S5-marker-append (nptquxlz) fixes what closing a bracket does to the state — the
//! (pre, post) pair appended in run order, the position moved to the pair's
//! post-boundary, the undone count cleared and the bound left standing — and
//! S5-ordered (kupzywsq) that marker order is the order operations ran in; M-layout
//! (szmuyqrq) places the module flat at src/markers_append.rs. It names the state
//! through M-markers' (qrqumusy) closed types, crosses the store through M-root
//! (ppwnzwwy), and reads and writes the whole file through M-markers-persist's
//! (wzyorrny) load and save — never std::fs, never a truncate.
//!
//! What this fixes: one transition. A missing file reads as empty, so the first append is
//! the appended state (M-markers-persist). Every failure is M-markers-persist's carried
//! reason; this module renders nothing.

use crate::markers::Marker;
use crate::markers_persist::{self, PersistError};
use crate::store::Store;

/// Append a bracketed pair to the live marker file, at the end, in run order.
///
/// The position moves to the pair's post-boundary and the undone count is cleared
/// and the bound left standing (S5-marker-append); a later pair lands after this one,
/// so file order is run order (S5-ordered). A missing file reads as empty, so the
/// first append is the appended state (M-markers-persist).
pub fn append(store: &Store, pre: &str, post: &str) -> Result<(), PersistError> {
    let mut state = markers_persist::load(store)?;
    state.markers.push(Marker {
        pre: pre.to_string(),
        post: post.to_string(),
    });
    state.at = post.to_string();
    state.u = 0;
    markers_persist::save(store, &state)
}
