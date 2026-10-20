//! Applying a journal side to a non-working-copy target through a disposable buffer.
//!
//! S3-journal-apply (omwrpwpq) fixes the shape: check the target's expected side and the
//! working copy's after side, materialise the target's side in a buffer opened at the target,
//! squash the buffer into the target, abandon the buffer, restore the user's working copy,
//! and leave the after side in it. A mismatch refuses naming the paths; no merge is
//! attempted and no non-working-copy change is written directly. S5-journal-store (qokrosvu)
//! fixes the content the sides name. M-jj (zruxtxrv) fixes the crossing; M-layout (szmuyqrq)
//! places the module flat at src/journal_apply.rs.
//!
//! What this fixes: undo's and redo's one apply. Undo expects the target to hold the after
//! side and the working copy the after side, writes before to the target and leaves the after
//! side in the working copy; redo writes after to the target and leaves the working copy at
//! the after side — the diff is gone. Every failure is carried; this module renders nothing.

use std::fs;
use std::path::Path;

use crate::jj::{Jj, JjError, Output};
use crate::journal::{self, Entry, JournalError, Side};
use crate::store::Store;

/// Which way a journal entry is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    /// Apply the before side to the target; the working copy keeps the after side.
    Undo,
    /// Apply the after side to the target; the working copy keeps the after side, so the diff
    /// is gone.
    Redo,
}

/// Why a journal side did not apply: the reason is carried, not collapsed.
#[derive(Debug)]
pub enum ApplyError {
    /// A journal entry or blob could not be read.
    Journal(JournalError),
    /// A jj crossing could not spawn.
    Spawn(String),
    /// A jj crossing ran and failed; its stderr is the evidence.
    Failed(String),
    /// The target and/or the working copy do not hold the expected side; the paths are named.
    Mismatch(Vec<String>),
    /// The working copy's files could not be read or written.
    WorkingCopy(String),
    /// Applying would leave a conflict.
    Conflict(String),
}

/// Apply `entry` in `direction`: verify both sides, then write the target's side into the
/// target and leave the working copy at the after side (S3-journal-apply).
pub fn apply(
    store: &Store,
    jj: &Jj,
    entry: &Entry,
    direction: Apply,
) -> Result<(), ApplyError> {
    let (target_expected, target_write) = match direction {
        Apply::Undo => (&entry.after, &entry.before),
        Apply::Redo => (&entry.before, &entry.after),
    };
    let working_expected = &entry.after; // the side the recording left in the WC (D15c)
    let working_write = &entry.after; // and the side it is left at

    // Both sides are checked before anything moves (D15c).
    let mut mismatched: Vec<String> = Vec::new();
    for (path, expected) in target_expected {
        let actual = target_content(jj, &entry.target, path)?;
        if actual.map(|bytes| journal::sha256_hex(&bytes)) != *expected {
            mismatched.push(path.clone());
        }
    }
    for (path, expected) in working_expected {
        let actual = working_content(store.root(), path)?;
        if actual.map(|bytes| journal::sha256_hex(&bytes)) != *expected {
            mismatched.push(path.clone());
        }
    }
    if !mismatched.is_empty() {
        return Err(ApplyError::Mismatch(mismatched));
    }

    if conflicts(jj)? {
        return Err(ApplyError::Conflict("a change is already conflicted".to_string()));
    }

    // The working copy is captured whole before the buffer: the change at `@` and the change
    // at `@-`. jj drops an empty, undescribed working-copy change when it moves the checkout,
    // so which of the two the restore needs is decided after the dance, by resolution.
    let working_copy = WorkingCopy::read(jj)?;

    // The buffer is a disposable change at the target; the side is written there and squashed
    // into the target (S3-journal-apply).
    cross(jj, &["new", &entry.target])?;
    for (path, hash) in target_write {
        let bytes = bytes_for(store, hash)?;
        write_working(store.root(), path, bytes.as_deref())?;
    }
    let paths: Vec<&str> = target_write.keys().map(String::as_str).collect();
    let mut squash = vec!["squash", "--into", entry.target.as_str(), "--"];
    squash.extend(paths);
    if let Err(error) = cross(jj, &squash) {
        restore(jj, &working_copy);
        return Err(error);
    }

    // Every crossing after the squash has landed is total: a failure in the conflict verdict,
    // the buffer abandon, the restore, or the final working-copy write puts the target back to
    // its expected side and the working copy back before the invocation refuses
    // (S3-journal-apply).
    match finish(store, jj, working_write, &working_copy) {
        Ok(()) => Ok(()),
        Err(error) => {
            let reversal = reverse_target(store, jj, &entry.target, target_expected);
            // The working copy is put back best-effort; the refusal carries the cause of the
            // failure that got here.
            let _ = restore_working_copy(jj, &working_copy);
            match reversal {
                Ok(()) => Err(error),
                Err(reversal) => Err(ApplyError::Failed(format!(
                    "{}; putting the target back also failed: {}",
                    describe(&error),
                    describe(&reversal)
                ))),
            }
        }
    }
}

