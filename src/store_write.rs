//! The write module: the node document named from its id, placed whole under the root by the
//! atomic rule.
//!
//! S2-write (pnpsmvpz) writes node documents to the working copy; this is the module that
//! realizes it. It names the document through S2-path's single derivation (pumpuqkv), the same
//! one the read calls, under the root it takes from the shared root holder (ppwnzwwy)
//! (S2-write-under-root, nkqpvozu), and places it whole (S2-write-whole, wznzyssk): the bytes it
//! is given are the document's complete contents, a replacement and never an append, never a
//! patch in place. It lands them by the shared atomic rule (rmmlokop): a temporary in the
//! document's own directory, a sibling that is not a node document, written and flushed and
//! renamed over the document. A write that cannot be completed leaves the document as it was and
//! no temporary behind (S2-write-fail, tkwuyxux), the cause carried as a value; the module
//! refuses nothing and renders nothing (S2-write-refusal-site, myoztqyu). It creates the
//! document's directory when it is absent — the working copy's `nodes/` can be missing after
//! an undo abandoned the last node or a sibling creation checked out a tree without it — and a
//! directory it cannot create is a write that cannot be completed. The id reaches it already
//! checked (S2-id, wpxrlzuy), so naming stays total and it validates nothing. M-layout
//! (szmuyqrq) places the module flat at src/store_write.rs.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::store::Store;
use crate::store_path::{self, Id};

/// Why a write could not be completed: the cause is carried, not rendered (S2-write-fail).
#[derive(Debug)]
pub struct WriteError(io::Error);

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Write `contents` as `id`'s node document under the store root (S2-write).
///
/// The document is placed whole: the bytes given are its complete contents, by atomic
/// replacement — a sibling temporary in the document's own directory, written and flushed, then
/// renamed over the document (S2-write-whole, rmmlokop). The temporary's name is not a node
/// document's, so a half-written file can never be read as one. A failure removes the temporary,
/// leaves the document as it was, and comes back as the value carrying its cause (S2-write-fail);
/// the document's directory is created when it is absent — a directory that cannot be created is
/// such a failure. The id is taken already checked (S2-id), so naming stays total and nothing here
/// validates it.
pub fn write(store: &Store, id: &Id, contents: &str) -> Result<(), WriteError> {
    let target = store.root().join(store_path::node_path(id));
    let directory = target
        .parent()
        .expect("a node document has a parent directory")
        .to_path_buf();
    fs::create_dir_all(&directory).map_err(WriteError)?;
    let temporary = directory.join(format!("{}.json.tmp-{}", id.as_str(), std::process::id()));

    let outcome = place(&temporary, &target, contents);
    if outcome.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    outcome.map_err(WriteError)
}

/// Place `contents` by the atomic rule: create the temporary in the target's own directory,
/// write it whole, flush it, close it, then rename it over the target (rmmlokop).
fn place(temporary: &PathBuf, target: &PathBuf, contents: &str) -> io::Result<()> {
    let mut file = fs::File::create(temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, target)
}
