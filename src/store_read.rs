//! Read a node document — whole, absent, or failed — from the change that owns it.
//!
//! The read interface (S2-read, nqktpzuu) reads node documents from jj, from the change that
//! owns the node; this is the module that realizes it with nothing of the old store behind it
//! — it holds no root and reaches no filesystem. It is handed the runner (M-jj, zruxtxrv) and
//! names the document by a repo-relative path at the owning change id; it crosses the seam
//! (S2-read-seam, zmnqomwt) through M-jj-read (osplmlvq). The read is total: the document
//! whole (S2-read-whole, nkyymtvp), absence when no such document is at that change
//! (S2-read-absent, tqvyvkwv) — not empty contents, not a failure — or a failure carried as
//! its evidence (S2-read-fail, wvrnvwzt), never rendered here. Its id → document entry locates
//! the document at `nodes/<id>.json` through M-store-path's single derivation (vkztyrzm).

use crate::jj::{Jj, Output};
use crate::jjread;
use crate::store_path::{node_path, Id};

/// What a node read found at the owning change (S2-read-whole, S2-read-absent).
#[derive(Debug)]
pub enum Document {
    /// The document's bytes as they stand at the owning change (S2-read-whole).
    Whole(String),
    /// No document exists at the owning change (S2-read-absent).
    Absent,
}

/// Why a node document could not be read (S2-read-fail: the evidence is carried).
#[derive(Debug)]
pub enum ReadError {
    /// The crossing itself could not run (S3-err).
    Spawn(String),
    /// The crossing ran but failed for a reason other than absence (S2-read-fail).
    Failed(Output),
}

/// Read the document at `path`, at `revision`, through the jj crossing (S2-read).
///
/// A document that is there reads whole; a path with no document at that change reads as
/// absence (S2-read-absent), distinct from any failure; any other crossing failure is carried
/// as its evidence (S2-read-fail). Absence is jj's own report that the path is not at the
/// revision, so the read stays one crossing (S2-read-seam).
pub fn read(jj: &Jj, path: &str, revision: &str) -> Result<Document, ReadError> {
    match jjread::read(jj, path, revision) {
        Ok(text) => Ok(Document::Whole(text)),
        Err(jjread::ReadError::Spawn(reason)) => Err(ReadError::Spawn(reason)),
        Err(jjread::ReadError::Failed(out)) if absent(&out) => Ok(Document::Absent),
        Err(jjread::ReadError::Failed(out)) => Err(ReadError::Failed(out)),
    }
}

/// Whether a crossing failure is jj reporting that the path is not at the revision
/// (S2-read-absent): absence is not a failure of the read.
fn absent(out: &Output) -> bool {
    out.stderr.contains("No such path")
}

/// Read `id`'s node document at the change the id names (S2-read): the path is M-store-path's
/// single derivation, `nodes/<id>.json`, read at the owning change id (S2-read-seam).
pub fn read_node(jj: &Jj, id: &Id) -> Result<Document, ReadError> {
    read(
        jj,
        node_path(id).to_str().expect("a node path is valid UTF-8"),
        id.as_str(),
    )
}
