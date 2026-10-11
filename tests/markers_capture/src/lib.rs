//! Gate for M-markers-capture (keep the entries an operation's run window holds for the
//! invoking workspace).
//!
//! A crate of its own: capture pulls in the append chain, which pulls in
//! serde_json and the marker schema validator, which plain `rustc --test` cannot
//! supply. Self-contained per M-tests — the shims pull in the module under test
//! and its siblings; no shared test-support module. The marker schema is carried in
//! the binary (`include_str!`), never read from the repo.

#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/markers.rs"]
mod markers;
#[path = "../../../src/markers_file.rs"]
mod markers_file;
#[path = "../../../src/markers_persist.rs"]
mod markers_persist;
#[path = "../../../src/markers_append.rs"]
mod markers_append;
#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/oplog.rs"]
mod oplog;
#[path = "../../../src/journal.rs"]
mod journal;
#[path = "../../../src/markers_capture.rs"]
mod markers_capture;

#[cfg(test)]
mod gate {
    use std::cell::Cell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::jj::Jj;
    use crate::markers::Kind;
    use crate::markers_capture::{capture, CaptureError};
    use crate::markers_persist::{load, PersistError, MARKERS_PATH};
    use crate::oplog::{oplog, window, OplogError};
    use crate::store::Store;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-markers-capture-{}-{}",
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

