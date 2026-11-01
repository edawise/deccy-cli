//! Gate for M-TWO-ROOTS (the trunk state root resolved from the invocation workspace).
//!
//! Self-contained per M-tests: pulls the module under test in with a #[path] shim and
//! carries its own fixtures — no shared test-support module, std only. Every fixture is a
//! scratch workspace under the system temp directory: no jj invocation, no network.
//!
//! A `.jj/repo` file entry may name its target absolutely or relatively. A relative entry
//! is anchored at the directory containing the entry (the workspace's `.jj` directory),
//! which is the form `jj workspace add` writes for a linked workspace.

#[path = "../src/state_root.rs"]
mod state_root;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use state_root::{trunk_root, StateRootError};

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
            "deccy-state-root-{}-{}-{}",
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

/// A workspace whose repository entry is its own: `.jj/repo` is a directory.
fn own_entry(workspace: &Path) {
    fs::create_dir_all(workspace.join(".jj/repo/store"))
        .expect("a repository entry is creatable");
}

/// A workspace whose repository entry is a pointer file naming `target`.
fn pointer_entry(workspace: &Path, target: &Path) {
    let entry = workspace.join(".jj/repo");
    fs::create_dir_all(entry.parent().expect("the entry has a parent"))
        .expect("the workspace's .jj directory is creatable");
    fs::write(&entry, target.to_str().expect("a fixture path is UTF-8"))
        .expect("the pointer file is writable");
}

/// Assert that `result` refuses with `cause` and names a cause on `Display`, never a root.
fn refused(result: Result<PathBuf, StateRootError>, cause: fn(&StateRootError) -> bool, label: &str) {
    match result {
        Ok(root) => panic!(
            "{label}: resolved to {} instead of refusing",
            root.display()
        ),
        Err(err) => {
            assert!(cause(&err), "{label}: refused with another cause: {err:?}");
            assert!(
                !err.to_string().trim().is_empty(),
                "{label}: the refusal names no cause"
            );
        }
    }
}

#[test]
fn a_workspace_whose_entry_is_its_own_resolves_to_itself() {
    let scratch = Scratch::new("own");
    let workspace = scratch.dir("workspace");
    own_entry(&workspace);

    let resolved = trunk_root(&workspace).expect("a workspace owning its entry resolves");

    assert_eq!(resolved, workspace, "the root is the workspace itself");
}

#[test]
fn an_own_entry_returns_the_normalized_root() {
    let scratch = Scratch::new("own-normalized");
    let root = scratch.dir("root");
    own_entry(&root);
    // The invocation root spelled through a `..` component: `<root>/child/..`. The child
    // exists, so the path is a legitimate spelling of `<root>` rather than a missing path.
    let child = root.join("child");
    fs::create_dir_all(&child).expect("the child directory is creatable");
    let spelled = child.join("..");

    let resolved = trunk_root(&spelled).expect("a workspace owning its entry resolves");

    assert_eq!(
        resolved, root,
        "the own-entry root is the canonical workspace root"
    );
    assert!(
        !resolved
            .components()
            .any(|part| part == std::path::Component::ParentDir),
        "the returned root carries no `..` component: {}",
        resolved.display()
    );
}

#[test]
fn a_pointer_to_another_workspace_entry_resolves_to_that_root() {
    let scratch = Scratch::new("pointer");
    let trunk = scratch.dir("trunk");
    own_entry(&trunk);
    let invocation = scratch.dir("invocation");
    pointer_entry(&invocation, &trunk.join(".jj/repo"));

    let resolved = trunk_root(&invocation).expect("a pointer to another entry resolves");

    assert_eq!(
        resolved, trunk,
        "the root is the workspace the entry leads to"
    );
    assert_ne!(
        resolved, invocation,
        "the invocation root is never the answer"
    );
}

#[test]
fn a_relative_pointer_to_another_workspace_entry_resolves_to_that_root() {
    let scratch = Scratch::new("relative-pointer");
    let main = scratch.dir("main-ws");
    own_entry(&main);
    let linked = scratch.dir("linked");
    // What `jj workspace add` writes into `<linked>/.jj/repo`: a path relative to the
    // `.jj` directory itself, here naming `<main>/.jj/repo`.
    pointer_entry(&linked, Path::new("../../main-ws/.jj/repo"));

    let resolved = trunk_root(&linked).expect("a relative pointer to another entry resolves");

    assert_eq!(
        resolved, main,
        "the root is the workspace the relative entry leads to"
    );
    assert_ne!(
        resolved, linked,
        "the invocation root is never the answer"
    );
}

