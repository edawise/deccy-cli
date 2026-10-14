//! The markers module: the marker file's own read and write.
//!
//! The marker file has its own interface (S2-markers, ppmvwrlu), opened on the one repo
//! root (M-root, ppwnzwwy) and never a generic read or write over arbitrary paths. This is
//! the module that realizes it: it reaches the file itself — its own read and its own
//! write, opened on the shared root and naming the file as a path under that root. `read`
//! loads the file whole (S2-markers-read, vupsskmp) — a missing file yielding nothing
//! (S2-markers-read-absent, rlumurvk), a file that is there yielding its contents even when
//! they are empty, an unreadable one carried as a value (S2-markers-read-fail, kyrnspkl) —
//! and `write` saves the contents whole (S2-markers-write,
//! xlqyyprp), landing them through the shared atomic rule (S2-atomic, rmmlokop), a
//! replacement never a truncate, a failed write carried and the file left unchanged
//! (S2-markers-write-fail, xokynzpp). The file is one document, rewritten whole on every
//! bracketed operation and every revert; every failure is carried as a value, never
//! rendered here. M-layout (szmuyqrq) places the module flat at src/markers_file.rs.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::store::Store;

/// The file the marker state is recorded in, relative to the repository root (S2-markers).
pub const MARKERS_PATH: &str = ".deccy/markers.json";

/// Why the marker file could not be read or written: the reason is carried, not collapsed
/// (S2-markers-read-fail, S2-markers-write-fail).
#[derive(Debug)]
pub enum FileError {
    /// The file could not be read.
    Read(String),
    /// The file could not be written.
    Write(String),
}

/// Read the marker file whole (S2-markers-read).
///
/// The contents come back as they stand — the whole file, never a part of it — or nothing
/// at all when the file is not there (S2-markers-read-absent): absence yields no contents,
/// not a failure, so the caller reaches it the way a first run does. A file that is there
/// yields its contents even when they are empty, so absence and empty contents stay
/// distinct states. Any other read error is carried (S2-markers-read-fail), never rendered.
pub fn read(store: &Store) -> Result<Option<String>, FileError> {
    load(store.root().join(MARKERS_PATH))
}

/// Write `contents` to the marker file whole (S2-markers-write).
///
/// The contents go back through the atomic rule (S2-atomic): a sibling temporary is
/// written, flushed, and renamed over the file, so no reader ever observes a partial file
/// and a replacement never leaves a stale tail. A failed write removes the temporary and
/// leaves the previous contents in place, the error carried (S2-markers-write-fail).
pub fn write(store: &Store, contents: &str) -> Result<(), FileError> {
    replace(&store.root().join(MARKERS_PATH), contents)
}

/// Read the file whole: absence yields nothing, any other error is carried (S2-markers-read).
fn load(target: PathBuf) -> Result<Option<String>, FileError> {
    match fs::read_to_string(&target) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(FileError::Read(error.to_string())),
    }
}

/// Write `contents` to `target` by atomic replacement (S2-atomic).
///
/// The temporary file lives in the target's own directory; it is written, flushed, and
/// renamed over the target, so no reader ever observes a partial file. A failed write
/// removes the temporary and leaves the previous contents in place, the error carried
/// (S2-markers-write-fail).
fn replace(target: &PathBuf, contents: &str) -> Result<(), FileError> {
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let directory = target
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(""));
    let temporary = directory.join(format!("{}.tmp-{}", file_name, std::process::id()));

    let outcome = write_then_replace(&temporary, target, contents);
    if outcome.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    outcome.map_err(|error| FileError::Write(error.to_string()))
}

/// The write itself: flush the temporary, then rename it over the target (S2-atomic).
fn write_then_replace(temporary: &PathBuf, target: &PathBuf, contents: &str) -> io::Result<()> {
    let mut file = fs::File::create(temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, target)
}
