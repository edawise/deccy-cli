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

    use crate::markers::{Kind, Marker, Markers};

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
            "truncated.json",
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
                "{name} refuses an unknown field beside pos"
            );
        }
    }

    #[test]
    fn a_document_missing_a_field_is_refused() {
        for missing in ["markers", "pos"] {
            let mut short = document("appended.json");
            short.as_object_mut().unwrap().remove(missing);
            assert!(
                serde_json::from_value::<Markers>(short).is_err(),
                "{missing} is required"
            );
        }
    }

    #[test]
    fn a_marker_serializes_to_its_own_fields_alone() {
        let marker = Marker {
            kind: Kind::Record,
            target: "170a1d5be6e3ce9a73f93e8281a062b72891b75".to_string(),
            entries: vec!["170a1d5be6e3ce9a73f93e8281a062b72891b75".to_string()],
            reverts: Vec::new(),
        };
        let fields = encoded(&marker);
        let fields = fields.as_object().unwrap();
        assert_eq!(fields.len(), 4, "a marker is exactly four fields");
        for name in ["kind", "target", "entries", "reverts"] {
            assert!(fields.contains_key(name), "{name} is a marker field");
        }
        assert_eq!(fields["kind"], Value::from("record"), "the kind renders lowercase");
    }

    #[test]
    fn the_three_kinds_round_trip() {
        for (name, kind) in [
            ("create", Kind::Create),
            ("record", Kind::Record),
            ("remove", Kind::Remove),
        ] {
            let text = format!(
                "{{\"kind\":\"{name}\",\"target\":\"ab\",\"entries\":[],\"reverts\":[]}}"
            );
            let parsed: Marker = serde_json::from_str(&text).unwrap();
            assert_eq!(parsed.kind, kind, "{name} parses as its kind");
            assert_eq!(
                encoded(&parsed)["kind"],
                Value::from(name),
                "{name} renders as its kind"
            );
        }
    }

    #[test]
    fn a_marker_missing_its_kind_or_target_is_refused() {
        for missing in ["kind", "target"] {
            let mut short = document("appended.json");
            short["markers"][0]
                .as_object_mut()
                .unwrap()
                .remove(missing);
            assert!(
                serde_json::from_value::<Markers>(short).is_err(),
                "a marker requires {missing}"
            );
        }
    }

    #[test]
    fn an_unknown_kind_is_refused() {
        let mut wrong = document("appended.json");
        wrong["markers"][0]["kind"] = Value::from("archive");
        assert!(
            serde_json::from_value::<Markers>(wrong).is_err(),
            "only the three operation kinds parse"
        );
    }
}
