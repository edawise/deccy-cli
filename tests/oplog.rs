//! Gate for M-jj-oplog (op log: read the current operation id).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/oplog.rs"]
mod oplog;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use oplog::{oplog, OplogError};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-oplog-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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

#[test]
fn the_current_operation_id_is_read_and_advances() {
    let dir = scratch("oplog");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    let before = oplog(&jjrun).unwrap();
    assert!(!before.is_empty());
    assert!(
        before.chars().all(|c| c.is_ascii_hexdigit()),
        "an operation id should be hex: {before:?}"
    );

    jj(&dir, &["describe", "-m", "an operation"]);
    let after = oplog(&jjrun).unwrap();

    assert_ne!(before, after);
}

#[test]
fn outside_a_repo_the_failure_is_a_value_carrying_its_evidence() {
    let dir = scratch("norepo");
    let jjrun = Jj::new("jj", dir.as_path());

    match oplog(&jjrun) {
        Err(OplogError::Failed(out)) => {
            assert_ne!(out.status, 0);
            assert!(!out.stderr.trim().is_empty());
        }
        other => panic!("expected a carried failure, got {other:?}"),
    }
}

#[test]
fn an_absent_binary_is_a_spawn_failure() {
    let dir = scratch("nobinary");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("/nonexistent/jj", dir.as_path());

    assert!(matches!(oplog(&jjrun), Err(OplogError::Spawn(_))));
}