#[test]
fn a_missing_entry_refuses() {
    let scratch = Scratch::new("missing");
    let workspace = scratch.dir("workspace");
    fs::create_dir_all(workspace.join(".jj")).expect("the .jj directory is creatable");

    refused(
        trunk_root(&workspace),
        |err| matches!(err, StateRootError::MissingEntry { .. }),
        "a workspace with no .jj/repo",
    );
}

#[test]
fn an_empty_pointer_refuses() {
    let scratch = Scratch::new("empty");
    let workspace = scratch.dir("workspace");
    let entry = workspace.join(".jj/repo");
    fs::create_dir_all(entry.parent().expect("the entry has a parent"))
        .expect("the .jj directory is creatable");
    fs::write(&entry, "").expect("the empty pointer is writable");

    refused(
        trunk_root(&workspace),
        |err| matches!(err, StateRootError::EmptyEntry { .. }),
        "a workspace whose .jj/repo is an empty pointer",
    );
}

#[test]
fn an_unreadable_entry_refuses() {
    let scratch = Scratch::new("unreadable");
    let workspace = scratch.dir("workspace");
    let entry = workspace.join(".jj/repo");
    fs::create_dir_all(entry.parent().expect("the entry has a parent"))
        .expect("the .jj directory is creatable");
    // A present file entry whose bytes no string read can decode: it is neither missing
    // nor empty, so the refusal must name the read failure rather than an absent target.
    fs::write(&entry, [0xff, 0xfe, 0x00]).expect("the unreadable pointer is writable");

    refused(
        trunk_root(&workspace),
        |err| matches!(err, StateRootError::UnreadableEntry { .. }),
        "a workspace whose .jj/repo cannot be decoded",
    );
}

#[test]
fn a_pointer_to_a_path_that_does_not_exist_refuses() {
    let scratch = Scratch::new("dangling");
    let workspace = scratch.dir("workspace");
    let ghost = scratch.root.join("ghost");
    pointer_entry(&workspace, &ghost.join(".jj/repo"));

    refused(
        trunk_root(&workspace),
        |err| matches!(err, StateRootError::DanglingEntry { .. }),
        "a workspace whose pointer names a non-existent path",
    );
}

#[test]
fn a_dangling_relative_pointer_refuses() {
    let scratch = Scratch::new("relative-dangling");
    let linked = scratch.dir("linked");
    // `../../ghost-ws` from `<linked>/.jj` is `<scratch>/ghost-ws`, which is never created.
    pointer_entry(&linked, Path::new("../../ghost-ws/.jj/repo"));

    refused(
        trunk_root(&linked),
        |err| matches!(err, StateRootError::DanglingEntry { .. }),
        "a workspace whose relative pointer names a non-existent workspace",
    );
}

#[test]
fn a_pointer_to_a_path_that_is_not_a_repository_entry_refuses() {
    let scratch = Scratch::new("not-a-repo");
    let workspace = scratch.dir("workspace");
    let plain = scratch.dir("plain");
    fs::write(plain.join("not-a-repo"), "not a repository entry\n")
        .expect("the plain directory is writable");
    pointer_entry(&workspace, &plain);

    refused(
        trunk_root(&workspace),
        |err| matches!(err, StateRootError::NotARepoEntry { .. }),
        "a workspace whose pointer names a path that is not a .jj/repo",
    );
}

#[test]
fn a_relative_pointer_to_a_path_that_is_not_a_repository_entry_refuses() {
    let scratch = Scratch::new("relative-not-a-repo");
    let plain = scratch.dir("plain");
    fs::write(plain.join("marker"), "not a repository entry\n")
        .expect("the plain directory is writable");
    let linked = scratch.dir("linked");
    // `../../plain` from `<linked>/.jj` is `<scratch>/plain`: it exists, but is not a `.jj/repo`.
    pointer_entry(&linked, Path::new("../../plain"));

    refused(
        trunk_root(&linked),
        |err| matches!(err, StateRootError::NotARepoEntry { .. }),
        "a workspace whose relative pointer names a path that is not a .jj/repo",
    );
}
