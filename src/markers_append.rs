//! Appending an operation's record to the live marker file: read, discard the tail,
//! append, write.
//!
//! S5-marker-append (nptquxlz) fixes what closing a bracket does to the state — the undone
//! tail discarded at the position, then the operation's kind, targeted change and kept entry
//! ids appended in run order with the position advanced to the new length — and S5-ordered
//! (kupzywsq) that marker order is the order operations ran in; M-layout (szmuyqrq) places
//! the module flat at src/markers_append.rs. The linear history (kpptprql) fixes the
//! truncation. It names the state through M-markers' (qrqumusy) closed types, crosses the
//! store through M-root (ppwnzwwy), and reads and writes the whole file through
//! M-markers-persist's (wzyorrny) load and save — never std::fs, never a truncate of the
//! file.
//!
//! What this fixes: one transition. A missing file reads as empty, so the first append
//! lands the appended state (M-markers-persist). Every failure is M-markers-persist's
//! carried reason; this module renders nothing.

use crate::markers::{Kind, Marker};
use crate::markers_persist::{self, PersistError};
use crate::store::Store;

/// Append one operation's own record — its kind, the change it targeted and its kept entry
/// ids — to the live marker file, at the end, in run order.
///
/// The undone tail is discarded first: the marker list is truncated to the position, so a
/// new operation supersedes the undone work (S5-marker-append, kpptprql). The new marker
/// then lands at the new end and the position advances to the list's length; a later marker
/// lands after this one, so file order is run order (S5-ordered). A fresh marker records no
/// reverts of its own. A missing file reads as empty, so the first append lands the appended
/// state (M-markers-persist).
pub fn append(
    store: &Store,
    kind: Kind,
    target: &str,
    entries: &[String],
) -> Result<(), PersistError> {
    let mut state = markers_persist::load(store)?;
    state.markers.truncate(state.pos as usize);
    state.markers.push(Marker {
        kind,
        target: target.to_string(),
        entries: entries.to_vec(),
        reverts: Vec::new(),
    });
    state.pos = state.markers.len() as u64;
    markers_persist::save(store, &state)
}
