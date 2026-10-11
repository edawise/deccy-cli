//! Gate for M-markers (the marker state types behind schemas/markers.json).
//!
//! A crate of its own: the types derive serde, so the gate pulls in serde and
//! serde_json. Self-contained per M-tests — the shim pulls in the module under test; no
//! shared test-support module.

#[path = "../../../src/markers.rs"]
mod markers;

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use crate::markers::{Marker, Markers};

    /// The marker corpus owned by S5-corpus, read in place — named, not claimed.
    fn corpus() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/markers")
    }

    fn read(name: &str) -> String {
        fs::read_to_string(corpus().join(name)).unwrap()
    }

    /// A corpus document as a JSON value.
    fn document(name: &str) -> Value {
        serde_json::from_str(&read(name)).unwrap()
    }

    fn encoded<T: serde::Serialize>(value: &T) -> Value {
        serde_json::to_value(value).unwrap()
    }

    #[test]
    fn the_init_value_serializes_to_the_init_document() {
        assert_eq!(encoded(&Markers::new()), document("init.json"));
    }

    #[test]
    fn every_corpus_document_round_trips_through_the_types() {
        for name in [
            "init.json",
            "appended.json",
            "undone.json",
            "undone2.json",
            "newer.json",
        ] {
            let parsed: Markers = serde_json::from_str(&read(name)).unwrap();
            assert_eq!(encoded(&parsed), document(name), "{name} round-trips");
        }
    }

    #[test]
    fn a_document_with_an_unknown_field_is_refused() {
        for name in ["init.json", "undone.json"] {
            let mut extra = document(name);
            extra["extra"] = Value::from(1);
            assert!(
                serde_json::from_value::<Markers>(extra).is_err(),
                "{name} refuses an unknown field beside b"
            );
        }
    }

    #[test]
    fn a_document_missing_a_field_is_refused() {
        for missing in ["u", "b"] {
            let mut short = document("appended.json");
            short.as_object_mut().unwrap().remove(missing);
            assert!(
                serde_json::from_value::<Markers>(short).is_err(),
                "{missing} is required"
            );
        }
    }

    #[test]
    fn a_marker_pair_serializes_to_ids_alone() {
        let marker = Marker {
            pre: "170a1d5be6e3ce9a73f93e8281a062b72891b75".to_string(),
            post: "322166e9f183f9c978a5f1ba9ef8c150507aa31".to_string(),
        };
        let fields = encoded(&marker);
        let fields = fields.as_object().unwrap();
        assert_eq!(fields.len(), 2, "a marker is exactly two fields");
        assert!(fields.contains_key("pre"));
        assert!(fields.contains_key("post"));
    }
}
