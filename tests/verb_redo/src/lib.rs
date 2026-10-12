//! Gate for M-V-redo (the redo verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj workspace whose
//! marker file names real operation ids read from the op log — it reaches the marker file,
//! so this is a crate of its own, needing serde, serde_json and the schema
//! validator. The compile-time `REDO_ROW` is the tie to M-route (a Verb row for the handler).

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/divergence.rs"]
mod divergence;
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
#[path = "../../../src/markers_append.rs"]
mod markers_append;
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
#[path = "../../../src/verb_redo.rs"]
mod verb_redo;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a Verb row for `redo` (M-route, ynvwvzrm) compiles.
const REDO_ROW: Verb<Context> = Verb {
    name: "redo",
    run: crate::verb_redo::run,
};

#[cfg(test)]
mod gate {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use crate::context::resolve;
    use crate::jj::Jj;
    use crate::markers::Markers;
    use crate::markers_file;
    use crate::markers_persist;
    use crate::oplog;
    use crate::refusal::Refusal;
    use crate::revert;
    use crate::store::Store;
    use crate::verb_redo::run;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-redo-{}-{}",
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

    /// A schema-valid marker document holding one marker of the given kind, targeting the
    /// given change.
    fn one_marker_of(
        kind: &str,
        target: &str,
        entries: &[String],
        reverts: &[String],
        pos: u64,
    ) -> String {
        serde_json::json!({
            "markers": [{"kind": kind, "target": target, "entries": entries, "reverts": reverts}],
            "pos": pos,
        })
        .to_string()
    }