/// The crossings after the squash has landed: verify it left no conflict, abandon the
/// buffer, restore the user's working copy, and leave the after side in it (S3-journal-apply,
/// D15c). The caller reverses the target on any failure here.
fn finish(
    store: &Store,
    jj: &Jj,
    working_write: &Side,
    working_copy: &WorkingCopy,
) -> Result<(), ApplyError> {
    if conflicts(jj)? {
        return Err(ApplyError::Conflict(
            "the squash leaves a change conflicted".to_string(),
        ));
    }
    cross(jj, &["abandon"])?;
    restore_working_copy(jj, working_copy)?;
    for (path, hash) in working_write {
        let bytes = bytes_for(store, hash)?;
        write_working(store.root(), path, bytes.as_deref())?;
    }
    Ok(())
}

/// The working copy's state before the buffer: the change at `@` and the change at `@-`,
/// read without snapshotting (M-jj-oplog's read discipline).
struct WorkingCopy {
    /// The change at `@` before the buffer.
    old: String,
    /// The change at `@-` before the buffer: where an empty working copy is reopened.
    parent: String,
}

impl WorkingCopy {
    fn read(jj: &Jj) -> Result<Self, ApplyError> {
        Ok(WorkingCopy {
            old: change_id(jj, "@")?,
            parent: change_id(jj, "@-")?,
        })
    }
}

/// Put the user's working copy back after the buffer is gone (S3-journal-apply): edit the old
/// change when jj still resolves it, otherwise open a fresh empty change on the old parent.
/// jj abandons an empty, undescribed working-copy change when it moves the checkout, so the
/// old id may no longer resolve; the choice is made here, after the dance, by resolution and
/// never by predicting emptiness. The recreated change is empty and its description, if any,
/// is not preserved.
fn restore_working_copy(jj: &Jj, working_copy: &WorkingCopy) -> Result<(), ApplyError> {
    if resolves(jj, &working_copy.old)? {
        cross(jj, &["edit", &working_copy.old])?;
    } else {
        cross(jj, &["new", &working_copy.parent])?;
    }
    Ok(())
}

/// Whether a change id still resolves; a read that cannot run carries its reason.
fn resolves(jj: &Jj, id: &str) -> Result<bool, ApplyError> {
    let out = jj
        .run(&[
            "log",
            "--ignore-working-copy",
            "--no-graph",
            "-r",
            id,
            "-T",
            "change_id",
        ])
        .map_err(spawn)?;
    Ok(out.status == 0)
}

/// Put the target back to the side it held before the apply, through the same buffer
/// mechanism (S3-journal-apply): open a buffer at the target, write `side` there, and squash
/// it into the target. The buffer is dropped whatever the write does; the caller puts the
/// working copy back.
fn reverse_target(
    store: &Store,
    jj: &Jj,
    target: &str,
    side: &Side,
) -> Result<(), ApplyError> {
    cross(jj, &["new", target])?;
    let reversed = (|| -> Result<(), ApplyError> {
        for (path, hash) in side {
            let bytes = bytes_for(store, hash)?;
            write_working(store.root(), path, bytes.as_deref())?;
        }
        let mut squash = vec!["squash", "--into", target, "--"];
        squash.extend(side.keys().map(String::as_str));
        cross(jj, &squash)?;
        Ok(())
    })();
    let _ = jj.run(&["abandon"]);
    reversed
}

