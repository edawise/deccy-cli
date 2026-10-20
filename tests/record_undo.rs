//! Gate for E-record-loop: the real record -> undo -> redo loop.
//!
//! Black-box per I1-test: assert_cmd drives the built binary against a tempfile-scoped jj
//! workspace, so the recording, its marker bracket, the undo apply and the redo apply meet as
//! they do in production. The fixture is the shape a recording leaves — the target and the
//! working copy both hold the after side (D15c, mnwqksmt) — not a hand-built before side.

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::Command as DeccyCommand;
use tempfile::TempDir;

/// A scratch directory; `deccy init` lays the jj workspace down inside it.
fn workspace() -> TempDir {
    tempfile::tempdir().unwrap()
}

/// The built binary, run in `dir` with `args`.
fn deccy(dir: &Path, args: &[&str]) -> DeccyCommand {
    let mut cmd = DeccyCommand::cargo_bin("deccy").unwrap();
    cmd.current_dir(dir).args(args);
    cmd
}

/// The node document the gate writes: a decision with inline content, so the two sides differ
/// by one string and the document validates.
fn node(body: &str) -> String {
    format!(
        "{{\"kind\":\"decision\",\"depends_on\":[\"zzzzzzzz\"],\"content\":{{\"type\":\"text\",\"ref\":\"inline\",\"content\":\"{body}\",\"rationale\":\"{body}\"}}}}"
    )
}

/// `jj ...` in `dir`, returning trimmed stdout; a bad exit fails the gate.
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

/// `deccy new` in `dir`, returning the id it prints.
fn new(dir: &Path, args: &[&str]) -> String {
    let out = deccy(dir, args).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// The parsed marker file.
fn markers(root: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(root.join(".deccy/markers.json")).unwrap()).unwrap()
}

/// The position the markers record: the applied prefix of the operation list.
fn position(root: &Path) -> u64 {
    markers(root)["pos"].as_u64().unwrap()
}

/// The one recording marker this gate makes.
fn recording(root: &Path) -> serde_json::Value {
    markers(root)["markers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|marker| marker["kind"] == "record")
        .cloned()
        .unwrap()
}

#[test]
fn record_undo_redo_returns_and_removes_the_recorded_diff() {
    let dir = workspace();
    let root = dir.path();
    deccy(root, &["init"]).assert().success();

    // A root decision, then a second change to hold the working-copy edit.
    let out = deccy(root, &["new", "-m", "A", "--on", "zzzzzzzz"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let a = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let path = format!("nodes/{a}.json");
    fs::write(root.join(&path), node("one")).unwrap();
    deccy(root, &["new", "-m", "B", "--on", a.as_str()])
        .assert()
        .success();

    // The real recording: the target holds v1, the working copy holds v2, and `deccy record`
    // moves v2 into the target, leaving both sides at the after side.
    fs::write(root.join(&path), node("two")).unwrap();
    deccy(root, &["record", path.as_str(), "--to", a.as_str()])
        .assert()
        .success();
    assert_eq!(
        jj(root, &["file", "show", "-r", a.as_str(), path.as_str()]),
        node("two"),
        "the recording lands the after side in the target"
    );
    assert_eq!(
        fs::read_to_string(root.join(&path)).unwrap(),
        node("two"),
        "the recording leaves the after side in the working copy"
    );

    // Undo: the target returns to the before side, the working copy keeps the after side, and
    // the returned diff shows against the target.
    deccy(root, &["undo"]).assert().success();
    assert_eq!(
        jj(root, &["file", "show", "-r", a.as_str(), path.as_str()]),
        node("one"),
        "undo puts the before side back in the target"
    );
    assert_eq!(
        fs::read_to_string(root.join(&path)).unwrap(),
        node("two"),
        "undo leaves the after side in the working copy"
    );
    assert!(
        jj(root, &["status"]).contains(path.as_str()),
        "the returned diff shows against the target"
    );

    // Redo: the target returns to the after side and the working copy is clean for it.
    deccy(root, &["redo"]).assert().success();
    assert_eq!(
        jj(root, &["file", "show", "-r", a.as_str(), path.as_str()]),
        node("two"),
        "redo puts the after side back in the target"
    );
    assert_eq!(
        fs::read_to_string(root.join(&path)).unwrap(),
        node("two"),
        "redo leaves the working copy at the after side"
    );
    assert!(
        !jj(root, &["status"]).contains(path.as_str()),
        "the recorded diff is gone from the working copy"
    );
}

/// The reported sequence: a creation undone immediately before the recording leaves the
/// working copy an empty change (jj abandons an empty, undescribed one when the next apply
/// moves the checkout), so undoing the recording must restore a fresh empty change on the
/// old parent rather than edit the change jj dropped — and must never leave the target
/// rewritten while the marker still calls the recording applied.
#[test]
fn undo_of_a_recording_survives_the_empty_working_copy_a_creation_undo_leaves() {
    let dir = workspace();
    let root = dir.path();
    deccy(root, &["init"]).assert().success();

    // A root decision at "one", a child B, then A is edited to "two" and really recorded.
    let a = new(root, &["new", "-m", "A", "--on", "zzzzzzzz"]);
    let path = format!("nodes/{a}.json");
    fs::write(root.join(&path), node("one")).unwrap();
    let b = new(root, &["new", "-m", "B", "--on", a.as_str()]);
    fs::write(root.join(&path), node("two")).unwrap();
    deccy(root, &["record", path.as_str(), "--to", a.as_str()])
        .assert()
        .success();

    // A further creation gives the next undo something to take; undoing it leaves the empty
    // working copy the reported failure needs.
    deccy(root, &["new", "-m", "C", "--on", b.as_str()])
        .assert()
        .success();
    deccy(root, &["undo"]).assert().success();
    assert_eq!(
        jj(root, &["log", "-r", "@", "--no-graph", "-T", "if(empty, \"empty\", \"changed\")"]),
        "empty",
        "a creation's undo leaves the working copy an empty change"
    );

    // The recording's undo: the target returns to the before side, the working copy keeps the
    // after side, and the marker is stepped back with the journal entry recorded as the
    // revert. Before the fix this refused because jj had dropped the empty working copy.
    let before = position(root);
    deccy(root, &["undo"]).assert().success();
    assert_eq!(
        jj(root, &["file", "show", "-r", a.as_str(), path.as_str()]),
        node("one"),
        "undo puts the before side back in the target"
    );
    assert_eq!(
        fs::read_to_string(root.join(&path)).unwrap(),
        node("two"),
        "undo leaves the after side in the working copy"
    );
    assert_eq!(position(root), before - 1, "the recording's marker is stepped back");
    let marker = recording(root);
    assert_eq!(
        marker["reverts"], marker["entries"],
        "the journal entry is recorded against the operation as its revert"
    );

    // A further undo takes the creation before the recording: B is gone.
    deccy(root, &["undo"]).assert().success();
    deccy(root, &["show", b.as_str()]).assert().code(2);

    // Redo restores the creation, then the recording: A returns to the after side and the
    // recording's reverts are cleared.
    deccy(root, &["redo"]).assert().success();
    deccy(root, &["redo"]).assert().success();
    assert_eq!(
        jj(root, &["file", "show", "-r", a.as_str(), path.as_str()]),
        node("two"),
        "redo puts the after side back in the target"
    );
    assert_eq!(
        fs::read_to_string(root.join(&path)).unwrap(),
        node("two"),
        "redo leaves the working copy at the after side"
    );
    assert!(
        recording(root)["reverts"].as_array().unwrap().is_empty(),
        "redo clears the reverts recorded against the operation"
    );
}
