//! Gate for M-jj-revert (revert: take a named op-log entry's work out of the log).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — `/bin/sh` stands in
//! for jj to prove the argv, and a real jj in a scratch repo proves the effect.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/revert.rs"]
mod revert;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use jj::{Jj, JjError};
use revert::revert;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-revert-{}-{}", std::process::id(), name));
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
fn the_revert_argv_is_op_revert_with_the_entry_id() {
    let dir = scratch("argv");
    // `/bin/sh` stands in for jj. The layer's first arg (`op`) names this script, so the
    // script's positional parameters are exactly the rest of the argv the layer built.
    fs::write(dir.join("op"), "printf '%s\\n' \"$@\"\n").unwrap();

    let out = revert(&Jj::new("/bin/sh", dir.as_path()), &["the-entry-id".to_string()]).unwrap();

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].status, 0);
    assert_eq!(out[0].stdout, "revert\nthe-entry-id\n");
}

#[test]
fn a_revert_takes_the_named_entry_out_of_the_repo_state() {
    let dir = scratch("revert");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    jj(&dir, &["describe", "-m", "an operation"]);
    let entry = op_id(&dir);

    let out = revert(&jjrun, &[entry]).unwrap();

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].status, 0);
    // the operation's description is out of the repo state again
    assert_eq!(
        jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "description.first_line()"]),
        ""
    );
}

#[test]
fn entries_are_reverted_in_the_order_given() {
    let dir = scratch("order");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    jj(&dir, &["describe", "-m", "one"]);
    let first = op_id(&dir);
    jj(&dir, &["describe", "-m", "two"]);
    let second = op_id(&dir);
    assert_ne!(first, second, "a second operation advances the op head");

    let out = revert(&jjrun, &[second, first]).unwrap();

    assert_eq!(
        out.iter().map(|crossing| crossing.status).collect::<Vec<_>>(),
        [0, 0]
    );
    assert_eq!(
        jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "description.first_line()"]),
        ""
    );
}

#[test]
fn a_bad_entry_id_is_a_failure_value_not_a_panic() {
    let dir = scratch("bad");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    let out = revert(&jjrun, &["qqqqqqqq".to_string()]).unwrap();

    assert_ne!(out[0].status, 0);
    assert!(!out[0].stderr.trim().is_empty());
}

#[test]
fn an_absent_binary_is_a_spawn_failure() {
    let dir = scratch("nobinary");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("/nonexistent/jj", dir.as_path());

    match revert(&jjrun, &["whatever".to_string()]) {
        Err(JjError::Spawn(reason)) => assert!(!reason.is_empty()),
        Ok(_) => panic!("a missing binary must not look like a crossing"),
    }
}
