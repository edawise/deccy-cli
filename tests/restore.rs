//! Gate for M-jj-restore (restore: restore a whole deccy operation).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — `/bin/sh` stands in
//! for jj to prove the argv, and a real jj in a scratch repo proves the effect.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/restore.rs"]
mod restore;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use jj::{Jj, JjError};
use restore::restore;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-restore-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run a real jj in `dir` and return trimmed stdout, failing the test on a bad exit.
fn jj(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("jj")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("jj is on PATH for the gate");
    assert!(
        out.status.success(),
        "jj {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// The current (head) operation id.
fn op_id(dir: &Path) -> String {
    jj(dir, &["op", "log", "--no-graph", "--limit", "1", "-T", "id"])
}

#[test]
fn the_restore_argv_is_op_restore_with_the_operation_id() {
    let dir = scratch("argv");
    // `/bin/sh` stands in for jj. The layer's first arg (`op`) names this script, so the
    // script's positional parameters are exactly the rest of the argv the layer built.
    fs::write(dir.join("op"), "printf '%s\\n' \"$@\"\n").unwrap();

    let out = restore(&Jj::new("/bin/sh", dir.as_path()), "the-operation-id").unwrap();

    assert_eq!(out.status, 0);
    assert_eq!(out.stdout, "restore\nthe-operation-id\n");
}

#[test]
fn a_restore_returns_the_repo_to_the_boundary_operation() {
    let dir = scratch("restore");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let boundary = op_id(&dir);

    fs::write(dir.join("f.txt"), "hello").unwrap();
    jj(&dir, &["describe", "-m", "an operation"]);
    assert_ne!(op_id(&dir), boundary, "an operation should advance the op head");

    let out = restore(&jjrun, &boundary).unwrap();

    assert_eq!(out.status, 0);
    // back at the boundary: no working-copy changes and no description
    assert_eq!(jj(&dir, &["diff", "--summary", "-r", "@"]), "");
    assert_eq!(
        jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "description.first_line()"]),
        ""
    );
}

#[test]
fn a_bad_operation_id_is_a_failure_value_not_a_panic() {
    let dir = scratch("bad");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    let out = restore(&jjrun, "qqqqqqqq").unwrap();

    assert_ne!(out.status, 0);
    assert!(!out.stderr.trim().is_empty());
}

#[test]
fn an_absent_binary_is_a_spawn_failure() {
    let dir = scratch("nobinary");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("/nonexistent/jj", dir.as_path());

    match restore(&jjrun, "whatever") {
        Err(JjError::Spawn(reason)) => assert!(!reason.is_empty()),
        Ok(_) => panic!("a missing binary must not look like a crossing"),
    }
}