    /// A scratch jj repo with an empty `nodes/` beside it, and the state directory the marker
    /// file lives in.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        jj(&dir, &["status"]);
        dir
    }

    fn runner(dir: &Path) -> Jj {
        Jj::new("jj", dir)
    }

    /// Bracket `run` and record it as a recording of one change (the common shape in this gate).
    fn run_op<T>(store: &Store, jjrun: &Jj, run: impl FnOnce() -> T) -> Result<T, CaptureError> {
        capture(store, jjrun, Kind::Record, || {
            (run(), Some("kqxyzmrn".to_string()))
        })
    }

    fn store_at(dir: &Path) -> Store {
        Store::new(dir)
    }

    /// The entry ids the run window between two head reads holds, in run order.
    fn entries(jjrun: &Jj, pre: &str, post: &str) -> Vec<String> {
        window(jjrun, pre, post)
            .unwrap()
            .into_iter()
            .map(|entry| entry.id)
            .collect()
    }

    #[test]
    fn a_bracketed_run_keeps_the_entries_its_window_holds() {
        let dir = repo("entries");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let before = oplog(&jjrun).unwrap();
        let after = Cell::new(String::new());
        let value = run_op(&store, &jjrun, || {
            let out = jjrun.run(&["describe", "-m", "the operation"]).unwrap();
            assert_eq!(out.status, 0);
            after.set(oplog(&jjrun).unwrap());
            "done"
        })
        .unwrap();
        let after = after.into_inner();

        assert_eq!(value, "done");
        let expected = entries(&jjrun, &before, &after);
        assert!(!expected.is_empty(), "the run moved the op head");
        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert_eq!(state.markers[0].entries, expected);
        assert!(
            state.markers[0].reverts.is_empty(),
            "a fresh marker records no reverts of its own"
        );
        assert_eq!(state.pos, 1, "the position advances to the new length");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_dirty_working_copy_is_settled_before_the_window() {
        // The working copy is dirty before the bracket opens. Its snapshot must be taken before
        // the window, so the run's entries are the operation's own and not a snapshot of edits
        // that predate the invocation (S5-observation).
        let dir = repo("settle");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        fs::write(dir.join("unsettled"), "not yet snapshotted\n").unwrap();
        let operation = Cell::new(String::new());
        run_op(&store, &jjrun, || {
            jj(&dir, &["describe", "-m", "the operation"]);
            operation.set(op_id(&dir));
        })
        .unwrap();

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert_eq!(
            state.markers[0].entries,
            vec![operation.into_inner()],
            "only the run's own operation is kept; the pre-existing dirty file settles before the window"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The head operation id, read by the test's own jj crossing.
    fn op_id(dir: &Path) -> String {
        jj(dir, &["op", "log", "--limit", "1", "--no-graph", "-T", "id"])
    }

    #[test]
    fn entries_land_in_run_order() {
        let dir = repo("order");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let mut rounds = Vec::new();
        for i in 0..3 {
            // The previous append leaves `.deccy/markers.json` unsettled; settle as the bracket
            // does, so `before` is the head the window opens at (S5-observation).
            jj(&dir, &["util", "snapshot"]);
            let before = oplog(&jjrun).unwrap();
            let after = Cell::new(String::new());
            run_op(&store, &jjrun, || {
                jjrun
                    .run(&["describe", "-m", &format!("operation {i}")])
                    .unwrap();
                after.set(oplog(&jjrun).unwrap());
            })
            .unwrap();
            rounds.push((before, after.into_inner()));
        }

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 3);
        for (marker, (before, after)) in state.markers.iter().zip(rounds.iter()) {
            assert_eq!(&marker.entries, &entries(&jjrun, before, after));
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_bracket_keeps_the_invoking_workspaces_entries() {
        // Two workspaces share the repo: the run works in the invoking one, and an operation
        // of the other workspace lands inside the same window. The operation's own entries are
        // the invoking workspace's, so the other workspace's operation is not the operation's.
        let dir = repo("two-workspaces");
        let store = store_at(&dir);
        let jjrun = runner(&dir);
        let other = scratch("two-workspaces-other");
        jj(
            &dir,
            &[
                "workspace",
                "add",
                "--name",
                "other",
                other.to_str().unwrap(),
            ],
        );

        let mut ours = Vec::new();
        let theirs = Cell::new(String::new());
        run_op(&store, &jjrun, || {
            jj(&dir, &["describe", "-m", "ours"]);
            ours.push(op_id(&dir));
            jj(&other, &["describe", "-m", "theirs"]);
            theirs.set(op_id(&other));
        })
        .unwrap();
        let theirs = theirs.into_inner();

        assert!(!ours.is_empty(), "the run moved the op head");
        assert_ne!(ours[0], theirs, "the other workspace's operation is a different op");
        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert_eq!(
            state.markers[0].entries, ours,
            "the bracket keeps the invoking workspace's entries"
        );
        assert!(
            !state.markers[0].entries.contains(&theirs),
            "the other workspace's operation is carried in the same window but is not the operation's"
        );
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&other).unwrap();
    }

    #[test]
    fn an_empty_root_brackets_from_empty() {
        let dir = repo("empty");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        assert!(!dir.join(MARKERS_PATH).exists());
        let before = oplog(&jjrun).unwrap();
        let after = Cell::new(String::new());
        run_op(&store, &jjrun, || {
            jjrun.run(&["describe", "-m", "the operation"]).unwrap();
            after.set(oplog(&jjrun).unwrap());
        })
        .unwrap();

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert_eq!(state.markers[0].entries, entries(&jjrun, &before, &after.into_inner()));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_read_aborts_before_the_run() {
        let dir = scratch("norepo");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let ran = Cell::new(false);
        let result = run_op(&store, &jjrun, || ran.set(true));
        assert!(
            matches!(&result, Err(CaptureError::Oplog(OplogError::Failed(_)))),
            "{result:?}"
        );
        assert!(!ran.get());
        assert!(!dir.join(MARKERS_PATH).exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_absent_binary_is_a_spawn_failure() {
        let dir = repo("nobinary");
        let store = store_at(&dir);
        let jjrun = Jj::new(dir.join("nope"), &dir);

        let ran = Cell::new(false);
        let result = run_op(&store, &jjrun, || ran.set(true));
        assert!(
            matches!(&result, Err(CaptureError::Oplog(OplogError::Spawn(_)))),
            "{result:?}"
        );
        assert!(!ran.get());
        assert!(!dir.join(MARKERS_PATH).exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_refused_append_is_carried() {
        let dir = repo("refused");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        fs::write(dir.join(MARKERS_PATH), "{ not json").unwrap();
        let ran = Cell::new(false);
        let result = run_op(&store, &jjrun, || {
            jjrun.run(&["describe", "-m", "the operation"]).unwrap();
            ran.set(true);
        });
        assert!(
            matches!(&result, Err(CaptureError::Append(PersistError::NotJson(_)))),
            "{result:?}"
        );
        assert!(ran.get());
        assert_eq!(
            fs::read_to_string(dir.join(MARKERS_PATH)).unwrap(),
            "{ not json"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failing_run_still_closes_its_bracket() {
        let dir = repo("failing");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let outcome: Result<(), &str> = run_op(&store, &jjrun, || {
            jjrun.run(&["describe", "-m", "the operation"]).unwrap();
            Err("the operation refused")
        })
        .unwrap();
        assert_eq!(outcome, Err("the operation refused"));

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert!(
            !state.markers[0].entries.is_empty(),
            "the head moved, so the window holds entries"
        );
        assert_eq!(state.pos, 1, "the position advances to the new length");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_bracket_records_the_operations_kind_and_target() {
        let dir = repo("kind");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        capture(&store, &jjrun, Kind::Create, || {
            jjrun.run(&["describe", "-m", "a creation"]).unwrap();
            ((), Some("kqxyzmrn".to_string()))
        })
        .unwrap();

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert_eq!(state.markers[0].kind, Kind::Create, "the kind is recorded");
        assert_eq!(state.markers[0].target, "kqxyzmrn", "the target is recorded");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_run_that_names_no_change_records_nothing() {
        // S5-record-named (wotmvzyt): an operation that names no change is not recorded — the
        // bracket closes and nothing is appended, so no marker names an empty target.
        let dir = repo("nothing-named");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let outcome: Result<(), &str> = capture(&store, &jjrun, Kind::Create, || {
            jjrun.run(&["describe", "-m", "a refused creation"]).unwrap();
            (Err("the birth refused"), None)
        })
        .unwrap();

        assert_eq!(outcome, Err("the birth refused"), "the value passes through");
        let state = load(&store).unwrap();
        assert!(state.markers.is_empty(), "nothing was appended");
        assert_eq!(state.pos, 0, "the position stays at zero");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_run_that_moves_nothing_records_nothing() {
        // S5-record-named (wotmvzyt): a bracket that kept no entries records nothing — no
        // marker is appended, so a no-op buys no un-undoable step.
        let dir = repo("nothing-moved");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        run_op(&store, &jjrun, || ()).unwrap();

        let state = load(&store).unwrap();
        assert!(state.markers.is_empty(), "nothing was appended");
        assert_eq!(state.pos, 0, "the position stands");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_run_that_moves_nothing_leaves_the_marker_file_untouched() {
        // A no-op is not an operation: the file keeps the redo tail it already held.
        let dir = repo("nothing-moved-tail");
        let store = store_at(&dir);
        let jjrun = runner(&dir);
        let seed = r#"{"markers":[{"kind":"record","target":"kqxyzmrn","entries":["aa"],"reverts":[]}],"pos":0}"#;
        fs::write(dir.join(MARKERS_PATH), seed).unwrap();

        run_op(&store, &jjrun, || ()).unwrap();

        let after = fs::read_to_string(dir.join(MARKERS_PATH)).unwrap();
        assert_eq!(after, seed, "the file is untouched: no append, no truncation");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_recording_stores_one_journal_entry_of_the_owned_set() {
        use std::collections::BTreeSet;
        use std::path::PathBuf;

        let dir = repo("journal-entry");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        jj(&dir, &["describe", "-m", "target"]);
        let target = jj(&dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"]);
        let node = format!("nodes/{target}.json");
        fs::write(
            dir.join(&node),
            format!(r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":["notes.txt"],"rationale":"r","content":"c"}}}}"#),
        )
        .unwrap();
        jj(&dir, &["status"]);
        jj(&dir, &["new", "-m", "child"]);
        fs::write(dir.join("notes.txt"), "edited\n").unwrap();
        fs::write(dir.join("unrelated.txt"), "not owned\n").unwrap();
        jj(&dir, &["status"]);

        let owned: BTreeSet<PathBuf> = [PathBuf::from(&node), PathBuf::from("notes.txt")]
            .into_iter()
            .collect();
        let outcome = crate::markers_capture::capture_recording(
            &store,
            &jjrun,
            &target,
            &owned,
            || {
                let out = jjrun
                    .run(&["squash", "--into", &target, "--", "notes.txt"])
                    .unwrap();
                assert_eq!(out.status, 0, "{}", out.stderr);
                ((), Some(target.clone()))
            },
        )
        .unwrap();
        assert_eq!(outcome, ());

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert_eq!(state.markers[0].kind, Kind::Record);
        assert_eq!(
            state.markers[0].entries.len(),
            1,
            "a recording stores one journal entry id"
        );
        let entry = crate::journal::get(&store, &state.markers[0].entries[0]).unwrap();
        assert_eq!(entry.before["notes.txt"], None, "the path was absent before");
        let after = entry.after["notes.txt"].as_deref().expect("the edit is stored");
        assert_eq!(crate::journal::content(&store, after).unwrap(), b"edited\n");
        assert!(
            entry.before.contains_key(&node),
            "the node file is part of the owned set"
        );
        assert!(
            !entry.before.contains_key("unrelated.txt"),
            "a path outside the owned set is not captured"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
