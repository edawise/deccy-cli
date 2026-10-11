//! Capture: bracket an operation with its pre- and post-operation ids.
//!
//! S5-bracket (otmxqplr) fixes the protocol — the op head read before the operation runs
//! and again after, both ids kept, the run itself untouched — S5-lifecycle (vwlrqoxk)
//! which invocations feed it, and G-capture (vntvmykw) that protocol fixed in a term;
//! M-layout (szmuyqrq) places the module flat at src/markers_capture.rs. The read is
//! M-jj-oplog's (mvwnksou), crossing M-jj's (zruxtxrv) runner, and the pair lands through
//! M-markers-append's (wkpyqtyx) close, which reaches the file through M-markers-persist
//! (wzyorrny) and M-root (ppwnzwwy).
//!
//! What this fixes: one transition. Read the live op head, run the given operation
//! untouched, read the head again, and keep the (pre, post) pair — so which invocation
//! brackets stays the caller's call (S5-lifecycle) and what the run does stays the run's.
//! A failed read aborts before the run, so nothing is bracketed that never opened. Every
//! failure is carried; this module renders nothing.

use crate::jj::Jj;
use crate::markers_append::append;
use crate::markers_persist::PersistError;
use crate::oplog::{oplog, OplogError};
use crate::store::Store;

/// Why a bracket did not close: the reason is carried, not collapsed.
#[derive(Debug)]
pub enum CaptureError {
    /// The op head could not be read before or after the run (M-jj-oplog).
    Oplog(OplogError),
    /// The pair could not be appended (M-markers-append).
    Append(PersistError),
}

impl From<OplogError> for CaptureError {
    fn from(error: OplogError) -> Self {
        CaptureError::Oplog(error)
    }
}

impl From<PersistError> for CaptureError {
    fn from(error: PersistError) -> Self {
        CaptureError::Append(error)
    }
}

/// Bracket `run` between the op head read before it and again after it (S5-bracket).
///
/// The run is invoked untouched between the two reads — its outcome is the caller's value
/// and passes through unexamined, so a run that fails after the head moved still closes
/// its bracket — and the (pre, post) pair is appended by M-markers-append, which keeps
/// file order run order. A failed read aborts before the run.
pub fn capture<T>(store: &Store, jj: &Jj, run: impl FnOnce() -> T) -> Result<T, CaptureError> {
    let pre = oplog(jj)?;
    let value = run();
    let post = oplog(jj)?;
    append(store, &pre, &post)?;
    Ok(value)
}
