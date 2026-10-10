//! Gate for M-context (the seam's caller).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parents
//! (jj, store, refusal); no shared test-support module. Plain `rustc --test`, std only — it
//! runs a real jj in a scratch repo and pins the root-optional contract: a directory with a
//! workspace resolves a root and a store, and a directory without one resolves root-less,
//! where the root and store accessors refuse naming the absence.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/store.rs"]
mod store;
#[path = "../src/refusal.rs"]
mod refusal;
#[path = "../src/context.rs"]
mod context;

use std::path::{Path, PathBuf};
use std::process::Command;

use context::resolve;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-context-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn jj_repo(dir: &Path) {
    let status = Command::new("jj")
        .args(["git", "init"])
        .arg(dir)
        .status()
        .expect("jj is on PATH for the gate");
    assert!(status.success(), "jj git init must succeed");
}

fn workspace_root(dir: &Path) -> String {
    let out = Command::new("jj")
        .args(["workspace", "root"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn the_resolved_binary_is_a_real_executable_path() {
    let dir = scratch("binary");
    jj_repo(&dir);
    let ctx = resolve(&dir).expect("a jj repo resolves");
    assert!(
        ctx.jj().binary().is_file(),
        "binary {:?} must be a real file",
        ctx.jj().binary()
    );
}

#[test]
fn the_resolved_root_is_the_workspace_root() {
    let dir = scratch("root");
    jj_repo(&dir);
    let ctx = resolve(&dir).expect("a jj repo resolves");
    let root = ctx.root().expect("a workspace resolves a root");
    assert_eq!(root.to_string_lossy(), workspace_root(&dir));
}

#[test]
fn the_store_is_opened_on_the_resolved_root() {
    let dir = scratch("store");
    jj_repo(&dir);
    let ctx = resolve(&dir).expect("a jj repo resolves");
    let root = ctx.root().expect("a workspace resolves a root");
    let store = ctx.store().expect("a workspace opens a store");
    assert_eq!(store.root(), root);
}

#[test]
fn a_directory_without_a_workspace_resolves_root_less_and_the_accessors_refuse() {
    let dir = scratch("no-repo");
    let ctx = resolve(&dir).expect("a directory without a workspace still resolves");
    assert!(ctx.root().is_err(), "a root-less context has no root");
    assert!(ctx.store().is_err(), "a root-less context has no store");
    let refusal = ctx.root().unwrap_err();
    assert!(
        refusal.cause().contains("workspace"),
        "the refusal names the absence: {refusal:?}"
    );
}
