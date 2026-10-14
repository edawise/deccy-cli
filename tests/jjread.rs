//! Gate for M-jj-read (read a file's contents at a named revision).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/jjread.rs"]
mod jjread;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use jjread::{read, ReadError};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-jjread-{}-{}", std::process::id(), name));
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
fn a_file_at_a_revision_is_read_back() {
    let dir = scratch("read");
    jj(&dir, &["git", "init"]);
    std::fs::write(dir.join("note.txt"), "hello\n").unwrap();
    jj(&dir, &["st"]);
    let jjrun = Jj::new("jj", dir.as_path());
    assert_eq!(read(&jjrun, "note.txt", "@").unwrap(), "hello\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_path_is_a_carried_failure() {
    let dir = scratch("missing");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    match read(&jjrun, "absent.txt", "@") {
        Err(ReadError::Failed(out)) => assert_ne!(out.status, 0, "a missing path must fail"),
        other => panic!("expected a carried failure, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_absent_binary_is_a_spawn_failure() {
    let dir = scratch("spawn");
    let jjrun = Jj::new("deccy-no-such-jj-binary", dir.as_path());
    assert!(matches!(
        read(&jjrun, "note.txt", "@"),
        Err(ReadError::Spawn(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
