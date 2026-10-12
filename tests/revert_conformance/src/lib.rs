//! Conformance gate for the revert half of undo and redo (M-V-undo, zzntzlpq; M-V-redo,
//! lmwqvpll).
//!
//! The two marker models fix what each revert lands the repo on — undo the entries of the
//! operation it names (specs/markers/undo.qnt, nrqwpwkq), redo the reverts recorded against
//! the next undone operation (specs/markers/redo.qnt, zovzqmon) — and the two handlers carry
//! that to the revert module. This gate drives each handler over a scratch jj workspace whose
//! marker file names real operation ids read from the op log, asserting both the repo's
//! observable state and the persisted state each invocation leaves. Self-contained per
//! M-tests (knqkxzns): the shims pull in both handlers and the siblings they call, mirroring
//! the crate root. It reaches the marker file through M-markers-persist, so it is a crate of
//! its own, needing serde, serde_json and the schema validator. M-layout (szmuyqrq) places
//! the crate flat under tests/; I1-serde (orrvsxqq) fixes the JSON layer the crate is written
//! against and I1-schema (zmmyomyp) the validator the marker documents are checked with.

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/divergence.rs"]
mod divergence;
#[path = "../../../src/jj_abandon.rs"]
mod jj_abandon;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/markers.rs"]
mod markers;
#[path = "../../../src/markers_file.rs"]
mod markers_file;
#[path = "../../../src/markers_persist.rs"]
mod markers_persist;
#[path = "../../../src/markers_guards.rs"]
mod markers_guards;
#[path = "../../../src/oplog.rs"]
mod oplog;
#[path = "../../../src/journal.rs"]
mod journal;
#[path = "../../../src/journal_apply.rs"]
mod journal_apply;
#[path = "../../../src/revert.rs"]
mod revert;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_undo.rs"]
mod verb_undo;
#[path = "../../../src/verb_redo.rs"]
mod verb_redo;

use crate::context::Context;
use crate::route::Verb;

/// Each handler is the table's shape: a Verb row for `undo` and one for `redo` (M-route,
/// ynvwvzrm) compile.
const UNDO_ROW: Verb<Context> = Verb {
    name: "undo",
    run: crate::verb_undo::run,
};

