//! Gate for M-main (the crate root: the module map, the verb table, the assembled entry).
//!
//! Self-contained per M-tests: this file carries its own helpers and fixtures. Black-box per
//! I1-test: assert_cmd drives the built binary, and each case gets a tempfile-scoped scratch
//! directory holding a real jj workspace, so the assembled entry resolves the substrate the
//! same way production does. Resolution is total (S3-root-optional): a directory with no
//! workspace still resolves, so the globals answer and a workspace-needing verb refuses on
//! the root-less context. The map check reads `src/lib.rs` back and compares it with the
//! modules under `src/`.

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::Command as DeccyCommand;
use tempfile::TempDir;

/// The bare list, in table order, with `init` first.
const VERB_LIST: &str = "init\nshow\nlog\nvalidate\nnew\nrecord\nremove\nundo\nredo\n";

/// A scratch jj workspace: the assembled entry resolves jj on PATH and the root at cwd.
fn workspace() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new("jj")
        .args(["git", "init"])
        .current_dir(dir.path())
        .output()
        .expect("jj is on PATH for the gate");
    assert!(
        out.status.success(),
        "jj git init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

/// The built binary, run in `dir` with `args`.
fn deccy(dir: &Path, args: &[&str]) -> DeccyCommand {
    let mut cmd = DeccyCommand::cargo_bin("deccy").unwrap();
    cmd.current_dir(dir).args(args);
    cmd
}

#[test]
fn version_prints_the_build_version() {
    let dir = workspace();
    deccy(dir.path(), &["--version"])
        .assert()
        .success()
        .stdout(format!("{}\n", env!("CARGO_PKG_VERSION")))
        .stderr("");
}

#[test]
fn bare_help_lists_the_verbs() {
    let dir = workspace();
    deccy(dir.path(), &["--help"])
        .assert()
        .success()
        .stdout(VERB_LIST)
        .stderr("");
}

#[test]
fn unknown_verb_refuses_named() {
    let dir = workspace();
    deccy(dir.path(), &["bogus"])
        .assert()
        .code(2)
        .stdout("")
        .stderr("unknown verb: bogus\n");
}

#[test]
fn no_verb_refuses() {
    let dir = workspace();
    deccy(dir.path(), &[])
        .assert()
        .code(2)
        .stdout("")
        .stderr("no verb given\n");
}

#[test]
fn bare_help_works_without_a_workspace() {
    // Resolution is total: a directory with no workspace resolves root-less, so the globals
    // still answer (S3-root-optional).
    let dir = tempfile::tempdir().unwrap();
    deccy(dir.path(), &["--help"])
        .assert()
        .success()
        .stdout(VERB_LIST)
        .stderr("");
}

#[test]
fn a_verb_that_needs_the_workspace_refuses_without_one() {
    // The root-less context is a state, not a resolution failure: a verb that reaches for the
    // store refuses, naming the absence (S3-root-required).
    let dir = tempfile::tempdir().unwrap();
    deccy(dir.path(), &["log"])
        .assert()
        .code(2)
        .stdout("");
}

#[test]
fn init_prepares_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    deccy(dir.path(), &["init"])
        .assert()
        .success()
        .stdout(format!(
            "initialised {}\n",
            fs::canonicalize(dir.path()).unwrap().display()
        ))
        .stderr("");
    assert!(dir.path().join(".git").is_dir(), "a git repository is created");
    assert!(dir.path().join(".jj").is_dir(), "a jj workspace is created");
    assert!(dir.path().join("nodes").is_dir(), "the node directory is created");
    assert!(dir.path().join(".deccy").is_dir(), "the state directory is created");
}

#[test]
fn init_refuses_an_already_initialised_target() {
    let dir = tempfile::tempdir().unwrap();
    deccy(dir.path(), &["init"]).assert().success();
    deccy(dir.path(), &["init"])
        .assert()
        .code(2)
        .stdout("")
        .stderr("the target is already initialised: .jj\n");
}

#[test]
fn the_map_names_exactly_the_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let declared = declared_modules(&fs::read_to_string(root.join("src/lib.rs")).unwrap());
    let present = modules_on_disk(&root.join("src"));
    assert_eq!(
        declared, present,
        "src/lib.rs must declare exactly the modules under src/"
    );
}

/// The `pub mod <name>;` declarations in the crate root, sorted.
fn declared_modules(lib: &str) -> Vec<String> {
    let mut names: Vec<String> = lib
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub mod ")?
                .strip_suffix(';')
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// The module stems of the `.rs` files directly under `src/`, sorted — the crate root and
/// the binary are not modules.
fn modules_on_disk(src: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(src)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if path.extension().map(|ext| ext == "rs").unwrap_or(false) {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                if stem == "lib" || stem == "main" {
                    None
                } else {
                    Some(stem)
                }
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}
