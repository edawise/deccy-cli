//! Gate for M-jj (the seam's runner).
//!
//! Self-contained per M-tests: the shim declares the module under test; no shared
//! test-support module. Plain `rustc --test`, std only — `/bin/sh` stands in for jj so the
//! runner's process mechanics are exercised without a real repo.

#[path = "../src/jj.rs"]
mod jj;

use std::fs;
use std::path::PathBuf;

use jj::{Jj, JjError};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-jj-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_crossing_captures_stdout_and_passes_args_untouched() {
    let dir = scratch("capture");
    let out = Jj::new("/bin/sh", dir.as_path())
        .run(&["-c", "printf 'out:%s\\n' \"$@\"", "sh", "log", "-r", "@"])
        .unwrap();
    assert_eq!(out.status, 0);
    assert_eq!(out.stdout, "out:log\nout:-r\nout:@\n");
    assert_eq!(out.stderr, "");
}

#[test]
fn a_nonzero_status_is_captured_with_its_stderr() {
    let dir = scratch("failure");
    let out = Jj::new("/bin/sh", dir.as_path())
        .run(&["-c", "echo bad 1>&2; exit 2"])
        .unwrap();
    assert_eq!(out.status, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(out.stderr, "bad\n");
}

#[test]
fn the_crossing_runs_in_the_supplied_root() {
    let dir = scratch("root");
    let sub = dir.join("nested");
    fs::create_dir(&sub).unwrap();
    let out = Jj::new("/bin/sh", sub.as_path()).run(&["-c", "pwd"]).unwrap();
    assert_eq!(
        fs::canonicalize(out.stdout.trim()).unwrap(),
        fs::canonicalize(&sub).unwrap()
    );
}

#[test]
fn a_missing_binary_fails_carrying_its_evidence() {
    let dir = scratch("missing");
    let jj = Jj::new(dir.join("nope"), dir.as_path());
    match jj.run(&["status"]) {
        Err(JjError::Spawn(reason)) => assert!(!reason.is_empty()),
        Ok(_) => panic!("a missing binary must not look like a crossing"),
    }
}
