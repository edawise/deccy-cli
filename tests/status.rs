//! Gate for M-jj-status (status: read the working-copy changed-path set).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/status.rs"]
mod status;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use status::{status, StatusError};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-status-{}-{}", std::process::id(), name));
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
fn the_edited_paths_are_read() {
    let dir = scratch("edited");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    std::fs::write(dir.join("b.txt"), "b").unwrap();

    let mut paths = status(&jjrun).unwrap();
    paths.sort();

    assert_eq!(paths, ["a.txt".to_string(), "b.txt".to_string()]);
}

#[test]
fn a_file_rename_is_read_as_both_paths() {
    let dir = scratch("rename");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    // Track it, so the move below is a rename rather than an add.
    jj(&dir, &["new", "-m", "base"]);
    std::fs::rename(dir.join("a.txt"), dir.join("b.txt")).unwrap();

    let mut paths = status(&jjrun).unwrap();
    paths.sort();

    assert_eq!(paths, ["a.txt".to_string(), "b.txt".to_string()]);
}

#[test]
fn a_directory_rename_is_read_as_both_full_paths() {
    let dir = scratch("dirrename");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    std::fs::create_dir_all(dir.join("d1/src")).unwrap();
    std::fs::write(dir.join("d1/src/lib.rs"), "x").unwrap();
    jj(&dir, &["new", "-m", "base"]);
    std::fs::rename(dir.join("d1"), dir.join("d2")).unwrap();

    let mut paths = status(&jjrun).unwrap();
    paths.sort();

    // jj names the move once, in the splice form `{d1 => d2}/src/lib.rs`.
    assert_eq!(
        paths,
        ["d1/src/lib.rs".to_string(), "d2/src/lib.rs".to_string()]
    );
}

#[test]
fn a_clean_working_copy_has_no_changed_paths() {
    let dir = scratch("clean");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    assert!(status(&jjrun).unwrap().is_empty());
}

#[test]
fn outside_a_repo_the_failure_is_a_value_carrying_its_evidence() {
    let dir = scratch("norepo");
    let jjrun = Jj::new("jj", dir.as_path());

    match status(&jjrun) {
        Err(StatusError::Failed(out)) => {
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

    assert!(matches!(status(&jjrun), Err(StatusError::Spawn(_))));
}
