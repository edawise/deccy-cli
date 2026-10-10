//! Gate for M-jj-log (log: list changes with their descriptions).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/jjlog.rs"]
mod jjlog;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use jjlog::{log, LogError};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-jjlog-{}-{}", std::process::id(), name));
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

fn change_id(dir: &Path, rev: &str) -> String {
    jj(dir, &["log", "-r", rev, "--no-graph", "-T", "change_id.shortest(8)"])
}

#[test]
fn each_change_is_listed_with_its_description() {
    let dir = scratch("log");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let alpha = change_id(&dir, "@");
    jj(&dir, &["describe", "-m", "alpha"]);
    jj(&dir, &["new", "-m", "beta"]);
    let beta = change_id(&dir, "@");

    let entries = log(&jjrun).unwrap();

    assert!(entries.contains(&(alpha, "alpha".to_string())));
    assert!(entries.contains(&(beta, "beta".to_string())));
}

#[test]
fn a_change_without_a_description_lists_with_an_empty_description() {
    let dir = scratch("nodesc");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let id = change_id(&dir, "@"); // a fresh working-copy change has no description

    let entries = log(&jjrun).unwrap();

    assert!(entries.contains(&(id, String::new())));
}

#[test]
fn outside_a_repo_the_failure_is_a_value_carrying_its_evidence() {
    let dir = scratch("norepo");
    let jjrun = Jj::new("jj", dir.as_path());

    match log(&jjrun) {
        Err(LogError::Failed(out)) => {
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

    assert!(matches!(log(&jjrun), Err(LogError::Spawn(_))));
}
