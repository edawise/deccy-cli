//! Conformance gate for the restore half of undo and redo (M-V-undo, zzntzlpq; M-V-redo,
//! lmwqvpll).
//!
//! The two marker models fix the boundary each restore lands the repo on — undo the
//! pre-boundary of the most recent applied pair (specs/markers/undo.qnt, nrqwpwkq), redo the
//! post-boundary of the next undone pair (specs/markers/redo.qnt, zovzqmon) — and the two
//! handlers carry that position to the restore module. This gate drives each handler over a
//! scratch jj workspace whose marker file names real operation ids read from the op log,
//! asserting both the repo's observable state and the persisted state each invocation leaves.
//! Self-contained per M-tests (knqkxzns): the shims pull in both handlers and the siblings they
//! call, mirroring the crate root. It reaches the marker file through M-markers-persist, so it
//! is a crate of its own, needing serde, serde_json and the schema validator. M-layout
//! (szmuyqrq) places the crate flat under tests/; I1-serde (orrvsxqq) fixes the JSON layer the
//! crate is written against and I1-schema (zmmyomyp) the validator the marker documents are
//! checked with.

#[path = "../../../src/jj.rs"]
mod jj;
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
#[path = "../../../src/restore.rs"]
mod restore;
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
    use crate::markers::Markers;
    use crate::markers_persist;
    use crate::refusal::Refusal;
    use crate::store::Store;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-restore-conformance-{}-{}",
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

    fn op_head(dir: &Path) -> String {
        jj(dir, &["op", "log", "--limit", "1", "--no-graph", "-T", "id"])
    }

    /// Write the marker document verbatim: the count-and-bound rule is the guards' rule, not the
    /// schema's, so an out-of-range `u` still passes the schema.
    fn write_markers(dir: &Path, text: &str) {
        fs::write(dir.join(".deccy/markers.json"), text).unwrap();
    }

    /// A schema-valid marker document holding one pair, the undone count and bound, and the
    /// current position at the given boundary.
    fn one_pair(pre: &str, post: &str, u: u64, b: u64, at: &str) -> String {
        format!(r#"{{"markers":[{{"pre":"{pre}","post":"{post}"}}],"u":{u},"b":{b},"at":"{at}"}}"#)
    }

    /// A schema-valid marker document holding the two real pairs of two batches — (h0, h1)
    /// and (h1, h2) — with nothing undone and the position at h2.
    fn two_pairs(h0: &str, h1: &str, h2: &str) -> String {
        format!(
            r#"{{"markers":[{{"pre":"{h0}","post":"{h1}"}},{{"pre":"{h1}","post":"{h2}"}}],"u":0,"b":0,"at":"{h2}"}}"#
        )
    }

    /// Run a real batch — write `notes.txt`, describe, then `new` — between two op-head
    /// reads, and persist the applied-state marker file (nothing undone, the position at the
    /// pair's post-boundary) naming the two real operation ids. Returns (pre, post).
    fn perform(dir: &Path) -> (String, String) {
        let pre = op_head(dir);
        edit(dir, "notes.txt", "the batch\n");
        jj(dir, &["describe", "-m", "batch"]);
        jj(dir, &["new"]);
        let post = op_head(dir);
        write_markers(dir, &one_pair(&pre, &post, 0, 0, &post));
        (pre, post)
    }

    /// Reach the state one undo leaves: restore the repo to the pair's pre-boundary, then
    /// persist the undone state — the count one back, the bound at the pair, the position at
    /// its pre-boundary. Undo is not under test here; the undone starting point is set up over
    /// real jj, exactly as `perform` sets up the applied one.
    fn undone(dir: &Path, pre: &str, post: &str) {
        jj(dir, &["op", "restore", pre]);
        write_markers(dir, &one_pair(pre, post, 1, 1, pre));
    }

    /// Two real batches — each writes its own file, describes, then `new` — between three
    /// op-head reads, and persist a schema-valid two-pair marker file naming them.
    /// Returns (h0, h1, h2).
    fn perform_two(dir: &Path) -> (String, String, String) {
        let h0 = op_head(dir);
        edit(dir, "one.txt", "one\n");
        jj(dir, &["describe", "-m", "batch one"]);
        jj(dir, &["new"]);
        let h1 = op_head(dir);
        edit(dir, "two.txt", "two\n");
        jj(dir, &["describe", "-m", "batch two"]);
        jj(dir, &["new"]);
        let h2 = op_head(dir);
        write_markers(dir, &two_pairs(&h0, &h1, &h2));
        (h0, h1, h2)
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
    fn undo_lands_the_pre_boundary_the_model_names() {
        let dir = repo("undo");
        let (h0, _h1) = perform(&dir);
        assert!(dir.join("notes.txt").exists(), "the batch wrote its file");
        let lines = undo(&dir, &[]).expect("the applied operation undoes");
        assert!(lines.is_empty(), "nothing on stdout");
        assert!(
            !dir.join("notes.txt").exists(),
            "the batch's file is gone: the repo is at h0"
        );
        let log = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(
            !log.contains("batch"),
            "its change is gone from jj log, got {log:?}"
        );
        let state = state(&dir);
        assert_eq!(
            state.at, h0,
            "undo lands the most recent applied pair's pre-boundary"
        );
        assert_eq!(state.u, 1, "the undone count steps up one");
        assert_eq!(state.b, 1, "the bound is set to the pair count");
        assert_eq!(state.markers.len(), 1, "the pair stands");
        assert_eq!(crate::UNDO_ROW.name, "undo");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_lands_the_post_boundary_the_model_names() {
        let dir = repo("redo");
        let (h0, h1) = perform(&dir);
        undone(&dir, &h0, &h1);
        assert!(
            !dir.join("notes.txt").exists(),
            "the batch's file is gone from the working copy"
        );
        let lines = redo(&dir, &[]).expect("the undone operation redoes");
        assert!(lines.is_empty(), "nothing on stdout");
        assert!(
            dir.join("notes.txt").exists(),
            "the batch's file is back: the repo is at h1"
        );
        let state = state(&dir);
        assert_eq!(
            state.at, h1,
            "redo lands the next undone pair's post-boundary"
        );
        assert_eq!(state.u, 0, "the undone count steps down to nothing undone");
        assert_eq!(state.b, 1, "the bound stands");
        assert_eq!(state.markers.len(), 1, "the pair stands");
        assert_eq!(crate::REDO_ROW.name, "redo");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_walk_follows_the_model_index() {
        let dir = repo("walk");
        let (h0, h1, h2) = perform_two(&dir);
        assert!(dir.join("two.txt").exists(), "both batches are applied");
        undo(&dir, &[]).expect("the first undo answers");
        assert_eq!(
            state(&dir).at,
            h1,
            "the first undo lands the second pair's pre-boundary"
        );
        assert!(
            dir.join("one.txt").exists() && !dir.join("two.txt").exists(),
            "the repo is at h1"
        );
        let after_first = state(&dir);
        assert_eq!(
            (after_first.u, after_first.b),
            (1, 2),
            "one undone, the bound at the pair count"
        );
        undo(&dir, &[]).expect("the second undo answers");
        assert_eq!(
            state(&dir).at,
            h0,
            "the second undo lands the first pair's pre-boundary"
        );
        assert!(
            !dir.join("one.txt").exists(),
            "the repo is at h0"
        );
        let after_second = state(&dir);
        assert_eq!((after_second.u, after_second.b), (2, 2), "two undone, the bound stands");
        redo(&dir, &[]).expect("the first redo answers");
        assert_eq!(
            state(&dir).at,
            h1,
            "the first redo lands the first pair's post-boundary"
        );
        assert!(dir.join("one.txt").exists(), "the repo is at h1");
        assert_eq!(state(&dir).u, 1, "one operation is undone again");
        redo(&dir, &[]).expect("the second redo answers");
        assert_eq!(
            state(&dir).at,
            h2,
            "the second redo lands the second pair's post-boundary"
        );
        assert!(dir.join("two.txt").exists(), "the repo is at h2");
        assert_eq!(state(&dir).u, 0, "nothing is undone");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_pairs_stand() {
        let dir = repo("pairs");
        let (_h0, _h1) = perform(&dir);
        let before = state(&dir).markers;
        assert_eq!(before.len(), 1, "the pair is there to begin with");
        undo(&dir, &[]).expect("the applied operation undoes");
        assert!(
            !dir.join("notes.txt").exists(),
            "the repo actually moved: the file is gone"
        );
        assert_eq!(state(&dir).markers, before, "undo leaves the pairs untouched");
        redo(&dir, &[]).expect("the undone operation redoes");
        assert!(
            dir.join("notes.txt").exists(),
            "the repo actually moved: the file is back"
        );
        assert_eq!(state(&dir).markers, before, "redo leaves the pairs untouched");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undo_then_redo_returns_the_repo() {
        let dir = repo("roundtrip");
        let (_h0, h1) = perform(&dir);
        undo(&dir, &[]).expect("the applied operation undoes");
        assert!(
            !dir.join("notes.txt").exists(),
            "the repo is at h0 after the undo"
        );
        redo(&dir, &[]).expect("the undone operation redoes");
        assert!(
            dir.join("notes.txt").exists(),
            "the repo is back at h1 after the redo"
        );
        let state = state(&dir);
        assert_eq!(state.at, h1, "the round trip lands the pair's post-boundary");
        assert_eq!(state.u, 0, "nothing is left undone");
        fs::remove_dir_all(&dir).unwrap();
    }
}