    /// The marker file read back verbatim, for the unchanged-after-refusal assertions.
    fn marker_file(dir: &Path) -> Option<String> {
        fs::read_to_string(dir.join(".deccy/markers.json")).ok()
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

    /// Revert `entries` newest first over real jj, returning each revert operation's id: the
    /// write half undo would use, driven here so redo starts from the state undo leaves.
    fn revert_all(jjrun: &Jj, entries: &[String]) -> Vec<String> {
        let mut reverts = Vec::new();
        for entry in entries.iter().rev() {
            revert::revert(jjrun, std::slice::from_ref(entry)).expect("the entry reverts");
            reverts.push(oplog::oplog(jjrun).expect("the op head reads"));
        }
        reverts
    }

    /// Reach the state one undo leaves: revert the operation's entries and record the reverts
    /// against the marker. Undo is not under test here, so the undone starting point is set up
    /// over jj exactly as undo would leave it.
    fn undone(dir: &Path, entries: &[String]) -> Vec<String> {
        let reverts = revert_all(&Jj::new("jj", dir), entries);
        write_markers(dir, &one_marker(entries, &reverts, 0));
        reverts
    }

    /// Reach the state two undos leave: both operations' entries reverted, the reverts recorded
    /// against their markers, the undone count and the bound at two.
    fn undone_two(dir: &Path, one: &[String], two: &[String]) {
        let jjrun = Jj::new("jj", dir);
        let reverts_two = revert_all(&jjrun, two);
        let reverts_one = revert_all(&jjrun, one);
        let text = serde_json::json!({
            "markers": [
                {"kind": "remove", "target": "kqxyzmrn", "entries": one, "reverts": reverts_one},
                {"kind": "remove", "target": "rnkqxyzm", "entries": two, "reverts": reverts_two},
            ],
            "pos": 0,
        })
        .to_string();
        write_markers(dir, &text);
    }

    /// The marker state read back through the persistence layer.
    fn state(dir: &Path) -> Markers {
        markers_persist::load(&Store::new(dir)).expect("the marker file loads")
    }

    /// `deccy redo <args>` through the real handler over the scratch workspace.
    fn redo(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repo resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        run(&context, &owned)
    }

    #[test]
    fn an_undone_operation_is_redone() {
        let dir = repo("redo");
        let entries = perform(&dir);
        assert!(dir.join("notes.txt").exists(), "the batch wrote its file");
        undone(&dir, &entries);
        assert!(
            !dir.join("notes.txt").exists(),
            "the batch's file is gone from the working copy"
        );
        let lines = redo(&dir, &[]).expect("the undone operation redoes");
        assert!(lines.is_empty(), "O7-redo: nothing on stdout");
        assert!(
            dir.join("notes.txt").exists(),
            "the batch's file is back in the working copy"
        );
        let log = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(log.contains("batch"), "its change is back in jj log, got {log:?}");
        assert_eq!(crate::REDO_ROW.name, "redo");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_state_is_written_after_the_reverts() {
        let dir = repo("state");
        let entries = perform(&dir);
        let reverts = undone(&dir, &entries);
        assert_eq!(reverts.len(), entries.len(), "one revert per entry");
        redo(&dir, &[]).expect("the undone operation redoes");
        let state = state(&dir);
        assert_eq!(state.pos, 1, "the position steps forward past one operation");
        assert_eq!(state.markers.len(), 1, "the one marker stands");
        assert_eq!(
            state.markers[0].entries, entries,
            "the operation's kept entry ids stand"
        );
        assert!(
            state.markers[0].reverts.is_empty(),
            "the reverts it named are cleared"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_does_not_bracket() {
        let dir = repo("nobracket");
        let entries = perform(&dir);
        undone(&dir, &entries);
        redo(&dir, &[]).expect("the undone operation redoes");
        assert_eq!(state(&dir).markers.len(), 1, "no marker is appended");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_invocation_redoes_one_operation() {
        let dir = repo("twostep");
        let (one, two) = perform_two(&dir);
        undone_two(&dir, &one, &two);
        assert_eq!(state(&dir).pos, 0, "both operations are undone");
        redo(&dir, &[]).expect("the first redo answers");
        assert!(dir.join("one.txt").exists(), "the first batch's file is back");
        assert!(!dir.join("two.txt").exists(), "the second batch's file is still gone");
        assert_eq!(state(&dir).pos, 1, "one operation is undone again");
        redo(&dir, &[]).expect("the second redo answers");
        assert!(dir.join("two.txt").exists(), "the second batch's file is back");
        assert_eq!(state(&dir).pos, 2, "nothing is undone");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_argument_refuses_naming_it() {
        let dir = repo("argument");
        let entries = perform(&dir);
        undone(&dir, &entries);
        let head = op_head(&dir);
        let before = marker_file(&dir);
        for args in [vec!["extra"], vec!["--frob"], vec!["extra", "--help"]] {
            match redo(&dir, &args) {
                Err(refusal) => assert!(
                    refusal.evidence().contains(&args[0].to_string()),
                    "{args:?} must name {:?}, got {refusal:?}",
                    args[0]
                ),
                Ok(lines) => panic!("{args:?} must refuse, got {lines:?}"),
            }
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_leading_help_is_excepted() {
        let dir = repo("help");
        let entries = perform(&dir);
        undone(&dir, &entries);
        let head = op_head(&dir);
        let before = marker_file(&dir);
        for args in [vec!["--help"], vec!["--help", "extra"]] {
            let lines = redo(&dir, &args).expect("help answers");
            assert_eq!(lines.len(), 1, "{args:?} answers one line");
        }
        assert_eq!(op_head(&dir), head, "help reverts nothing");
        assert_eq!(marker_file(&dir), before, "help leaves the marker file alone");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_answers_without_a_workspace() {
        // U1/xlvoxmyl: the surface is answered before the store is taken, so a --help
        // invocation needs no workspace (S3-root-required covers the operation, not help).
        let dir = scratch("help-rootless");
        let lines = redo(&dir, &["--help"]).expect("help answers on a root-less context");
        assert_eq!(
            lines,
            vec![crate::verb_redo::HELP.to_string()],
            "the verb's own help line"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_argument_without_a_workspace_refuses_naming_it() {
        // U1: a bad argument is refused by name, not preempted by the absent workspace.
        let dir = scratch("argument-rootless");
        match redo(&dir, &["nope"]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"nope".to_string()),
                "the refusal names the argument, got {refusal:?}"
            ),
            Ok(lines) => panic!("the argument must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nothing_undone_refuses_and_nothing_moves() {
        let dir = repo("nothing");
        perform(&dir);
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("nothing undone must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_append_after_an_undo_leaves_nothing_to_redo() {
        let dir = repo("truncated");
        let entries = perform(&dir);
        undone(&dir, &entries);
        assert_eq!(state(&dir).pos, 0, "nothing is applied");

        // The new operation discards the undone tail at the position before appending.
        let pre = op_head(&dir);
        edit(&dir, "second.txt", "second\n");
        jj(&dir, &["describe", "-m", "second batch"]);
        jj(&dir, &["new"]);
        let post = op_head(&dir);
        let second = window(&dir, &pre, &post);
        crate::markers_append::append(
            &Store::new(&dir),
            crate::markers::Kind::Record,
            "rnkqxyzm",
            &second,
        )
        .expect("the new operation brackets");
        assert_eq!(
            state(&dir).markers.len(),
            1,
            "the undone tail is discarded"
        );
        assert_eq!(state(&dir).pos, 1, "the position is at the end");

        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert_eq!(refusal.cause(), "nothing is undone"),
            Ok(lines) => panic!("the tail is gone, so redo must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_marker_file_refuses_naming_the_corruption() {
        let dir = repo("corrupt");
        let entries = perform(&dir);
        undone(&dir, &entries);
        write_markers(&dir, "{not json");
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty(), "the corruption is named"),
            Ok(lines) => panic!("a corrupt file must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_count_or_bound_beyond_the_markers_refuses_and_nothing_moves() {
        let dir = repo("beyond");
        let entries = perform(&dir);
        undone(&dir, &entries);
        write_markers(&dir, &one_marker(&entries, &[], 5));
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("a count or bound past the markers must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_entry_whose_operation_is_gone_refuses_naming_it() {
        let dir = repo("gone");
        let entries = perform(&dir);
        let ghost = "aaaaaaaaaaaaaaaa";
        write_markers(&dir, &one_marker(&entries, &[ghost.to_string()], 0));
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&ghost.to_string()),
                "the refusal names the entry, got {refusal:?}"
            ),
            Ok(lines) => panic!("a gone entry must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A merge operation's id: two operations driven at once make jj reconcile them, and the
    /// reconcile operation carries several parents, which a redo will not revert.
    fn merge_entry(dir: &Path) -> String {
        let template =
            "self.id() ++ \" \" ++ self.parents().map(|op| op.id()).join(\",\") ++ \"\\n\"";
        for _ in 0..8 {
            let mut first = Command::new("jj")
                .arg("new")
                .current_dir(dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut second = Command::new("jj")
                .arg("new")
                .current_dir(dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let _ = first.wait();
            let _ = second.wait();
            let log = jj(dir, &["op", "log", "--no-graph", "-T", template]);
            let found = log.lines().find_map(|line| {
                let mut fields = line.split_whitespace();
                let id = fields.next()?;
                let parents = fields.next().unwrap_or("");
                parents.contains(',').then(|| id.to_string())
            });
            if let Some(id) = found {
                return id;
            }
        }
        panic!("two concurrent operations leave a reconcile operation");
    }

    #[test]
    fn an_entry_that_merges_parents_is_refused_by_name() {
        // The merge entry stands where a recorded revert stands (qxmzpupt): redo re-reverts
        // it, and jj refuses a merging parent, which the refusal names.
        let dir = repo("merge");
        let entries = perform(&dir);
        let merge = merge_entry(&dir);
        write_markers(&dir, &one_marker(&entries, &[merge.clone()], 0));
        match redo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&merge),
                "the merge entry is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a merge entry must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_revert_that_refuses_partway_rolls_back() {
        let dir = repo("partway");
        let entries = perform(&dir);
        let reverts = undone(&dir, &entries);
        let ghost = "aaaaaaaaaaaaaaaa";
        // the ghost is reverts-oldest, so the recorded reverts re-revert first and the ghost
        // then refuses: the re-reverts already made have to be undone for the state to return.
        let mut with_ghost = vec![ghost.to_string()];
        with_ghost.extend(reverts);
        write_markers(&dir, &one_marker(&entries, &with_ghost, 0));
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&ghost.to_string()),
                "the refusing entry is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a partway refusal must refuse, got {lines:?}"),
        }
        assert!(
            !dir.join("notes.txt").exists(),
            "the re-reverts already made have been undone: the repo is back at the undone state"
        );
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_revert_that_diverges_the_working_copy_rolls_back() {
        let dir = repo("divergent");
        // A base change holding a file, and a child holding a squashed edit into it.
        jj(&dir, &["describe", "-m", "base"]);
        edit(&dir, "f.txt", "v1\n");
        let base = jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"]);
        jj(&dir, &["new", "-m", "child"]);
        edit(&dir, "child.txt", "child\n");
        edit(&dir, "f.txt", "v2\n");
        jj(&dir, &["status"]);
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
        let post = op_head(&dir);
        let entries = window(&dir, &pre, &post);
        assert_eq!(entries.len(), 1, "the squash is one entry");
        // Reach the state undo leaves, cleanly.
        let reverts = undone(&dir, &entries);

        // Amend the working-copy commit after the undo: reverting the revert now splits it.
        edit(&dir, "dirty.txt", "uncommitted\n");
        jj(&dir, &["status"]);
        let before = marker_file(&dir);

        match redo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&reverts[0]),
                "the refusing entry is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a diverging revert must refuse, got {lines:?}"),
        }
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        assert_eq!(
            jj(&dir, &["log", "-r", "divergent()", "--no-graph", "-T", "commit_id"]),
            "",
            "no change is left divergent"
        );
        assert!(
            dir.join("dirty.txt").exists(),
            "the uncommitted work stands"
        );
        assert_eq!(
            fs::read_to_string(dir.join("f.txt")).unwrap(),
            "v2\n",
            "the undone state stands: the reverting revert was reverted"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_revert_that_diverges_a_rewritten_ancestor_rolls_back() {
        let dir = repo("divergent-ancestor");
        // A base change holding a file, and a child holding a squashed edit into it.
        jj(&dir, &["describe", "-m", "base"]);
        edit(&dir, "f.txt", "v1\n");
        let base = jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"]);
        jj(&dir, &["new", "-m", "child"]);
        edit(&dir, "child.txt", "child\n");
        edit(&dir, "f.txt", "v2\n");
        jj(&dir, &["status"]);
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
        let post = op_head(&dir);
        let entries = window(&dir, &pre, &post);
        let reverts = undone(&dir, &entries);

        // Amend the change the revert rewrote, then move the working copy off it: the working
        // copy change is clean, but redoing still splits that rewritten ancestor.
        edit(&dir, "child.txt", "child edited\n");
        jj(&dir, &["new"]);
        let before = marker_file(&dir);

        match redo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&reverts[0]),
                "the refusing entry is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a diverging revert must refuse, got {lines:?}"),
        }
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        assert_eq!(
            jj(&dir, &["log", "-r", "divergent()", "--no-graph", "-T", "commit_id"]),
            "",
            "no change is left divergent"
        );
        assert_eq!(
            fs::read_to_string(dir.join("child.txt")).unwrap(),
            "child edited\n",
            "the amended change stands"
        );
        assert_eq!(
            fs::read_to_string(dir.join("f.txt")).unwrap(),
            "v2\n",
            "the undone state stands: the reverting revert was reverted"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Make `path` read-only, so a save into it cannot complete; `writable` gives it back.
    fn read_only(path: &Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// Give the path its write permission back so the scratch tree can be torn down.
    fn writable(path: &Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn a_marker_file_that_cannot_be_read_refuses_naming_the_read() {
        let dir = repo("unreadable");
        let entries = perform(&dir);
        undone(&dir, &entries);
        // A directory where the marker file belongs: the read fails as a read failure, not
        // as absence and not as a corruption (S2-markers-refusal-site, sprvousp). Nothing
        // is reverted, because the load is what fails first.
        fs::remove_file(dir.join(".deccy/markers.json")).unwrap();
        fs::create_dir_all(dir.join(".deccy/markers.json")).unwrap();
        let carried = match markers_file::read(&Store::new(&dir)) {
            Err(markers_file::FileError::Read(reason)) => reason,
            other => panic!("the read failure is the read's own, got {other:?}"),
        };
        let head = op_head(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert_eq!(
                refusal.cause(),
                carried,
                "the read's own reason is the refusal's cause, not the corruption's"
            ),
            Ok(lines) => panic!("a marker file that cannot be read must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_marker_file_that_cannot_be_written_refuses_naming_the_save() {
        let dir = repo("unwritable");
        let entries = perform(&dir);
        undone(&dir, &entries);
        // The save after the re-reverts cannot complete: the state directory is read-only.
        // The save failure reverses the re-reverts already made (S5-save-failure, ntlspwxk):
        // the graph returns to its pre-redo state and the marker file is left as it was.
        let markers_before = marker_file(&dir);
        let log_before = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        let notes_before = dir.join("notes.txt").exists();
        read_only(&dir.join(".deccy"));
        let result = redo(&dir, &[]);
        writable(&dir.join(".deccy"));
        match result {
            Err(refusal) => assert!(
                !refusal.cause().is_empty(),
                "the save failure is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a marker file that cannot be written must refuse, got {lines:?}"),
        }
        assert_eq!(
            marker_file(&dir),
            markers_before,
            "the marker file is left as it was"
        );
        assert_eq!(
            jj(&dir, &["log", "--no-graph", "-T", "description"]),
            log_before,
            "the graph returns to its pre-redo state"
        );
        assert_eq!(
            dir.join("notes.txt").exists(),
            notes_before,
            "the undone state stands"
        );
        assert_eq!(state(&dir).pos, 0, "the position stands at the undone marker");
        // The same redo lands once the save can be written.
        redo(&dir, &[]).expect("the same redo answers once the save can land");
        assert!(dir.join("notes.txt").exists(), "the batch's file is back");
        assert_eq!(state(&dir).pos, 1, "the operation is redone");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A real target change holding `file = contents`, and a child working copy that still
    /// holds it — the after side a recording leaves: the state one recording leaves for the
    /// journal gate.
    fn journal_target(dir: &Path, summary: &str, file: &str, contents: &str) -> String {
        jj(dir, &["describe", "-m", summary]);
        let target = jj(dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"]);
        edit(dir, file, contents);
        jj(dir, &["status"]);
        jj(dir, &["new", "-m", "child"]);
        jj(dir, &["status"]);
        target
    }

    fn journal_undone(dir: &Path, target: &str, file: &str, contents: &str) -> String {
        let store = Store::new(dir);
        let mut before = std::collections::BTreeMap::new();
        before.insert(file.to_string(), None);
        let mut after = std::collections::BTreeMap::new();
        after.insert(file.to_string(), Some(contents.as_bytes().to_vec()));
        let id = crate::journal::put(&store, "record", target, &before, &after)
            .expect("the journal entry is stored");
        let entry = crate::journal::get(&store, &id).unwrap();
        crate::journal_apply::apply(
            &store,
            &Jj::new("jj", dir),
            &entry,
            crate::journal_apply::Apply::Undo,
        )
        .expect("the recorded diff is returned to the working copy");
        write_markers(
            dir,
            &one_marker_of("record", target, &[id.clone()], &[id.clone()], 0),
        );
        id
    }

    #[test]
    fn redo_after_undo_restores_target_and_leaves_the_after_side() {
        let dir = repo("journal-redo");
        let target = journal_target(&dir, "batch", "notes.txt", "the batch\n");
        journal_undone(&dir, &target, "notes.txt", "the batch\n");
        assert!(dir.join("notes.txt").exists(), "undo left the after side in the working copy");
        redo(&dir, &[]).expect("the recording redoes");
        assert_eq!(
            fs::read_to_string(dir.join("notes.txt")).unwrap(),
            "the batch\n",
            "the working copy is left at the after side"
        );
        assert_eq!(
            jj(&dir, &["diff", "--name-only"]),
            "",
            "the recorded diff is gone from the working copy"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "notes.txt"]),
            "the batch",
            "the target holds the after side again"
        );
        let state = state(&dir);
        assert_eq!(state.pos, 1);
        assert!(state.markers[0].reverts.is_empty(), "the reverts are cleared");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_conflict_refuses() {
        let dir = repo("journal-redo-conflict");
        let target = journal_target(&dir, "batch", "notes.txt", "the batch\n");
        journal_undone(&dir, &target, "notes.txt", "the batch\n");
        // Rewrite the target; it no longer holds the before side redo expects.
        jj(&dir, &["edit", &target]);
        edit(&dir, "notes.txt", "rewritten\n");
        jj(&dir, &["status"]);
        jj(&dir, &["new", "-m", "child again"]);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(refusal.evidence().contains(&"notes.txt".to_string())),
            Ok(lines) => panic!("a conflict must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_after_edit_of_owned_path_refuses() {
        let dir = repo("journal-redo-edit");
        let target = journal_target(&dir, "batch", "notes.txt", "the batch\n");
        journal_undone(&dir, &target, "notes.txt", "the batch\n");
        edit(&dir, "notes.txt", "edited after undo\n");
        match redo(&dir, &[]) {
            Err(refusal) => assert!(refusal.evidence().contains(&"notes.txt".to_string())),
            Ok(lines) => panic!("an edited owned path must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
