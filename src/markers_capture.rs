//! Capture: keep the entries an operation's run window holds for the invoking workspace.
//!
//! S5-bracket (otmxqplr) fixes the protocol — the op head read before the operation runs
//! and again after, the run itself untouched — S5-lifecycle (vwlrqoxk) which invocations
//! feed it, S5-observation (urqytyqn) the settle and the observational reads, and
//! G-capture (vntvmykw) that protocol fixed in a term; M-layout (szmuyqrq) places the
//! module flat at src/markers_capture.rs. The read is M-jj-oplog's (mvwnksou), crossing
//! M-jj's (zruxtxrv) runner, and the entries land through M-markers-append's (wkpyqtyx)
//! append, which reaches the file through M-markers-persist (wzyorrny) and M-root
//! (ppwnzwwy).
//!
//! What this fixes: one transition. Settle the working copy once (`jj util snapshot`),
//! read the live op head, run the given operation untouched, read the head again, and keep
//! the window's entries that are the invoking workspace's own (G-footprint, xwnylypn) — so
//! which invocation brackets stays the caller's call (S5-lifecycle), what the run does
//! stays the run's, and a snapshot of edits that predate the invocation is outside the
//! window rather than in it. A failed settle or read aborts before the run, so nothing is
//! bracketed that never opened. The run answers the target it named; one that answers none
//! closes its bracket and records nothing, so no marker names an empty target, and one whose
//! window kept no entries records nothing too, so a no-op buys no un-undoable step
//! (S5-record-named, wotmvzyt). Every failure is carried; this module renders nothing.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::jj::Jj;
use crate::journal::{self, JournalError, Snapshot};
use crate::markers::Kind;
use crate::markers_append::append;
use crate::markers_persist::PersistError;
use crate::oplog::{oplog, window, workspace, OplogError};
use crate::store::Store;

/// Bracket `run` between the op head read before it and again after it (S5-bracket), appending
/// the operation's record when the run names the change it acted on (S5-record-named, wotmvzyt).
///
/// The run answers its value and the target it named — `None` when it named none. A run that
/// names none closes its bracket and records nothing: no entries are kept and no marker is
/// appended, so no marker names an empty target. A run that names a target and kept entries
/// is recorded even when it fails — its window may hold work that landed before the refusal —
/// while a run whose window kept no entries records nothing: no marker is appended and the
/// marker file is left as it stood. The working copy is settled once before the
/// pre-read, so its snapshot is outside the window (S5-observation). A failed settle or read
/// aborts before the run; every failure is carried.
pub fn capture<T>(
    store: &Store,
    jj: &Jj,
    kind: Kind,
    run: impl FnOnce() -> (T, Option<String>),
) -> Result<T, CaptureError> {
    settle(jj)?;
    let pre = oplog(jj)?;
    let (value, target) = run();
    let post = oplog(jj)?;
    if let Some(target) = target {
        let entries = footprint(jj, &pre, &post)?;
        if !entries.is_empty() {
            append(store, kind, &target, &entries)?;
        }
    }
    Ok(value)
}

/// Bracket a recording: snapshot the target's owned paths before and after the run, store the
/// two sides as one journal entry, and append a marker naming that entry (S5-journal-capture).
///
/// The target's owned set is the caller's (D2), so exactly those paths are snapshotted; a run
/// that names no target or kept no entries records nothing, exactly as `capture` does. The
/// entry is written before the marker that names it, so no marker points at a missing entry.
pub fn capture_recording<T>(
    store: &Store,
    jj: &Jj,
    target: &str,
    owned: &BTreeSet<PathBuf>,
    run: impl FnOnce() -> (T, Option<String>),
) -> Result<T, CaptureError> {
    settle(jj)?;
    let pre = oplog(jj)?;
    let before = snapshot(jj, target, owned)?;
    let (value, named) = run();
    let post = oplog(jj)?;
    if named.is_some() {
        let entries = footprint(jj, &pre, &post)?;
        if !entries.is_empty() {
            let after = snapshot(jj, target, owned)?;
            let entry = journal::put(store, "record", target, &before, &after)?;
            append(store, Kind::Record, target, &[entry])?;
        }
    }
    Ok(value)
}

/// Why a bracket did not close: the reason is carried, not collapsed.
#[derive(Debug)]
pub enum CaptureError {
    /// The working copy could not be settled, or the op head could not be read before or
    /// after the run (M-jj-oplog, S5-observation).
    Oplog(OplogError),
    /// The entries could not be appended (M-markers-append).
    Append(PersistError),
    /// The journal entry could not be stored (M-journal).
    Journal(JournalError),
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

impl From<JournalError> for CaptureError {
    fn from(error: JournalError) -> Self {
        CaptureError::Journal(error)
    }
}

impl From<crate::jj::JjError> for CaptureError {
    fn from(error: crate::jj::JjError) -> Self {
        CaptureError::Oplog(OplogError::from(error))
    }
}

/// The target's owned paths at its change, before or after the recording (S5-journal-capture).
fn snapshot(
    jj: &Jj,
    target: &str,
    owned: &BTreeSet<PathBuf>,
) -> Result<Snapshot, CaptureError> {
    let mut side = Snapshot::new();
    for path in owned {
        let name = path.to_string_lossy().into_owned();
        side.insert(name.clone(), read_at(jj, target, &name)?);
    }
    Ok(side)
}

/// One path's content at the target change, or none when the path is absent. stdout is read as
/// bytes so a binary file survives the snapshot.
fn read_at(jj: &Jj, revision: &str, path: &str) -> Result<Option<Vec<u8>>, CaptureError> {
    let listed = jj.run(&[
        "file",
        "list",
        "--ignore-working-copy",
        "-r",
        revision,
        "--",
        path,
    ])?;
    if listed.status != 0 {
        return Err(CaptureError::Oplog(OplogError::Failed(listed)));
    }
    if listed.stdout.trim().is_empty() {
        return Ok(None);
    }
    let shown = jj.run_bytes(&[
        "file",
        "show",
        "--ignore-working-copy",
        "-r",
        revision,
        "--",
        path,
    ])?;
    if shown.status != 0 {
        return Err(CaptureError::Oplog(OplogError::Failed(crate::jj::Output {
            status: shown.status,
            stdout: String::from_utf8_lossy(&shown.stdout).into_owned(),
            stderr: shown.stderr,
        })));
    }
    Ok(Some(shown.stdout))
}

/// Settle the working copy once, before the window opens (S5-observation): the snapshot of
/// whatever it already held becomes an operation of its own, taken before the pre-read, so
/// it never falls inside the window and is never part of what the bracket keeps.
fn settle(jj: &Jj) -> Result<(), OplogError> {
    let out = jj.run(&["util", "snapshot"])?;
    if out.status != 0 {
        return Err(OplogError::Failed(out));
    }
    Ok(())
}

/// The entries the run window between the two head reads holds for the invoking workspace
/// (G-footprint): the window's entries, attributed to the workspace the runner stands in,
/// in run order.
///
/// The op log does not say which workspace is invoking, so that is read off the runner's own
/// root through M-jj-oplog; a window that holds none of its entries keeps nothing.
fn footprint(jj: &Jj, pre: &str, post: &str) -> Result<Vec<String>, OplogError> {
    let invoking = workspace(jj)?;
    Ok(window(jj, pre, post)?
        .into_iter()
        .filter(|entry| entry.workspace == invoking)
        .map(|entry| entry.id)
        .collect())
}
