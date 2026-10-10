//! Gate for M-jj-new (creation: insert a change after the newest named parent).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/jj_new.rs"]
mod jj_new;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use jj_new::{open, OpenError};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-jjnew-{}-{}", std::process::id(), name));
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

fn description(dir: &Path, rev: &str) -> String {
    jj(dir, &["log", "-r", rev, "--no-graph", "-T", "description.first_line()"])
}

fn parents(dir: &Path, rev: &str) -> Vec<String> {
    jj(
        dir,
        &[
            "log",
            "-r",
            &format!("parents({rev})"),
            "--no-graph",
            "-T",
            "change_id.shortest(8) ++ \"\\n\"",
        ],
    )
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_string)
    .collect()
}

/// Whether `id` is among the revisions reachable from `rev` (its ancestors and itself).
fn reachable(dir: &Path, rev: &str, id: &str) -> bool {
    jj(
        dir,
        &[
            "log",
            "-r",
            &format!("::{rev}"),
            "--no-graph",
            "-T",
            "change_id.shortest(8) ++ \"\\n\"",
        ],
    )
    .lines()
    .any(|line| line == id)
}

#[test]
fn a_change_is_opened_after_the_named_parent_with_the_summary() {
    let dir = scratch("single");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let base = change_id(&dir, "@");

    let out = open(&jjrun, "child change", &[&base]).unwrap();

    assert_eq!(out.status, 0);
    assert_eq!(description(&dir, "@"), "child change");
    assert_eq!(parents(&dir, "@"), [base]);
}

#[test]
fn the_opening_inserts_after_the_newest_of_several_named_parents() {
    let dir = scratch("newest");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let a = change_id(&dir, "@");
    jj(&dir, &["describe", "-m", "a"]);
    jj(&dir, &["new", "-m", "b"]);
    let b = change_id(&dir, "@");

    // b is a's child: the newest of {a, b} is b, so the new change goes after b.
    let out = open(&jjrun, "inserted", &[&a, &b]).unwrap();

    assert_eq!(out.status, 0);
    assert_eq!(description(&dir, "@"), "inserted");
    assert_eq!(
        parents(&dir, "@"),
        [b.clone()],
        "positioned after the newest dependency, not merged onto both"
    );
    assert!(
        reachable(&dir, "@", &a),
        "the older dependency is an ancestor of the new change"
    );
}

#[test]
fn the_opening_does_not_branch_when_the_position_has_a_child() {
    let dir = scratch("nobranch");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    jj(&dir, &["describe", "-m", "a"]);
    jj(&dir, &["new", "-m", "b"]);
    let b = change_id(&dir, "@");
    jj(&dir, &["new", "-m", "c"]);
    let c = change_id(&dir, "@");

    // b already has a child (c); inserting after b rebases c onto the new change
    // instead of leaving a sibling behind.
    let out = open(&jjrun, "inserted", &[&b]).unwrap();

    assert_eq!(out.status, 0);
    let inserted = change_id(&dir, "@");
    assert_eq!(parents(&dir, "@"), [b], "inserted after the position");
    assert_eq!(
        parents(&dir, &c),
        [inserted],
        "the position's existing child rebased onto the insertion"
    );
}

#[test]
fn a_forked_parent_set_is_non_linear_not_a_panic() {
    let dir = scratch("forked");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let base = change_id(&dir, "@");
    jj(&dir, &["describe", "-m", "base"]);
    jj(&dir, &["new", "-m", "left"]);
    let left = change_id(&dir, "@");
    jj(&dir, &["new", &base, "-m", "right"]);
    let right = change_id(&dir, "@");

    // left and right diverge off base: two heads in jj history, so no unique position.
    match open(&jjrun, "merge", &[&left, &right]) {
        Err(OpenError::NonLinear(ids)) => {
            assert!(ids.contains(&left), "{ids:?} names the left head");
            assert!(ids.contains(&right), "{ids:?} names the right head");
        }
        other => panic!("a forked parent set must be a refusal value, got {other:?}"),
    }
}

#[test]
fn a_bad_parent_is_a_failure_value_not_a_panic() {
    let dir = scratch("bad");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    // `zzzzzzzz` would resolve: the jj root change id is all z's.
    let out = open(&jjrun, "x", &["qqqqqqqq"]).unwrap();

    assert_ne!(out.status, 0);
    assert!(!out.stderr.trim().is_empty());
}