const REDO_ROW: Verb<Context> = Verb {
    name: "redo",
    run: crate::verb_redo::run,
};

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::context::resolve;
    use crate::jj::Jj;
    use crate::markers::Markers;
    use crate::markers_persist;
    use crate::oplog;
    use crate::refusal::Refusal;
    use crate::store::Store;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-revert-conformance-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
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

    /// A scratch jj workspace with an empty `nodes/` (role="a real workspace"); the schemas are
    /// carried in the binary (`include_str!`), never read from the repo. The fixture trees stay
    /// out of jj's tracking: the marker file is ignored, so the batch's operations are the only
    /// ones the op log records.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".gitignore"), "nodes/\n.deccy/\n").unwrap();
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        dir
    }

    /// Write `contents` to `path` in the working copy (jj snapshots it).
    fn edit(dir: &Path, path: &str, contents: &str) {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, contents).unwrap();
    }

    fn change_id(dir: &Path) -> String {
        jj(dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"])
    }

    fn op_head(dir: &Path) -> String {
        jj(dir, &["op", "log", "--limit", "1", "--no-graph", "-T", "id"])
    }

    /// The op-log entries the run window between two head reads carries, oldest first.
    fn window(dir: &Path, pre: &str, post: &str) -> Vec<String> {
        oplog::window(&Jj::new("jj", dir), pre, post)
            .expect("the op log reads")
            .into_iter()
            .map(|entry| entry.id)
            .collect()
    }

    /// Write the marker document verbatim: the position rule is the guards' rule, not the
    /// schema's, so an out-of-range `pos` still passes the schema.
    fn write_markers(dir: &Path, text: &str) {
        fs::write(dir.join(".deccy/markers.json"), text).unwrap()
    }

    /// A schema-valid marker document holding one marker: the operation's kept entry ids, the
    /// reverts recorded against it, and the position.
    fn one_marker(entries: &[String], reverts: &[String], pos: u64) -> String {
        serde_json::json!({
            "markers": [{"kind": "remove", "target": "kqxyzmrn", "entries": entries, "reverts": reverts}],
            "pos": pos,
        })
        .to_string()
    }

    /// Run a real batch — write `notes.txt`, describe, then `new` — between two op-head reads,
    /// persist a marker file naming the run window's entries, and return them, oldest first.
    fn perform(dir: &Path) -> Vec<String> {
        let pre = op_head(dir);
        edit(dir, "notes.txt", "the batch\n");
        jj(dir, &["describe", "-m", "batch"]);
        jj(dir, &["new"]);
        let post = op_head(dir);
        let entries = window(dir, &pre, &post);
        write_markers(dir, &one_marker(&entries, &[], 1));
        entries
    }

    /// Two real batches — each writes its own file, describes, then `new` — and a marker file
    /// naming both windows, nothing undone. Returns the two entry lists, oldest first.
    fn perform_two(dir: &Path) -> (Vec<String>, Vec<String>) {
        let h0 = op_head(dir);
        edit(dir, "one.txt", "one\n");
        jj(dir, &["describe", "-m", "batch one"]);
        jj(dir, &["new"]);
        let h1 = op_head(dir);
        edit(dir, "two.txt", "two\n");
        jj(dir, &["describe", "-m", "batch two"]);
        jj(dir, &["new"]);
        let h2 = op_head(dir);
        let one = window(dir, &h0, &h1);
        let two = window(dir, &h1, &h2);
        let text = serde_json::json!({
            "markers": [
                {"kind": "remove", "target": "kqxyzmrn", "entries": one, "reverts": []},
                {"kind": "remove", "target": "rnkqxyzm", "entries": two, "reverts": []},
            ],
            "pos": 2,
        })
        .to_string();
        write_markers(dir, &text);
        (one, two)
    }

    /// The marker state read back through the persistence layer.
    fn state(dir: &Path) -> Markers {
        markers_persist::load(&Store::new(dir)).expect("the marker file loads")
    }

    /// `deccy undo <args>` through the real handler over the scratch workspace.
    fn undo(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repo resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        crate::verb_undo::run(&context, &owned)
    }

    /// `deccy redo <args>` through the real handler over the scratch workspace.
    fn redo(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repo resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        crate::verb_redo::run(&context, &owned)
    }

    #[test]
    fn undo_lands_the_operation_the_model_names() {
        let dir = repo("undo");
        let entries = perform(&dir);
        assert!(dir.join("notes.txt").exists(), "the batch wrote its file");
        let lines = undo(&dir, &[]).expect("the applied operation undoes");
        assert!(lines.is_empty(), "nothing on stdout");
        assert!(
            !dir.join("notes.txt").exists(),
            "the repo is at the operation's pre-state"
        );
        let log = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(
            !log.contains("batch"),
            "its change is gone from jj log, got {log:?}"
        );
        let state = state(&dir);
        assert_eq!(state.pos, 0, "the position steps back one");
        assert_eq!(state.markers.len(), 1, "the markers stand");
        assert_eq!(state.markers[0].entries, entries, "the operation stands");
        assert_eq!(
            state.markers[0].reverts.len(),
            entries.len(),
            "the reverts the invocation made are written"
        );
        assert_eq!(crate::UNDO_ROW.name, "undo");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_lands_the_operation_the_model_names() {
        let dir = repo("redo");
        let entries = perform(&dir);
        undo(&dir, &[]).expect("the applied operation undoes");
        assert!(
            !dir.join("notes.txt").exists(),
            "the batch's file is gone from the working copy"
        );
        let lines = redo(&dir, &[]).expect("the undone operation redoes");
        assert!(lines.is_empty(), "nothing on stdout");
        assert!(
            dir.join("notes.txt").exists(),
            "the repo is back at the operation"
        );
        let state = state(&dir);
        assert_eq!(state.pos, 1, "the position steps forward one");
                assert_eq!(state.markers.len(), 1, "the markers stand");
        assert_eq!(
            state.markers[0].entries, entries,
            "the operation stands"
        );
        assert!(
            state.markers[0].reverts.is_empty(),
            "the reverts it reverted are cleared"
        );
        assert_eq!(crate::REDO_ROW.name, "redo");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_walk_follows_the_model_index() {
        let dir = repo("walk");
        perform_two(&dir);
        assert!(
            dir.join("one.txt").exists() && dir.join("two.txt").exists(),
            "both batches are applied"
        );
        undo(&dir, &[]).expect("the first undo answers");
        assert!(
            dir.join("one.txt").exists() && !dir.join("two.txt").exists(),
            "the repo is at the second operation's pre-state"
        );
        assert_eq!(state(&dir).pos, 1, "one operation is undone");
        undo(&dir, &[]).expect("the second undo answers");
        assert!(
            !dir.join("one.txt").exists(),
            "the repo is at the first operation's pre-state"
        );
        assert_eq!(state(&dir).pos, 0, "two operations are undone");
        redo(&dir, &[]).expect("the first redo answers");
        assert!(
            dir.join("one.txt").exists() && !dir.join("two.txt").exists(),
            "the repo is at the first operation"
        );
        assert_eq!(state(&dir).pos, 1, "one operation is undone again");
        redo(&dir, &[]).expect("the second redo answers");
        assert!(dir.join("two.txt").exists(), "the repo is at the second operation");
        assert_eq!(state(&dir).pos, 2, "nothing is undone");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_markers_stand() {
        let dir = repo("markers");
        let entries = perform(&dir);
        undo(&dir, &[]).expect("the applied operation undoes");
        assert!(
            !dir.join("notes.txt").exists(),
            "the repo actually moved: the file is gone"
        );
        assert_eq!(
            state(&dir).markers[0].entries,
            entries,
            "undo leaves the markers' entries standing"
        );
        redo(&dir, &[]).expect("the undone operation redoes");
        assert!(
            dir.join("notes.txt").exists(),
            "the repo actually moved: the file is back"
        );
        assert_eq!(
            state(&dir).markers[0].entries,
            entries,
            "redo leaves the markers' entries standing"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undo_then_redo_returns_the_repo() {
        let dir = repo("roundtrip");
        perform(&dir);
        undo(&dir, &[]).expect("the applied operation undoes");
        assert!(
            !dir.join("notes.txt").exists(),
            "the repo is at the operation's pre-state after the undo"
        );
        redo(&dir, &[]).expect("the undone operation redoes");
        assert!(
            dir.join("notes.txt").exists(),
            "the repo is back at the operation after the redo"
        );
        let state = state(&dir);
        assert_eq!(state.pos, 1, "nothing is left undone");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recording_uses_journal_change_uses_op_revert() {
        // A recording's undo applies its journal entry and records that entry id; a removal's
        // undo reverts its op-log entries and records those ids.
        let recorded = repo("journal-leg");
        jj(&recorded, &["describe", "-m", "target"]);
        let target = change_id(&recorded);
        edit(&recorded, "notes.txt", "recorded\n");
        jj(&recorded, &["status"]);
        jj(&recorded, &["new", "-m", "child"]);
        fs::remove_file(recorded.join("notes.txt")).unwrap();
        jj(&recorded, &["status"]);
        let store = Store::new(&recorded);
        let mut before = std::collections::BTreeMap::new();
        before.insert("notes.txt".to_string(), None);
        let mut after = std::collections::BTreeMap::new();
        after.insert("notes.txt".to_string(), Some(b"recorded\n".to_vec()));
        let journal_id = crate::journal::put(&store, "record", &target, &before, &after).unwrap();
        let text = serde_json::json!({
            "markers": [{"kind": "record", "target": target, "entries": [journal_id.clone()], "reverts": []}],
            "pos": 1,
        })
        .to_string();
        write_markers(&recorded, &text);
        undo(&recorded, &[]).expect("the recording undoes");
        let recorded_state = state(&recorded);
        assert_eq!(
            recorded_state.markers[0].reverts,
            vec![journal_id],
            "a recording records its journal entry id"
        );
        assert!(
            recorded.join("notes.txt").exists(),
            "the recording's diff returns to the working copy"
        );
        fs::remove_dir_all(&recorded).unwrap();

        let removed = repo("op-leg");
        let entries = perform(&removed);
        undo(&removed, &[]).expect("the removal undoes");
        let removed_state = state(&removed);
        assert_eq!(
            removed_state.markers[0].reverts.len(),
            entries.len(),
            "a change-level operation records its op ids"
        );
        fs::remove_dir_all(&removed).unwrap();
    }
}
