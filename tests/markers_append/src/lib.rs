//! Gate for M-markers-append (append the operation's kept entry ids to the live marker file).
//!
//! A crate of its own: append pulls in the persist module, which pulls in
//! serde_json and the marker schema validator, which plain `rustc --test` cannot
//! supply. Self-contained per M-tests — the shims pull in the module under test
//! and its siblings; no shared test-support module. It reads the marker corpus by
//! path and claims it not; the marker schema is carried in the binary (`include_str!`).

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

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use crate::markers::Kind;
    use crate::markers_append::append;
    use crate::markers_persist::{load, PersistError, MARKERS_PATH};
    use crate::store::Store;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-markers-append-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A scratch store root with an empty `nodes/` beside it, and the state directory the marker
    /// file lives in.
    fn root(name: &str) -> PathBuf {
        let dir = scratch(name);
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        dir
    }

    /// The repo corpus document `<name>` under tests/fixtures/markers.
    fn corpus(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/markers")
            .join(format!("{name}.json"))
    }

    fn store_at(root: &Path) -> Store {
        Store::new(root)
    }

    #[test]
    fn first_append_lands_the_appended_document() {
        let r = root("first");
        let doc: Value =
            serde_json::from_str(&fs::read_to_string(corpus("appended")).unwrap()).unwrap();
        let entries: Vec<String> = doc["markers"][0]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().to_string())
            .collect();
        let store = store_at(&r);
        append(&store, Kind::Record, "kqxyzmrn", &entries).unwrap();
        assert_eq!(serde_json::to_value(load(&store).unwrap()).unwrap(), doc);
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn the_marker_lands_last_in_file_order() {
        let r = root("order");
        fs::write(
            r.join(MARKERS_PATH),
            r#"{"markers":[{"kind":"record","target":"kqxyzmrn","entries":["01"],"reverts":[]},{"kind":"record","target":"kqxyzmrn","entries":["03"],"reverts":[]}],"pos":2}"#,
        )
        .unwrap();
        let store = store_at(&r);
        append(&store, Kind::Record, "kqxyzmrn", &["05".to_string()]).unwrap();
        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 3);
        assert_eq!(state.pos, 3, "the position advances to the new length");
        assert_eq!(
            (
                state.markers[0].entries[0].as_str(),
                state.markers[1].entries[0].as_str()
            ),
            ("01", "03")
        );
        assert_eq!(state.markers[2].entries, ["05"]);
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn the_undone_tail_is_discarded_and_the_position_advances() {
        let r = root("undone");
        fs::copy(corpus("undone"), r.join(MARKERS_PATH)).unwrap();
        let store = store_at(&r);
        append(&store, Kind::Record, "rnkqxyzm", &["cc".to_string()]).unwrap();
        let state = load(&store).unwrap();
        assert_eq!(
            state.markers.len(),
            1,
            "the undone tail is discarded before the append"
        );
        assert_eq!(
            state.markers[0].target, "rnkqxyzm",
            "only the new operation's record remains"
        );
        assert_eq!(state.pos, 1, "the position advances to the new length");
        assert_eq!(state.markers[0].entries, ["cc"]);
        assert!(
            state.markers[0].reverts.is_empty(),
            "a fresh marker records no reverts of its own"
        );
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn a_corrupt_file_is_refused() {
        let r = root("notjson");
        fs::write(r.join(MARKERS_PATH), "{ not json").unwrap();
        let error = append(&store_at(&r), Kind::Record, "kqxyzmrn", &["aa".to_string()]).unwrap_err();
        assert!(format!("{error:?}").contains("NotJson"), "{error:?}");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn a_nonconforming_file_is_refused() {
        let r = root("invalid");
        fs::write(
            r.join(MARKERS_PATH),
            r#"{"markers":[{"entries":["not-hex"],"reverts":[]}],"pos":0}"#,
        )
        .unwrap();
        let error = append(&store_at(&r), Kind::Record, "kqxyzmrn", &["aa".to_string()]).unwrap_err();
        assert!(matches!(error, PersistError::Invalid(_)), "{error:?}");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn an_append_leaves_the_file_and_no_temporary() {
        let r = root("stray");
        append(&store_at(&r), Kind::Record, "kqxyzmrn", &["aa".to_string()]).unwrap();
        assert!(r.join(MARKERS_PATH).is_file());
        let mut names: Vec<String> = fs::read_dir(&r)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, [".deccy", "nodes"]);
        fs::remove_dir_all(&r).unwrap();
    }
}
