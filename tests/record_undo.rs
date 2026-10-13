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

/// The creation marker for the change `target` names.
fn creation(root: &Path, target: &str) -> serde_json::Value {
    markers(root)["markers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|marker| marker["kind"] == "create" && marker["target"] == target)
        .cloned()
        .expect("a creation marker names the change it created")
}

/// The ids a marker field holds.
fn ids(field: &serde_json::Value) -> Vec<String> {
    field
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect()
}

/// Every change the graph still holds.
fn changes(root: &Path) -> Vec<String> {
    jj(
        root,
        &[
            "log",
            "-r",
            "all()",
            "--no-graph",
            "-T",
            "change_id.shortest(8) ++ \"\\n\"",
        ],
    )
    .lines()
    .map(|id| id.trim().to_string())
    .filter(|id| !id.is_empty())
    .collect()
}

/// The change the working copy stands at.
fn working_copy(root: &Path) -> String {
    jj(
        root,
        &[
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            "change_id.shortest(8)",
        ],
    )
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

/// A creation undone over a recording, and the recording undone over that landing. A creation's
/// undo is the operation revert of the entries the marker kept, so the created change is removed
/// with its document gone and the position steps back to the change the birth was placed beside
/// (zzntzlpq, oqtomppq) — that change stands with its own document, so the working copy there
/// carries content: the empty shape holds only where the change beside the birth is empty, which
/// this fixture's is not. The recording's undo then applies the before side to the target while
/// the after side stays in the working copy, and records the journal entry as the operation's
/// revert. The further undo takes the creation the recording's rewrite sits below: a revert that
/// leaves a change with more than one commit is itself reverted, the reverts already made are
/// reverted, and the invocation refuses naming the entry, with the pre-undo state restored
/// (S5-revert-divergent, ppytsptl).
#[test]
fn undo_of_a_recording_lands_at_the_change_a_creation_undo_returns_to() {
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

    // A further creation placed beside B, whose own document is its content — so the change the
    // birth is placed beside is not empty and the landing below has content of its own.
    let c = new(root, &["new", "-m", "C", "--on", b.as_str()]);

    // The creation's undo: the created change is removed with its document, and the working copy
    // is back at the change the birth was placed beside.
    deccy(root, &["undo"]).assert().success();
    let standing = changes(root);
    assert!(
        !standing.contains(&c),
        "the created change is removed, got {standing:?}"
    );
    assert!(
        standing.contains(&b),
        "the change the birth was placed beside stands, got {standing:?}"
    );
    assert_eq!(
        working_copy(root),
        b,
        "the working copy is back at the change the birth was placed beside"
    );
    assert!(
        !root.join(format!("nodes/{c}.json")).exists(),
        "the created change's document is gone"
    );
    assert_eq!(
        jj(
            root,
            &[
                "log",
                "-r",
                "@",
                "--no-graph",
                "-T",
                "if(empty, \"empty\", \"changed\")"
            ]
        ),
        "changed",
        "the change the birth was placed beside carries its own document"
    );
    assert_eq!(
        jj(root, &["diff", "-r", "@", "--summary"]),
        format!("A nodes/{b}.json"),
        "the landing is the beside change's own content and nothing of the created change"
    );

    // The recording's undo over that landing: the target returns to the before side, the working
    // copy keeps the after side, and the journal entry is recorded as the operation's revert.
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
    assert_eq!(
        position(root),
        before - 1,
        "the recording's marker is stepped back"
    );
    let marker = recording(root);
    assert_eq!(
        marker["reverts"], marker["entries"],
        "the journal entry is recorded against the operation as its revert"
    );

    // The further undo takes the creation the recording's rewrite sits below: its reverted entries
    // would leave a change with more than one commit, so the revert is itself reverted and the
    // invocation refuses, naming the entry, with the pre-undo state restored.
    let kept = ids(&creation(root, b.as_str())["entries"]);
    assert!(!kept.is_empty(), "a creation keeps its own crossing");
    let held = position(root);
    let refused = deccy(root, &["undo"]).output().unwrap();
    assert_eq!(
        refused.status.code(),
        Some(2),
        "the further undo refuses: {refused:?}"
    );
    let named = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(
        kept.iter().any(|entry| named.contains(entry.as_str())),
        "the refusal names the entry it could not revert, got {named:?} among {kept:?}"
    );
    assert_eq!(
        position(root),
        held,
        "the refused undo leaves the position where it was"
    );
    assert!(
        creation(root, b.as_str())["reverts"]
            .as_array()
            .unwrap()
            .is_empty(),
        "the reverts the refused undo made are reverted, so none stay recorded"
    );
    assert_eq!(
        jj(root, &["file", "show", "-r", a.as_str(), path.as_str()]),
        node("one"),
        "the target keeps the before side the recording's undo left"
    );
    assert_eq!(
        fs::read_to_string(root.join(&path)).unwrap(),
        node("two"),
        "the working copy keeps the after side"
    );
    assert!(
        changes(root).contains(&b),
        "the creation the refused undo would have taken stands"
    );
    assert_eq!(
        jj(
            root,
            &[
                "log",
                "-r",
                "divergent()",
                "--no-graph",
                "-T",
                "commit_id"
            ]
        ),
        "",
        "the reverted revert leaves no divergent change"
    );

    // Redo restores the recording: the after side returns to the target and the reverts recorded
    // against the operation are cleared.
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
    assert!(
        recording(root)["reverts"].as_array().unwrap().is_empty(),
        "redo clears the reverts recorded against the operation"
    );
}
