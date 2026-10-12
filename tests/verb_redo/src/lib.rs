//! Gate for M-V-redo (the redo verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj workspace whose
//! marker file names real operation ids read from the op log — it reaches the marker file,
//! so this is a crate of its own, needing serde, serde_json and the schema
//! validator. The compile-time `REDO_ROW` is the tie to M-route (a Verb row for the handler).

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
    use std::process::Command;

    use crate::context::resolve;
    use crate::markers::Markers;
    use crate::markers_persist;
    use crate::refusal::Refusal;
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

    /// Write the marker document verbatim: the count-and-bound rule is the guards' rule, not the
    /// schema's, so an out-of-range `u` still passes the schema.
    fn write_markers(dir: &Path, text: &str) {
        fs::write(dir.join(".deccy/markers.json"), text).unwrap();
    }

    /// A schema-valid marker document holding one pair with the given boundaries.
    fn one_pair(pre: &str, post: &str, u: u64, b: u64) -> String {
        format!(r#"{{"markers":[{{"pre":"{pre}","post":"{post}"}}],"u":{u},"b":{b},"at":"0"}}"#)
    }

    /// The marker file read back verbatim, for the unchanged-after-refusal assertions.
    fn marker_file(dir: &Path) -> Option<String> {
        fs::read_to_string(dir.join(".deccy/markers.json")).ok()
    }

    /// Run a real batch — write `notes.txt`, describe, then `new` — between two op-head
    /// reads, and persist a schema-valid one-pair marker file naming them. Returns (pre, post).
    fn perform(dir: &Path) -> (String, String) {
        let pre = op_head(dir);
        edit(dir, "notes.txt", "the batch\n");
        jj(dir, &["describe", "-m", "batch"]);
        jj(dir, &["new"]);
        let post = op_head(dir);
        write_markers(dir, &one_pair(&pre, &post, 0, 0));
        (pre, post)
    }

    /// A schema-valid marker document holding the two real pairs of two batches — (h0, h1)
    /// and (h1, h2) — with nothing undone and the position at h2.
    fn two_pairs(h0: &str, h1: &str, h2: &str) -> String {
        format!(
            r#"{{"markers":[{{"pre":"{h0}","post":"{h1}"}},{{"pre":"{h1}","post":"{h2}"}}],"u":0,"b":0,"at":"{h2}"}}"#
        )
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

    /// Reach the state one undo leaves: restore the repo to the pair's pre-boundary, then
    /// persist the undone state — the count one back, the bound at the pair, the position at
    /// its pre-boundary (S5-undo-effect). Undo is not under test here, so the undone starting
    /// point is set up over real jj, exactly as `perform` sets up the applied one.
    fn undone(dir: &Path, pre: &str, post: &str) {
        jj(dir, &["op", "restore", pre]);
        write_markers(
            dir,
            &format!(r#"{{"markers":[{{"pre":"{pre}","post":"{post}"}}],"u":1,"b":1,"at":"{pre}"}}"#),
        );
    }

    /// Reach the state two undos leave: restore the repo to the first pair's pre-boundary,
    /// then persist the twice-undone state (u=2, b=2, at=h0).
    fn undone_two(dir: &Path, h0: &str, h1: &str, h2: &str) {
        jj(dir, &["op", "restore", h0]);
        write_markers(
            dir,
            &format!(
                r#"{{"markers":[{{"pre":"{h0}","post":"{h1}"}},{{"pre":"{h1}","post":"{h2}"}}],"u":2,"b":2,"at":"{h0}"}}"#
            ),
        );
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
        let (pre, post) = perform(&dir);
        assert!(dir.join("notes.txt").exists(), "the batch wrote its file");
        undone(&dir, &pre, &post);
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
    fn the_state_is_written_after_the_restore() {
        let dir = repo("state");
        let (pre, post) = perform(&dir);
        undone(&dir, &pre, &post);
        let before = state(&dir);
        assert_eq!(before.u, 1, "the undone state has one operation undone");
        assert_eq!(before.b, 1, "its bound stands at the pair");
        assert_eq!(before.at, pre, "its position is the pair's pre-boundary");
        redo(&dir, &[]).expect("the undone operation redoes");
        let state = state(&dir);
        assert_eq!(state.u, 0, "the undone count steps forward to nothing undone");
        assert_eq!(state.b, 1, "the bound stands");
        assert_eq!(state.at, post, "the position is the pair's post-boundary");
        assert_eq!(state.markers.len(), 1, "the one pair stands");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_does_not_bracket() {
        let dir = repo("nobracket");
        let (pre, post) = perform(&dir);
        undone(&dir, &pre, &post);
        redo(&dir, &[]).expect("the undone operation redoes");
        assert_eq!(state(&dir).markers.len(), 1, "no pair is appended");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_invocation_redoes_one_operation() {
        let dir = repo("twostep");
        let (h0, h1, h2) = perform_two(&dir);
        undone_two(&dir, &h0, &h1, &h2);
        assert_eq!(state(&dir).at, h0, "the position is the first pair's pre-boundary");
        assert!(!dir.join("one.txt").exists(), "the first batch's file is gone");
        assert!(!dir.join("two.txt").exists(), "the second batch's file is gone");
        redo(&dir, &[]).expect("the first redo answers");
        assert_eq!(state(&dir).at, h1, "the position is the first pair's post-boundary");
        assert!(dir.join("one.txt").exists(), "the first batch's file is back");
        redo(&dir, &[]).expect("the second redo answers");
        assert_eq!(state(&dir).at, h2, "the position is the second pair's post-boundary");
        assert!(dir.join("two.txt").exists(), "the second batch's file is back");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_argument_refuses_naming_it() {
        let dir = repo("argument");
        perform(&dir);
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
        assert_eq!(op_head(&dir), head, "nothing was restored");
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
            let lines = redo(&dir, &args).expect("help answers");
            assert_eq!(lines.len(), 1, "{args:?} answers one line");
        }
        assert_eq!(op_head(&dir), head, "help restores nothing");
        assert_eq!(marker_file(&dir), before, "help leaves the marker file alone");
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
        assert_eq!(op_head(&dir), head, "nothing was restored");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_newer_operation_refuses_and_nothing_moves() {
        let dir = repo("newer");
        let (h0, h1, h2) = perform_two(&dir);
        edit(&dir, "three.txt", "three\n");
        jj(&dir, &["describe", "-m", "batch three"]);
        jj(&dir, &["new"]);
        let h3 = op_head(&dir);
        write_markers(
            &dir,
            &format!(
                r#"{{"markers":[{{"pre":"{h0}","post":"{h1}"}},{{"pre":"{h1}","post":"{h2}"}},{{"pre":"{h2}","post":"{h3}"}}],"u":1,"b":2,"at":"{h2}"}}"#
            ),
        );
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("a newer operation must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was restored");
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
        match redo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty(), "the corruption is named"),
            Ok(lines) => panic!("a corrupt file must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was restored");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_count_or_bound_beyond_the_pairs_refuses_and_nothing_moves() {
        let dir = repo("beyond");
        let (_pre, post) = perform(&dir);
        write_markers(&dir, &one_pair(&post, &post, 5, 5));
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("a count or bound past the pairs must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was restored");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_boundary_whose_operation_is_gone_refuses_naming_it() {
        let dir = repo("gone");
        let (pre, _post) = perform(&dir);
        let ghost = "aaaaaaaaaaaaaaaa";
        write_markers(&dir, &one_pair(&pre, ghost, 1, 1));
        let head = op_head(&dir);
        let before = marker_file(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&ghost.to_string()),
                "the refusal names the boundary, got {refusal:?}"
            ),
            Ok(lines) => panic!("a gone boundary must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was restored");
        assert_eq!(marker_file(&dir), before, "the marker file is unchanged");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Make `path` read-only, so a save into it cannot complete; undone by `writable`.
    fn read_only(path: &Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// Restore write permission so the scratch tree can be torn down.
    fn writable(path: &Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn a_marker_file_that_cannot_be_read_refuses_naming_the_read() {
        let dir = repo("unreadable");
        let (pre, post) = perform(&dir);
        undone(&dir, &pre, &post);
        // A directory where the marker file belongs: the read fails as a read failure, not
        // as absence and not as a corruption (S2-markers-refusal-site, sprvousp). Nothing
        // is restored, because the load is what fails first.
        fs::remove_file(dir.join(".deccy/markers.json")).unwrap();
        fs::create_dir_all(dir.join(".deccy/markers.json")).unwrap();
        let head = op_head(&dir);
        match redo(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.cause().contains("directory"),
                "the read failure is named, not the corruption, got {refusal:?}"
            ),
            Ok(lines) => panic!("a marker file that cannot be read must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was restored");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_marker_file_that_cannot_be_written_refuses_naming_the_save() {
        let dir = repo("unwritable");
        let (pre, post) = perform(&dir);
        undone(&dir, &pre, &post);
        // The save after the restore cannot complete: the state directory is read-only. The
        // decided order (S5-save-after-restore, kqqysmxn) means the restore has already
        // happened, so the refusal is pinned, not "nothing moved".
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
        fs::remove_dir_all(&dir).unwrap();
    }
}
