//! Gate for M-markers-capture (capture: bracket an operation between the op head
//! read before it and again after).
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
#[path = "../../../src/markers_capture.rs"]
mod markers_capture;

#[cfg(test)]
mod gate {
    use std::cell::Cell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::jj::Jj;
    use crate::markers_capture::{capture, CaptureError};
    use crate::markers_persist::{load, PersistError, MARKERS_PATH};
    use crate::oplog::{oplog, OplogError};
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

    fn store_at(dir: &Path) -> Store {
        Store::new(dir)
    }

    #[test]
    fn a_bracketed_run_keeps_its_pair() {
        let dir = repo("pair");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let before = oplog(&jjrun).unwrap();
        let after = Cell::new(String::new());
        let value = capture(&store, &jjrun, || {
            let out = jjrun.run(&["describe", "-m", "the operation"]).unwrap();
            assert_eq!(out.status, 0);
            after.set(oplog(&jjrun).unwrap());
            "done"
        })
        .unwrap();
        let after = after.into_inner();

        assert_eq!(value, "done");
        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        let pair = &state.markers[0];
        assert_eq!(pair.pre, before);
        assert_eq!(pair.post, after);
        assert_ne!(pair.pre, pair.post);
        assert_eq!(state.at, after);
        assert_eq!(state.u, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pairs_land_in_run_order() {
        let dir = repo("order");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let mut rounds = Vec::new();
        for i in 0..3 {
            let before = oplog(&jjrun).unwrap();
            let after = Cell::new(String::new());
            capture(&store, &jjrun, || {
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
        for (pair, (before, after)) in state.markers.iter().zip(rounds.iter()) {
            assert_eq!(&pair.pre, before);
            assert_eq!(&pair.post, after);
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_root_brackets_from_empty() {
        let dir = repo("empty");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        assert!(!dir.join(MARKERS_PATH).exists());
        let before = oplog(&jjrun).unwrap();
        let after = Cell::new(String::new());
        capture(&store, &jjrun, || {
            jjrun.run(&["describe", "-m", "the operation"]).unwrap();
            after.set(oplog(&jjrun).unwrap());
        })
        .unwrap();

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        assert_eq!(state.markers[0].pre, before);
        assert_eq!(state.markers[0].post, after.into_inner());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_read_aborts_before_the_run() {
        let dir = scratch("norepo");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let ran = Cell::new(false);
        let result = capture(&store, &jjrun, || ran.set(true));
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
        let result = capture(&store, &jjrun, || ran.set(true));
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
        let result = capture(&store, &jjrun, || {
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

        let outcome: Result<(), &str> = capture(&store, &jjrun, || {
            jjrun.run(&["describe", "-m", "the operation"]).unwrap();
            Err("the operation refused")
        })
        .unwrap();
        assert_eq!(outcome, Err("the operation refused"));

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        let pair = &state.markers[0];
        assert_ne!(pair.pre, pair.post);
        assert_eq!(state.u, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_run_that_moves_nothing_still_brackets() {
        // A run that moves no jj operation still closes its bracket — no degenerate pair
        // is special-cased.
        let dir = repo("still");
        let store = store_at(&dir);
        let jjrun = runner(&dir);

        let head = oplog(&jjrun).unwrap();
        capture(&store, &jjrun, || ()).unwrap();

        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 1);
        let pair = &state.markers[0];
        assert_eq!(pair.pre, head);
        assert_eq!(pair.post, head);
        assert_eq!(state.at, head);
        fs::remove_dir_all(&dir).unwrap();
    }
}
