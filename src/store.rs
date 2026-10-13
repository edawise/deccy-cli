//! The store: one root for file-backed graph state.
//!
//! S2 makes one store under one repo root the boundary every verb and guard crosses;
//! M-root (ppwnzwwy) fixes the one root held and lent with no reach of its own, and S2-ifs
//! (xquwmsts) the four interfaces each opened on it. This holds the root and lends it, and no
//! read or write reach of its own — each interface brings its own, and the read reach has left
//! it (M-store-read, tmwyqnyz, S2-read); identity, path derivation, validation, resolution,
//! and ownership each elaborate it.

use std::path::{Path, PathBuf};

/// One store over one repo root: the single door to file-backed graph state.
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Open a store on `root`. The root is the whole state; nothing else is reached.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Store { root: root.into() }
    }

    /// The root this store was opened on.
    pub fn root(&self) -> &Path {
        &self.root
    }
}
