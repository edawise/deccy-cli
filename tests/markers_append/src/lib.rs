//! Gate for M-markers-append (append a bracketed pair to the live marker file).
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
        let pre = doc["markers"][0]["pre"].as_str().unwrap();
        let post = doc["markers"][0]["post"].as_str().unwrap();
        let store = store_at(&r);
        append(&store, pre, post).unwrap();
        assert_eq!(serde_json::to_value(load(&store).unwrap()).unwrap(), doc);
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn the_pair_lands_last_in_file_order() {
        let r = root("order");
        fs::write(
            r.join(MARKERS_PATH),
            r#"{"markers":[{"pre":"01","post":"02"},{"pre":"03","post":"04"}],"u":0,"b":0,"at":"04"}"#,
        )
        .unwrap();
        let store = store_at(&r);
        append(&store, "05", "06").unwrap();
        let state = load(&store).unwrap();
        assert_eq!(state.markers.len(), 3);
        assert_eq!((state.markers[0].pre.as_str(), state.markers[1].pre.as_str()), ("01", "03"));
        assert_eq!(
            (state.markers[2].pre.as_str(), state.markers[2].post.as_str()),
            ("05", "06")
        );
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn the_undone_count_is_cleared_and_the_bound_stands() {
        let r = root("undone");
        fs::copy(corpus("undone"), r.join(MARKERS_PATH)).unwrap();
        let store = store_at(&r);
        append(&store, "cc", "dd").unwrap();
        let state = load(&store).unwrap();
        assert_eq!(state.u, 0, "the undone count is cleared");
        assert_eq!(state.b, 1, "the bound stands");
        assert_eq!(state.markers.len(), 2);
        assert_eq!(
            (state.markers[1].pre.as_str(), state.markers[1].post.as_str()),
            ("cc", "dd")
        );
        assert_eq!(state.at, "dd");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn the_position_lands_on_the_new_pairs_post_boundary() {
        let r = root("position");
        let store = store_at(&r);
        append(&store, "aa", "bb").unwrap();
        let state = load(&store).unwrap();
        assert_eq!(state.at, "bb");
        assert_ne!(state.at, "aa");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn a_corrupt_file_is_refused() {
        let r = root("notjson");
        fs::write(r.join(MARKERS_PATH), "{ not json").unwrap();
        let error = append(&store_at(&r), "aa", "bb").unwrap_err();
        assert!(format!("{error:?}").contains("NotJson"), "{error:?}");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn a_nonconforming_file_is_refused() {
        let r = root("invalid");
        fs::write(r.join(MARKERS_PATH), r#"{"markers":[],"u":0,"b":0,"at":"not-hex"}"#).unwrap();
        let error = append(&store_at(&r), "aa", "bb").unwrap_err();
        assert!(matches!(error, PersistError::Invalid(_)), "{error:?}");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn an_append_leaves_the_file_and_no_temporary() {
        let r = root("stray");
        append(&store_at(&r), "aa", "bb").unwrap();
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
