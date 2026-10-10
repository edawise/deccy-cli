//! Gate for M-jj-oplog (op log: read the current operation id, the run window's entries,
//! and the invoking workspace).
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
use oplog::{oplog, window, workspace, OplogError};

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
    match workspace(&jjrun) {
        Err(OplogError::Failed(out)) => assert_ne!(out.status, 0),
        other => panic!("expected a carried failure, got {other:?}"),
    }
}

/// The head operation id, read by the test's own jj crossing.
fn op_id(dir: &Path) -> String {
    jj(dir, &["op", "log", "--limit", "1", "--no-graph", "-T", "id"])
}

/// The head operation id, read without snapshotting the working copy.
fn observing_op_id(dir: &Path) -> String {
    jj(
        dir,
        &[
            "op",
            "log",
            "--ignore-working-copy",
            "--limit",
            "1",
            "--no-graph",
            "-T",
            "id",
        ],
    )
}

/// Run `read` against a repo whose working copy holds an unsettled file, and require that the
/// read left the op head where it was (S5-observation): reading the op log must not settle the
/// working copy.
fn assert_observing(name: &str, read: impl Fn(&Jj, &str)) {
    let dir = scratch(name);
    jj(&dir, &["git", "init"]);
    jj(&dir, &["status"]);
    let jjrun = Jj::new("jj", dir.as_path());
    let head = observing_op_id(&dir);
    std::fs::write(dir.join("unsettled"), "not yet snapshotted\n").unwrap();

    read(&jjrun, &head);

    assert_eq!(
        observing_op_id(&dir),
        head,
        "{name} left the working copy unsettled"
    );
}

#[test]
fn the_reads_do_not_settle_the_working_copy() {
    assert_observing("observing-oplog", |jjrun, _head| {
        oplog(jjrun).unwrap();
    });
    assert_observing("observing-window", |jjrun, head| {
        window(jjrun, head, head).unwrap();
    });
    assert_observing("observing-workspace", |jjrun, _head| {
        workspace(jjrun).unwrap();
    });
}

#[test]
fn the_window_holds_the_entries_between_two_head_reads() {
    let dir = scratch("window");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());

    let before = oplog(&jjrun).unwrap();
    jj(&dir, &["describe", "-m", "one"]);
    let first = op_id(&dir);
    jj(&dir, &["describe", "-m", "two"]);
    let second = op_id(&dir);
    let after = oplog(&jjrun).unwrap();

    let entries = window(&jjrun, &before, &after).unwrap();
    assert_eq!(
        entries.iter().map(|entry| entry.id.clone()).collect::<Vec<_>>(),
        vec![first, second],
        "the window holds the entries created in between, oldest first"
    );
    let invoking = workspace(&jjrun).unwrap();
    assert!(
        entries.iter().all(|entry| entry.workspace == invoking),
        "each entry is attributed to the workspace its work ran in"
    );
}

#[test]
fn the_invoking_workspace_is_read() {
    let first = scratch("workspace");
    jj(&first, &["git", "init"]);
    let second = scratch("workspace-second");
    jj(
        &first,
        &[
            "workspace",
            "add",
            "--name",
            "second",
            second.to_str().unwrap(),
        ],
    );

    assert_eq!(
        workspace(&Jj::new("jj", first.as_path())).unwrap(),
        "default",
        "the workspace the runner stands in is the one that is read"
    );
    assert_eq!(
        workspace(&Jj::new("jj", second.as_path())).unwrap(),
        "second"
    );
}

#[test]
fn an_absent_binary_is_a_spawn_failure() {
    let dir = scratch("nobinary");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("/nonexistent/jj", dir.as_path());

    assert!(matches!(oplog(&jjrun), Err(OplogError::Spawn(_))));
}
