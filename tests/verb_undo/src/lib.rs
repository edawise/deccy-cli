//! Gate for M-V-undo (the undo verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj workspace whose
//! marker file names real operation ids read from the op log — it reaches the marker file,
//! so this is a crate of its own, needing serde, serde_json and the schema
//! validator. The compile-time `UNDO_ROW` is the tie to M-route (a Verb row for the handler).

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
#[path = "../../../src/verb_undo.rs"]
mod verb_undo;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a Verb row for `undo` (M-route, ynvwvzrm) compiles.
const UNDO_ROW: Verb<Context> = Verb {
    name: "undo",
    run: crate::verb_undo::run,
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
    use crate::store::Store;
    use crate::verb_undo::run;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-undo-{}-{}",
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
        one_marker_of("remove", "kqxyzmrn", entries, reverts, pos)
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
    /// persist a schema-valid marker file naming the run window's entries, and return them,
    /// oldest first.
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

    /// A merge operation's id: two operations driven at once make jj reconcile them, and the
    /// reconcile operation carries several parents, which an undo will not revert.
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

    /// The marker state read back through the persistence layer.
    fn state(dir: &Path) -> Markers {
        markers_persist::load(&Store::new(dir)).expect("the marker file loads")
    }

    /// `deccy undo <args>` through the real handler over the scratch workspace.
    fn undo(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repo resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        run(&context, &owned)
    }

    #[test]
    fn a_marked_operation_is_undone() {
        let dir = repo("undo");
        perform(&dir);
        assert!(dir.join("notes.txt").exists(), "the batch wrote its file");
        let lines = undo(&dir, &[]).expect("a marked operation undoes");
        assert!(lines.is_empty(), "O7-undo: nothing on stdout");
        assert!(
            !dir.join("notes.txt").exists(),
            "the batch's file is gone from the working copy"
        );
        let log = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(
            !log.contains("batch"),
            "its change is gone from jj log, got {log:?}"
        );
        assert_eq!(crate::UNDO_ROW.name, "undo");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_creation_is_undone_by_removing_the_created_change() {
        let dir = repo("undo-create");
        jj(&dir, &["describe", "-m", "created"]);
        let created = jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"]);
        jj(&dir, &["new"]);
        write_markers(
            &dir,
            &one_marker_of("create", &created, &[], &[], 1),
        );
        let lines = undo(&dir, &[]).expect("a creation undoes");
        assert!(lines.is_empty(), "O7-undo: nothing on stdout");
        let log = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(
            !log.contains("created"),
            "the created change is removed, got {log:?}"
        );
        let state = state(&dir);
        assert_eq!(state.pos, 0, "the operation is undone");
        assert_eq!(
            state.markers[0].reverts.len(),
            1,
            "the removal is recorded as one revert, not one per entry"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_state_is_written_after_the_reverts() {
        let dir = repo("state");
        let entries = perform(&dir);
        undo(&dir, &[]).expect("a marked operation undoes");
        let state = state(&dir);
        assert_eq!(state.pos, 0, "the position steps back to zero");
        assert_eq!(state.markers.len(), 1, "the one marker stands");
        assert_eq!(
            state.markers[0].entries, entries,
            "the operation's kept entry ids stand"
        );
        assert_eq!(
            state.markers[0].reverts.len(),
            entries.len(),
            "the reverts made are recorded against the operation"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undo_does_not_bracket() {
        let dir = repo("nobracket");
        let entries = perform(&dir);
        undo(&dir, &[]).expect("a marked operation undoes");
        let state = state(&dir);
        assert_eq!(state.markers.len(), 1, "no marker is appended");
        assert_eq!(state.markers[0].entries, entries, "the marker stands");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_invocation_undoes_one_operation() {
        let dir = repo("twostep");
        perform_two(&dir);
        undo(&dir, &[]).expect("the first undo answers");
        assert!(dir.join("one.txt").exists(), "the first batch's file stands");
        assert!(
            !dir.join("two.txt").exists(),
            "the second batch's file is gone"
        );
        assert_eq!(state(&dir).pos, 1, "one operation is undone");
        undo(&dir, &[]).expect("the second undo answers");
        assert!(
            !dir.join("one.txt").exists(),
            "the first batch's file is gone"
        );
        assert_eq!(state(&dir).pos, 0, "two operations are undone");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_append_after_an_undo_discards_the_tail() {
        // F4: undo, then a new operation, then undo. The new operation discards the undone
        // tail, so the second undo takes the new operation and the third has no abandoned
        // operation to walk onto: it refuses `no operation exists`.
        let dir = repo("linear");
        perform(&dir);
        undo(&dir, &[]).expect("the first operation undoes");
        assert_eq!(state(&dir).pos, 0, "nothing is applied");

        // A new operation brackets: discard the tail at the position, then append.
        let pre = op_head(&dir);
        edit(&dir, "second.txt", "second\n");
        jj(&dir, &["describe", "-m", "second batch"]);
        jj(&dir, &["new"]);
        let post = op_head(&dir);
        let entries = window(&dir, &pre, &post);
        let store = Store::new(&dir);
        crate::markers_append::append(
            &store,
            crate::markers::Kind::Remove,
            "kqxyzmrn",
            &entries,
        )
        .expect("the new operation brackets");
        let after_append = state(&dir);
        assert_eq!(after_append.markers.len(), 1, "the undone tail is discarded");
        assert_eq!(after_append.pos, 1, "the position is at the end");

        undo(&dir, &[]).expect("the new operation undoes");
        assert_eq!(state(&dir).pos, 0, "the new operation is undone");
        match undo(&dir, &[]) {
            Err(refusal) => assert_eq!(refusal.cause(), "no operation exists"),
            Ok(lines) => panic!("the tail is gone, so undo must refuse, got {lines:?}"),
        }
        assert_eq!(
            state(&dir).markers[0].entries,
            entries,
            "the one marker is the new operation"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_argument_refuses_naming_it() {
        let dir = repo("argument");
        perform(&dir);
        let head = op_head(&dir);
        let before = marker_file(&dir);
        for args in [vec!["extra"], vec!["--frob"], vec!["extra", "--help"]] {
            match undo(&dir, &args) {
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
        perform(&dir);
        let head = op_head(&dir);
        let before = marker_file(&dir);
        for args in [vec!["--help"], vec!["--help", "extra"]] {
            let lines = undo(&dir, &args).expect("help answers");
            assert_eq!(lines.len(), 1, "{args:?} answers one line");
        }
        assert_eq!(op_head(&dir), head, "help reverts nothing");
        assert_eq!(
            marker_file(&dir),
            before,
            "help leaves the marker file alone"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_answers_without_a_workspace() {
        // U1/xlvoxmyl: the surface is answered before the store is taken, so a --help
        // invocation needs no workspace (S3-root-required covers the operation, not help).
        let dir = scratch("help-rootless");
        let lines = undo(&dir, &["--help"]).expect("help answers on a root-less context");
        assert_eq!(
            lines,
            vec![crate::verb_undo::HELP.to_string()],
            "the verb's own help line"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_argument_without_a_workspace_refuses_naming_it() {
        // U1: a bad argument is refused by name, not preempted by the absent workspace.
        let dir = scratch("argument-rootless");
        match undo(&dir, &["nope"]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"nope".to_string()),
                "the refusal names the argument, got {refusal:?}"
            ),
            Ok(lines) => panic!("the argument must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_stack_refuses_and_nothing_moves() {
        let dir = repo("empty");
        let head = op_head(&dir);
        match undo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("an empty stack must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "no marker file is written"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_operation_undone_refuses_and_nothing_moves() {
        let dir = repo("exhausted");
        perform_two(&dir);
        undo(&dir, &[]).expect("the first undo answers");
        undo(&dir, &[]).expect("the second undo answers");
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match undo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("an exhausted stack must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "the refusal reverted nothing");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_marker_file_refuses_naming_the_corruption() {
        let dir = repo("corrupt");
        perform(&dir);
        write_markers(&dir, "{not json");
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match undo(&dir, &[]) {
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
        write_markers(&dir, &one_marker(&entries, &[], 5));
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match undo(&dir, &[]) {
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
        perform(&dir);
        let ghost = "aaaaaaaaaaaaaaaa";
        write_markers(&dir, &one_marker(&[ghost.to_string()], &[], 1));
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match undo(&dir, &[]) {
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

    #[test]
    fn an_entry_that_merges_parents_is_refused_by_name() {
        let dir = repo("merge");
        let merge = merge_entry(&dir);
        write_markers(&dir, &one_marker(&[merge.clone()], &[], 1));
        match undo(&dir, &[]) {
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
        let ghost = "aaaaaaaaaaaaaaaa";
        // the ghost sits oldest-but-one, so the newest entry reverts first and the ghost then
        // refuses: the reversion already made has to be undone for the state to return.
        let mut with_ghost = entries.clone();
        with_ghost.insert(entries.len() - 1, ghost.to_string());
        write_markers(&dir, &one_marker(&with_ghost, &[], 1));
        let before = marker_file(&dir);
        match undo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&ghost.to_string()),
                "the refusing entry is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a partway refusal must refuse, got {lines:?}"),
        }
        assert_eq!(
            jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "description.first_line()"]),
            "",
            "the reversion already made has been undone: the working copy is back where it began"
        );
        assert!(
            dir.join("notes.txt").exists(),
            "the repo is back at its pre-undo state"
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
        write_markers(&dir, &one_marker_of("remove", "kqxyzmrn", &entries, &[], 1));

        // Amend the working-copy commit after the squash: reverting the squash now splits it.
        edit(&dir, "dirty.txt", "uncommitted\n");
        jj(&dir, &["status"]);
        let before = marker_file(&dir);

        match undo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&entries[0]),
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
            "the squashed edit stands: the reverting revert was reverted"
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
        write_markers(&dir, &one_marker_of("remove", "kqxyzmrn", &entries, &[], 1));

        // Amend the change the squash rewrote, then move the working copy off it: the working
        // copy change is clean, but reverting still splits that rewritten ancestor.
        edit(&dir, "child.txt", "child edited\n");
        jj(&dir, &["new"]);
        let before = marker_file(&dir);

        match undo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&entries[0]),
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
        perform(&dir);
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
        match undo(&dir, &[]) {
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
        perform(&dir);
        // The save after the reverts cannot complete: the state directory is read-only.
        // The save failure reverses the write half (S5-save-failure, ntlspwxk): the graph
        // returns to its pre-invocation state and the marker file is left as it was.
        let markers_before = marker_file(&dir);
        let log_before = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(
            dir.join("notes.txt").exists(),
            "the batch's file stands before undo"
        );
        read_only(&dir.join(".deccy"));
        let result = undo(&dir, &[]);
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
            "the graph returns to its pre-invocation state"
        );
        assert!(
            dir.join("notes.txt").exists(),
            "the batch's file is restored"
        );
        assert_eq!(state(&dir).pos, 1, "the position stands at the undoes marker");
        // The same undo lands once the save can be written.
        undo(&dir, &[]).expect("the same undo answers once the save can land");
        assert!(!dir.join("notes.txt").exists(), "the batch's file is gone");
        assert_eq!(state(&dir).pos, 0, "the operation is undone");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_creation_undo_that_cannot_save_restores_the_change() {
        let dir = repo("unwritable-create");
        // The created change is the working copy, as `deccy new` leaves it.
        jj(&dir, &["describe", "-m", "created"]);
        let created = jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"]);
        write_markers(&dir, &one_marker_of("create", &created, &[], &[], 1));
        // The save failure reverses the creation's removal (S5-save-failure, ntlspwxk):
        // the created change stands and the marker file is left as it was.
        let markers_before = marker_file(&dir);
        let log_before = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        read_only(&dir.join(".deccy"));
        let result = undo(&dir, &[]);
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
            "the created change is restored"
        );
        assert_eq!(state(&dir).pos, 1, "the position stands at the undoes marker");
        // The same undo lands once the save can be written.
        undo(&dir, &[]).expect("the same undo answers once the save can land");
        assert_eq!(state(&dir).pos, 0, "the creation is undone");
        let log = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(
            !log.contains("created"),
            "the created change is removed, got {log:?}"
        );
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

    /// Store one recording's journal entry and write a marker naming it.
    fn journal_marker(dir: &Path, target: &str, file: &str, contents: &str) -> String {
        let store = Store::new(dir);
        let mut before = std::collections::BTreeMap::new();
        before.insert(file.to_string(), None);
        let mut after = std::collections::BTreeMap::new();
        after.insert(file.to_string(), Some(contents.as_bytes().to_vec()));
        let id = crate::journal::put(&store, "record", target, &before, &after)
            .expect("the journal entry is stored");
        write_markers(dir, &one_marker_of("record", target, &[id.clone()], &[], 1));
        id
    }

    #[test]
    fn two_records_undo_twice() {
        let dir = repo("journal-two");
        let one = journal_target(&dir, "one", "one.txt", "one\n");
        let two = journal_target(&dir, "two", "two.txt", "two\n");
        let one_id = journal_marker(&dir, &one, "one.txt", "one\n");
        // The second marker is the newest; the first entry is kept beside it.
        let store = Store::new(&dir);
        let mut before = std::collections::BTreeMap::new();
        before.insert("two.txt".to_string(), None);
        let mut after = std::collections::BTreeMap::new();
        after.insert("two.txt".to_string(), Some(b"two\n".to_vec()));
        let two_id = crate::journal::put(&store, "record", &two, &before, &after).unwrap();
        let text = serde_json::json!({
            "markers": [
                {"kind": "record", "target": one, "entries": [one_id], "reverts": []},
                {"kind": "record", "target": two, "entries": [two_id], "reverts": []},
            ],
            "pos": 2,
        })
        .to_string();
        write_markers(&dir, &text);

        undo(&dir, &[]).expect("the first recording undoes");
        assert_eq!(state(&dir).pos, 1);
        assert_eq!(
            fs::read_to_string(dir.join("two.txt")).unwrap(),
            "two\n",
            "the working copy keeps the after side the recording left"
        );
        undo(&dir, &[]).expect("the second recording undoes");
        assert_eq!(state(&dir).pos, 0);
        assert_eq!(
            jj(&dir, &["log", "-r", "divergent()", "--no-graph", "-T", "commit_id"]),
            "",
            "no change is left divergent"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn record_then_unrelated_dirty_undo() {
        let dir = repo("journal-dirty");
        let target = journal_target(&dir, "batch", "notes.txt", "the batch\n");
        journal_marker(&dir, &target, "notes.txt", "the batch\n");
        edit(&dir, "dirty.txt", "unrelated\n");
        undo(&dir, &[]).expect("an unrelated dirty path never blocks a journal apply");
        assert_eq!(
            fs::read_to_string(dir.join("dirty.txt")).unwrap(),
            "unrelated\n",
            "the unrelated path stands"
        );
        assert!(
            dir.join("notes.txt").exists(),
            "the working copy keeps the after side, so the diff shows against the target"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undo_returns_diff_to_working_copy() {
        let dir = repo("journal-return");
        let target = journal_target(&dir, "batch", "notes.txt", "the batch\n");
        journal_marker(&dir, &target, "notes.txt", "the batch\n");
        // A recording leaves the after side in the working copy: the file is there and the
        // target holds it too, so the working copy shows no diff against the target yet.
        assert_eq!(
            fs::read_to_string(dir.join("notes.txt")).unwrap(),
            "the batch\n",
            "the recording leaves the after side in the working copy"
        );
        assert_eq!(
            jj(&dir, &["diff", "--name-only"]),
            "",
            "the sides match before the undo"
        );
        undo(&dir, &[]).expect("the recording undoes");
        assert_eq!(
            fs::read_to_string(dir.join("notes.txt")).unwrap(),
            "the batch\n",
            "the working copy still holds the after side"
        );
        assert_eq!(
            jj(&dir, &["diff", "--name-only"]),
            "notes.txt",
            "the recorded diff appears against the target's before side"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refused_undo_retry_is_one_line() {
        let dir = repo("journal-retry");
        let target = journal_target(&dir, "batch", "notes.txt", "the batch\n");
        journal_marker(&dir, &target, "notes.txt", "the batch\n");
        // The working copy holds a wrong side; the apply refuses before moving.
        edit(&dir, "notes.txt", "someone else\n");
        match undo(&dir, &[]) {
            Err(refusal) => {
                let rendered = format!("{refusal:?}");
                assert!(!rendered.contains("Internal error"), "no internal error: {rendered}");
                assert!(!rendered.contains("already exists"), "no retry trap: {rendered}");
            }
            Ok(lines) => panic!("a working-copy mismatch must refuse, got {lines:?}"),
        }
        // With the working copy back at its after side, the same undo lands.
        edit(&dir, "notes.txt", "the batch\n");
        undo(&dir, &[]).expect("the retry lands once the side is restored");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn journal_conflict_names_path() {
        let dir = repo("journal-conflict");
        let target = journal_target(&dir, "batch", "notes.txt", "the batch\n");
        journal_marker(&dir, &target, "notes.txt", "the batch\n");
        // Rewrite the target to something else; the target no longer holds the after side.
        edit(&dir, "notes.txt", "rewritten\n");
        jj(&dir, &["status"]);
        match undo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"notes.txt".to_string()),
                "the mismatched path is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a target mismatch must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
