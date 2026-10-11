//! Gate for M-markers-persist (load and save markers.json).
//!
//! A crate of its own: load and save pull in serde_json and the schema
//! validator, which plain `rustc --test` cannot supply. Self-contained per M-tests —
//! the shims pull in the module under test and its siblings; no shared test-support
//! module. It reads the marker corpus by path and claims it not; the marker schema
//! is carried in the binary (`include_str!`), never read from the repo.

#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/markers.rs"]
mod markers;
#[path = "../../../src/markers_file.rs"]
mod markers_file;
#[path = "../../../src/markers_persist.rs"]
mod markers_persist;

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use crate::markers::{Marker, Markers};
    use crate::markers_persist::{load, save, PersistError, MARKERS_PATH};
    use crate::store::Store;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-markers-persist-{}-{}",
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
    fn a_missing_file_reads_as_empty() {
        let r = root("missing");
        assert_eq!(load(&store_at(&r)).unwrap(), Markers::new());
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn the_file_sits_beside_nodes() {
        let r = root("beside");
        assert_eq!(MARKERS_PATH, ".deccy/markers.json");
        save(&store_at(&r), &Markers::new()).unwrap();
        assert!(r.join(MARKERS_PATH).is_file());
        assert_eq!(fs::read_dir(r.join("nodes")).unwrap().count(), 0);
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn pairs_and_counts_round_trip() {
        let r = root("roundtrip");
        let store = store_at(&r);
        let state = Markers {
            markers: vec![
                Marker {
                    pre: "aa".into(),
                    post: "bb".into(),
                },
                Marker {
                    pre: "cc".into(),
                    post: "dd".into(),
                },
            ],
            u: 2,
            b: 0,
            at: "cc".into(),
        };
        save(&store, &state).unwrap();
        assert_eq!(load(&store).unwrap(), state);
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn pairs_load_in_file_order() {
        let r = root("order");
        fs::write(
            r.join(MARKERS_PATH),
            r#"{"markers":[{"pre":"01","post":"02"},{"pre":"03","post":"04"}],"u":0,"b":0,"at":"0"}"#,
        )
        .unwrap();
        let state = load(&store_at(&r)).unwrap();
        assert_eq!(
            state.markers,
            vec![
                Marker {
                    pre: "01".into(),
                    post: "02".into(),
                },
                Marker {
                    pre: "03".into(),
                    post: "04".into(),
                },
            ]
        );
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn the_corpus_documents_load() {
        for name in ["init", "appended", "undone", "undone2", "newer"] {
            let r = root(&format!("corpus-{name}"));
            let doc = corpus(name);
            fs::copy(&doc, r.join(MARKERS_PATH)).unwrap();
            let state = load(&store_at(&r)).unwrap();
            let expected: Value =
                serde_json::from_str(&fs::read_to_string(&doc).unwrap()).unwrap();
            assert_eq!(serde_json::to_value(&state).unwrap(), expected, "{name}");
            fs::remove_dir_all(&r).unwrap();
        }
    }

    #[test]
    fn a_file_that_does_not_parse_is_refused() {
        let r = root("notjson");
        fs::write(r.join(MARKERS_PATH), "{ not json").unwrap();
        let error = load(&store_at(&r)).unwrap_err();
        assert!(format!("{error:?}").contains("NotJson"), "{error:?}");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn a_nonconforming_file_is_refused() {
        let r = root("invalid");
        fs::write(r.join(MARKERS_PATH), r#"{"markers":[],"u":0,"b":0,"at":"not-hex"}"#).unwrap();
        let error = load(&store_at(&r)).unwrap_err();
        assert!(matches!(error, PersistError::Invalid(_)), "{error:?}");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn an_empty_file_is_not_a_missing_one() {
        let r = root("empty");
        fs::write(r.join(MARKERS_PATH), "").unwrap();
        let error = load(&store_at(&r)).unwrap_err();
        assert!(matches!(error, PersistError::NotJson(_)), "{error:?}");
        fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn a_save_leaves_the_file_and_no_temporary() {
        let r = scratch("stray");
        fs::create_dir_all(r.join(".deccy")).unwrap();
        save(&store_at(&r), &Markers::new()).unwrap();
        let names: Vec<String> = fs::read_dir(r.join(".deccy"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["markers.json"]);
        fs::remove_dir_all(&r).unwrap();
    }
}
