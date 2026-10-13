//! The exclude module: the exclude file's own read and append.
//!
//! The exclude file has its own interface (S2-exclude, zuvsnyxy), opened on the one repo
//! root (M-root, ppwnzwwy) and never a generic read or write over arbitrary paths. This is
//! the module that realizes it: it reaches the file itself — its own read and its own write,
//! opened on the shared root and naming the file as a path under that root. `read` loads the file whole (S2-exclude-read,
//! xpxukylv) — a missing file reading as empty (S2-exclude-read-absent, zvmsnkln), an
//! unreadable one carried as a value (S2-exclude-read-fail, llkvmtou) — and `append` writes
//! the recorded entry as a line (S2-exclude-append, xlowmllo), landing the whole file back
//! through the shared atomic rule (S2-atomic, rmmlokop), a replacement never a
//! truncate, a failed append carried and the file left as it was (S2-exclude-append-fail,
//! zwurwxwq). The file is git's own exclude template, appended rather than clobbered; every
//! failure is carried as a value, never rendered here. M-layout (szmuyqrq) places the module
//! flat at src/exclude.rs.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::store::Store;

/// The file the state directory is recorded in, relative to the repository root (S2-exclude).
pub const EXCLUDE_PATH: &str = ".git/info/exclude";

/// Why the exclude file could not be read or written: the reason is carried, not collapsed
/// (S2-exclude-read-fail, S2-exclude-append-fail).
#[derive(Debug)]
pub enum ExcludeError {
    /// The file could not be read.
    Read(String),
    /// The file could not be written.
    Write(String),
}

/// Read the exclude file whole (S2-exclude-read).
///
/// The contents come back as they stand, so every line already recorded is visible to the
/// write. A missing file reads as empty (S2-exclude-read-absent): absence is the empty
/// state, not a failure, so the caller reaches it the way the markers reach a missing
/// marker file. Any other read error is carried (S2-exclude-read-fail), never rendered.
pub fn read(store: &Store) -> Result<String, ExcludeError> {
    load(store.root().join(EXCLUDE_PATH))
}

/// Append `entry` as a line to the exclude file under the store's root (S2-exclude-append).
///
/// The file is never rebuilt from a template and never truncated: everything already there
/// stays, a final line without a newline gets one first so `entry` stands on its own line,
/// and the whole file goes back through the atomic rule (S2-atomic). A missing file is
/// read as empty, so the append creates it carrying the entry alone (S2-exclude-read-absent).
/// This reads the file itself rather than through `read`: the append's own read is what it
/// writes back, and the interface's read is the interface's answer, not this call path's. A
/// read or write failure is carried (S2-exclude-read-fail, S2-exclude-append-fail), never
/// rendered; a failed write leaves the file as it was.
pub fn append(store: &Store, entry: &str) -> Result<(), ExcludeError> {
    let target = store.root().join(EXCLUDE_PATH);
    let mut recorded = load(target.clone())?;
    if !recorded.is_empty() && !recorded.ends_with('\n') {
        recorded.push('\n');
    }
    recorded.push_str(entry);
    recorded.push('\n');
    replace(&target, &recorded)
}

/// Read the file whole: absence is the empty state, any other error is carried
/// (S2-exclude-read). Shared by the interface's read and the append's own read.
fn load(target: PathBuf) -> Result<String, ExcludeError> {
    match fs::read_to_string(&target) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(ExcludeError::Read(error.to_string())),
    }
}

/// Write `contents` to `target` by atomic replacement (S2-atomic).
///
/// The temporary file lives in the target's own directory; it is written, flushed, and
/// renamed over the target, so no reader ever observes a partial file. A failed write
/// removes the temporary and leaves the previous contents in place, the error carried
/// (S2-exclude-append-fail).
fn replace(target: &PathBuf, contents: &str) -> Result<(), ExcludeError> {
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
    outcome.map_err(|error| ExcludeError::Write(error.to_string()))
}

/// The write itself: flush the temporary, then rename it over the target (S2-atomic).
fn write_then_replace(temporary: &PathBuf, target: &PathBuf, contents: &str) -> io::Result<()> {
    let mut file = fs::File::create(temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, target)
}