/// The failure as text, so two causes can be carried together when a reversal also fails.
fn describe(error: &ApplyError) -> String {
    match error {
        ApplyError::Journal(error) => format!("{error:?}"),
        ApplyError::Spawn(reason)
        | ApplyError::Failed(reason)
        | ApplyError::WorkingCopy(reason)
        | ApplyError::Conflict(reason) => reason.clone(),
        ApplyError::Mismatch(paths) => {
            format!("the expected side is missing at {}", paths.join(", "))
        }
    }
}

/// The bytes a side names, or none when the path is absent.
fn bytes_for(store: &Store, hash: &Option<String>) -> Result<Option<Vec<u8>>, ApplyError> {
    match hash {
        Some(hash) => journal::content(store, hash)
            .map(Some)
            .map_err(ApplyError::Journal),
        None => Ok(None),
    }
}

/// The content of `path` at the target change, or none when it is absent (S5-journal-store,
/// M-jj).
fn target_content(jj: &Jj, target: &str, path: &str) -> Result<Option<Vec<u8>>, ApplyError> {
    let listed = jj
        .run(&[
            "file",
            "list",
            "--ignore-working-copy",
            "-r",
            target,
            "--",
            path,
        ])
        .map_err(spawn)?;
    if listed.status != 0 {
        return Err(ApplyError::Failed(listed.stderr.trim().to_string()));
    }
    if listed.stdout.trim().is_empty() {
        return Ok(None);
    }
    let shown = jj
        .run_bytes(&[
            "file",
            "show",
            "--ignore-working-copy",
            "-r",
            target,
            "--",
            path,
        ])
        .map_err(spawn)?;
    if shown.status != 0 {
        return Err(ApplyError::Failed(shown.stderr.trim().to_string()));
    }
    Ok(Some(shown.stdout))
}

/// The working copy's content at `path`, or none when it is absent.
fn working_content(root: &Path, path: &str) -> Result<Option<Vec<u8>>, ApplyError> {
    match fs::read(root.join(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ApplyError::WorkingCopy(error.to_string())),
    }
}

/// Write one working-copy path: content, or removal when the side is absent.
fn write_working(
    root: &Path,
    path: &str,
    bytes: Option<&[u8]>,
) -> Result<(), ApplyError> {
    let full = root.join(path);
    match bytes {
        Some(bytes) => {
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| ApplyError::WorkingCopy(error.to_string()))?;
            }
            fs::write(&full, bytes).map_err(|error| ApplyError::WorkingCopy(error.to_string()))
        }
        None => match fs::remove_file(&full) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ApplyError::WorkingCopy(error.to_string())),
        },
    }
}

/// The change id at `revset` in the working copy's view, read without snapshotting it
/// (M-jj-oplog's read discipline).
fn change_id(jj: &Jj, revset: &str) -> Result<String, ApplyError> {
    let out = cross(
        jj,
        &[
            "log",
            "--ignore-working-copy",
            "--no-graph",
            "-r",
            revset,
            "-T",
            "change_id",
        ],
    )?;
    Ok(out.stdout.trim().to_string())
}

/// Whether any change is conflicted (S3-journal-apply's conflict check).
fn conflicts(jj: &Jj) -> Result<bool, ApplyError> {
    let out = cross(jj, &[
        "log",
        "--ignore-working-copy",
        "--no-graph",
        "-r",
        "conflicts()",
        "-T",
        "change_id",
    ])?;
    Ok(!out.stdout.trim().is_empty())
}

/// Restore the user's working copy after a failed apply; the original failure is the one
/// reported. Best-effort: the buffer is dropped, then the working copy is put back.
fn restore(jj: &Jj, working_copy: &WorkingCopy) {
    let _ = jj.run(&["abandon"]);
    let _ = restore_working_copy(jj, working_copy);
}

/// Run one crossing, carrying a spawn failure and a non-zero status as values.
fn cross(jj: &Jj, args: &[&str]) -> Result<Output, ApplyError> {
    let out = jj.run(args).map_err(spawn)?;
    if out.status != 0 {
        return Err(ApplyError::Failed(out.stderr.trim().to_string()));
    }
    Ok(out)
}

fn spawn(error: JjError) -> ApplyError {
    match error {
        JjError::Spawn(reason) => ApplyError::Spawn(reason),
    }
}

impl From<JournalError> for ApplyError {
    fn from(error: JournalError) -> Self {
        ApplyError::Journal(error)
    }
}
