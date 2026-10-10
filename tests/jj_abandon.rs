//! Gate for M-jj-abandon (removal: abandon a change).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/jj_abandon.rs"]
mod jj_abandon;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use jj_abandon::abandon;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-jjabandon-{}-{}", std::process::id(), name));
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

/// The first line of every visible change's description.
fn descriptions(dir: &Path) -> Vec<String> {
    jj(
        dir,
        &["log", "-r", "all()", "--no-graph", "-T", "description.first_line() ++ \"\\n\""],
    )
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_string)
    .collect()
}

#[test]
fn the_named_change_is_abandoned() {
    let dir = scratch("abandon");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let victim = change_id(&dir, "@");
    jj(&dir, &["describe", "-m", "victim"]);

    let out = abandon(&jjrun, &victim).unwrap();

    assert_eq!(out.status, 0);
    assert!(!descriptions(&dir).contains(&"victim".to_string()));
}

#[test]
fn abandoning_a_parent_reparents_its_child() {
    let dir = scratch("reparent");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let parent = change_id(&dir, "@");
    jj(&dir, &["describe", "-m", "parent"]);
    jj(&dir, &["new", "-m", "child"]);

    let out = abandon(&jjrun, &parent).unwrap();

    assert_eq!(out.status, 0);
    let descs = descriptions(&dir);
    assert!(descs.contains(&"child".to_string()));
    assert!(!descs.contains(&"parent".to_string()));
}

#[test]
fn a_bad_id_is_a_failure_value_not_a_panic() {
    let dir = scratch("bad");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    let out = abandon(&jjrun, "qqqqqqqq").unwrap();

    assert_ne!(out.status, 0);
    assert!(!out.stderr.trim().is_empty());
}
