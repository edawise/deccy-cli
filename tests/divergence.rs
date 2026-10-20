//! Gate for M-jj-divergence (divergence: read whether any change is divergent).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/divergence.rs"]
mod divergence;

use std::path::{Path, PathBuf};
use std::process::Command;

use divergence::{divergent, DivergenceError};
use jj::Jj;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "deccy-divergence-{}-{}",
        std::process::id(),
        name
    ));
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

fn op_head(dir: &Path) -> String {
    jj(dir, &["op", "log", "--limit", "1", "--no-graph", "-T", "id"])
}

#[test]
fn a_clean_working_copy_is_not_divergent() {
    let dir = scratch("clean");
    jj(&dir, &["git", "init"]);
    jj(&dir, &["new", "-m", "base"]);
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let jjrun = Jj::new("jj", dir.as_path());

    assert!(!divergent(&jjrun).unwrap());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_squash_reverted_under_an_amended_working_copy_is_divergent() {
    let dir = scratch("divergent");
    jj(&dir, &["git", "init"]);
    jj(&dir, &["new", "-m", "A"]);
    std::fs::write(dir.join("f.txt"), "v1").unwrap();
    let base = jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"]);
    jj(&dir, &["new", "-m", "B"]);
    std::fs::write(dir.join("nb.txt"), "b").unwrap();
    std::fs::write(dir.join("f.txt"), "v2").unwrap();

    let pre = op_head(&dir);
    jj(
        &dir,
        &[
            "squash",
            "--into",
            &base,
            "--use-destination-message",
            "--",
            "f.txt",
        ],
    );
    let squash = op_head(&dir);
    assert_ne!(pre, squash, "the squash is one operation");

    // Amend the working-copy commit after the squash, then revert the squash: jj splits the
    // working copy into two versions of one change.
    std::fs::write(dir.join("dirty.txt"), "uncommitted").unwrap();
    jj(&dir, &["status"]);
    jj(&dir, &["op", "revert", &squash]);

    let jjrun = Jj::new("jj", dir.as_path());
    assert!(divergent(&jjrun).unwrap());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn outside_a_repo_the_failure_is_a_value_carrying_its_evidence() {
    let dir = scratch("norepo");
    let jjrun = Jj::new("jj", dir.as_path());

    match divergent(&jjrun) {
        Err(DivergenceError::Failed(out)) => {
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

    assert!(matches!(divergent(&jjrun), Err(DivergenceError::Spawn(_))));
}
