//! Gate for M-jj-place (placement: move working-copy edits into an older change).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only — it runs a real jj
//! in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/jj_place.rs"]
mod jj_place;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use jj_place::place;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-jjplace-{}-{}", std::process::id(), name));
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
fn a_working_copy_edit_moves_into_the_named_change() {
    let dir = scratch("place");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let target = change_id(&dir, "@");
    jj(&dir, &["describe", "-m", "target"]);
    // A fresh source with no description, so squash never prompts for a combined message.
    jj(&dir, &["new"]);
    std::fs::write(dir.join("f.txt"), "hello").unwrap();

    let out = place(&jjrun, &target, &["f.txt".to_string()]).unwrap();

    assert_eq!(out.status, 0);
    assert_eq!(jj(&dir, &["file", "show", "-r", &target, "f.txt"]), "hello");
    assert_eq!(jj(&dir, &["diff", "-r", "@"]), "");
}

#[test]
fn a_bad_id_is_a_failure_value_not_a_panic() {
    let dir = scratch("bad");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    std::fs::write(dir.join("f.txt"), "hello").unwrap();

    let out = place(&jjrun, "qqqqqqqq", &["f.txt".to_string()]).unwrap();

    assert_ne!(out.status, 0);
    assert!(!out.stderr.trim().is_empty());
}

#[test]
fn the_pathset_narrows_the_move_to_the_named_files() {
    let dir = scratch("narrow");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let target = change_id(&dir, "@");
    jj(&dir, &["describe", "-m", "target"]);
    // A fresh source with no description, so squash never prompts for a combined message.
    jj(&dir, &["new"]);
    std::fs::write(dir.join("a.txt"), "one").unwrap();
    std::fs::write(dir.join("b.txt"), "two").unwrap();

    let out = place(&jjrun, &target, &["a.txt".to_string()]).unwrap();

    assert_eq!(out.status, 0);
    assert_eq!(jj(&dir, &["file", "show", "-r", &target, "a.txt"]), "one");
    // The unnamed edit did not move: it still shows in the source's working copy.
    let remaining = jj(&dir, &["diff", "-r", "@"]);
    assert!(remaining.contains("b.txt"), "b.txt should remain: {remaining}");
    assert!(!remaining.contains("a.txt"), "a.txt should have moved: {remaining}");
}
