//! Gate for M-main (rklnxlmy), refine edge to M-TWO-ROOTS (xymxqnvz): the crate root names
//! the state-root resolver in its flat module map.
//!
//! The resolver's own contract is gated by its own evaluation; this check is the *use* the
//! crate root makes of the resolver: the module is declared once at the root, so consumers
//! reach `trunk_root` and `StateRootError` through the public path `deccy_cli::state_root`,
//! not through a module nested inside `context.rs` and not through a `#[path]` shim of
//! `src/state_root.rs`. The test therefore links the library target and drives that path as
//! a consumer would; it declares no module shim of its own.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use deccy_cli::state_root::{trunk_root, StateRootError};

/// A scratch directory that removes itself, so every fixture stays under the temp dir.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Scratch {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is after the epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "deccy-main-state-root-path-{}-{}-{}",
            std::process::id(),
            nanos,
            label
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the scratch root is creatable");
        let root = fs::canonicalize(&root).expect("the scratch root canonicalizes");
        Scratch { root }
    }

    fn dir(&self, name: &str) -> PathBuf {
        let dir = self.root.join(name);
        fs::create_dir_all(&dir).expect("the fixture directory is creatable");
        dir
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The public path reaches the resolver: a workspace whose `.jj/repo` is its own repository
/// entry resolves to its own root through `deccy_cli::state_root::trunk_root`.
#[test]
fn the_public_path_resolves_a_workspaces_own_root() {
    let scratch = Scratch::new("own");
    let workspace = scratch.dir("workspace");
    fs::create_dir_all(workspace.join(".jj").join("repo").join("store"))
        .expect("a repository entry is creatable");

    let resolved = trunk_root(&workspace).expect("the workspace's own repository entry resolves");

    assert_eq!(
        resolved,
        fs::canonicalize(&workspace).expect("the workspace root canonicalizes"),
        "the public module resolves the workspace's own root"
    );
}

/// The public path exposes the resolver's typed refusal: a workspace with no repository
/// entry refuses as `deccy_cli::state_root::StateRootError::MissingEntry`, naming the entry
/// it looked for rather than answering a root.
#[test]
fn the_public_path_exposes_the_typed_refusal() {
    let scratch = Scratch::new("missing");
    let workspace = scratch.dir("workspace");

    match trunk_root(&workspace) {
        Ok(root) => panic!(
            "resolved to {} instead of refusing a missing entry",
            root.display()
        ),
        Err(StateRootError::MissingEntry { path }) => {
            assert_eq!(path, workspace.join(".jj").join("repo"));
        }
        Err(other) => panic!("refused with another cause: {other:?}"),
    }
}
