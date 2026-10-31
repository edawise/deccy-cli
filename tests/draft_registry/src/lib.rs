//! Gate for M-draft-registry (the draft record store).
//!
//! A crate of its own: the store needs serde_json and the in-process JSON Schema
//! validator, which plain `rustc --test` cannot supply. Self-contained per M-tests — the
//! shim pulls in the flat module under test and this crate defines its own fixtures and
//! helpers. The gate drives the module over a fresh temporary directory, so the store's
//! storage, naming, discovery and refusal clauses are observed as files on disk: one JSON
//! document per record at `<root>/<handle>.json`, its filename the handle, found by
//! reading the directory and read only through the record schema. The registry root is the
//! first write's to create: the module is handed a root that is not there and the record it
//! stores is what makes the root a registry.

#[path = "../../../src/draft_registry.rs"]
mod draft_registry;

#[cfg(test)]
mod gate {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};

    use crate::draft_registry::{path_for, read, scan, write, Error};

    /// A fresh directory for one test. Most tests hand it to the module as the registry
    /// root; a test that needs a root the first write must create asks for `absent_root`.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-draft-registry-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the scratch directory is created");
        dir
    }

    /// The registry root for one test, deliberately not created: the first write is what
    /// must make it, so the root is the observable the root-creation clauses are checked on.
    fn absent_root(name: &str) -> PathBuf {
        scratch(name).join("work")
    }

    /// A conforming record of the shape `schemas/work.json` fixes.
    fn record(handle: &str) -> Value {
        json!({
            "handle": handle,
            "state": "open",
            "node": "rroqqrzq",
            "edit": "uplqqqrs",
            "workspace": {
                "name": format!("draft-{handle}"),
                "path": format!(".deccy/agent/workspaces/drafts/{handle}"),
            },
            "base": {
                "src/draft_registry.rs": "0f1e2d3c",
                "schemas/work.json": null,
            },
        })
    }

    /// Write `document` at the path `handle` names, bypassing `write` so a fixture can be
    /// non-conforming. The path itself is the module's contract.
    fn put_raw(root: &Path, handle: &str, document: &Value) {
        let text = serde_json::to_vec_pretty(document).expect("the fixture is JSON");
        fs::write(path_for(root, handle), text).expect("the fixture is written directly");
    }

    /// The handle stems of every `.json` file under `root`.
    fn json_stems(root: &Path) -> Vec<String> {
        fs::read_dir(root)
            .expect("the scratch directory is readable")
            .filter_map(|entry| {
                let entry = entry.expect("the directory entry is readable");
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".json").map(|stem| stem.to_string())
            })
            .collect()
    }

    #[test]
    fn path_for_is_exactly_the_handle_under_root() {
        let root = scratch("path");
        assert_eq!(path_for(&root, "k4mz7qta"), root.join("k4mz7qta.json"));
        assert_eq!(
            path_for(&root, "k4mz7qta").file_name().unwrap(),
            std::ffi::OsStr::new("k4mz7qta.json")
        );
    }

    #[test]
    fn write_then_read_round_trips() {
        let root = scratch("round-trip");
        let document = record("k4mz7qta");
        write(&root, &document).expect("the record is written");
        assert_eq!(read(&root, "k4mz7qta").expect("the record reads"), document);
    }

    #[test]
    fn a_record_is_stored_at_exactly_its_handle_json() {
        let root = scratch("naming");
        write(&root, &record("k4mz7qta")).expect("the record is written");
        assert!(path_for(&root, "k4mz7qta").is_file());
        assert_eq!(json_stems(&root), vec!["k4mz7qta".to_string()]);
    }

    #[test]
    fn two_handles_leave_two_documents() {
        let root = scratch("two");
        write(&root, &record("k4mz7qta")).expect("the first record is written");
        write(&root, &record("uqwnzmtb")).expect("the second record is written");
        let mut stems = json_stems(&root);
        stems.sort();
        assert_eq!(stems, vec!["k4mz7qta".to_string(), "uqwnzmtb".to_string()]);
        assert_eq!(read(&root, "k4mz7qta").unwrap(), record("k4mz7qta"));
        assert_eq!(read(&root, "uqwnzmtb").unwrap(), record("uqwnzmtb"));
    }

    #[test]
    fn a_re_write_replaces_instead_of_appending() {
        let root = scratch("replace");
        write(&root, &record("k4mz7qta")).expect("the first write lands");
        let mut second = record("k4mz7qta");
        second["state"] = json!("landed");
        write(&root, &second).expect("the second write lands");
        assert_eq!(
            read(&root, "k4mz7qta").unwrap(),
            second,
            "one document holds the record, so a second write replaces it"
        );
        assert_eq!(json_stems(&root), vec!["k4mz7qta".to_string()]);
    }

    #[test]
    fn the_first_write_creates_the_registry_root_and_stores_the_record() {
        let parent = scratch("first-write");
        let root = parent.join("work");
        assert!(
            !root.exists(),
            "the fixture registry root is absent before the first write"
        );

        let document = record("k4mz7qta");
        write(&root, &document)
            .expect("the first write creates the registry root and stores the record");

        assert!(root.is_dir(), "the first write creates the registry root");
        let path = path_for(&root, "k4mz7qta");
        assert!(path.is_file(), "the record lands at <root>/<handle>.json");
        assert_eq!(
            json_stems(&root),
            vec!["k4mz7qta".to_string()],
            "the root holds exactly the one record"
        );
        assert_eq!(
            read(&root, "k4mz7qta").expect("the record the first write stored reads back"),
            document
        );
        assert_eq!(
            scan(&root).expect("the registry the first write created scans"),
            vec![("k4mz7qta".to_string(), document.clone())],
            "scan finds the record the first write stored"
        );

        // What is stored is the record itself: the bytes under a root the write created are
        // the bytes of the same document written under a root that already existed, so
        // creating the root adds nothing to and takes nothing from the stored record.
        let stored = fs::read(&path).expect("the stored document reads as bytes");
        let already_there = scratch("first-write-existing");
        write(&already_there, &document).expect("the same document is written under a root that exists");
        assert_eq!(
            stored,
            fs::read(path_for(&already_there, "k4mz7qta")).expect("the second copy reads as bytes"),
            "creating the root does not change the document that is stored"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&stored).expect("the stored bytes are JSON"),
            document,
            "the stored document is the record that was written"
        );
    }

    #[test]
    fn a_second_write_to_the_created_root_replaces_the_record() {
        let root = absent_root("second-write");
        write(&root, &record("k4mz7qta"))
            .expect("the first write creates the root and stores the record");
        assert!(root.is_dir(), "the first write creates the registry root");

        let mut second = record("k4mz7qta");
        second["state"] = json!("landed");
        write(&root, &second).expect("a second write lands in the root the first write created");

        assert_eq!(
            read(&root, "k4mz7qta").expect("the replaced record reads"),
            second,
            "one document holds the record, so the second write replaces it"
        );
        assert_eq!(
            json_stems(&root),
            vec!["k4mz7qta".to_string()],
            "the second write replaces rather than appends"
        );
    }

    #[test]
    fn a_refused_first_write_leaves_no_record() {
        let root = absent_root("refused-first-write");
        let mut document = record("k4mz7qta");
        document.as_object_mut().unwrap().remove("state");

        let error = write(&root, &document).expect_err("a document that fails the schema is refused");
        match &error {
            Error::Invalid(reported, _) => assert_eq!(
                reported,
                &path_for(&root, "k4mz7qta"),
                "the refusal names the path the document would have been stored at"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }

        // What the check sees after the refusal: no record document under the root and
        // nothing for `scan` to return. Whether the refused write got as far as creating the
        // root is not asserted — the first-write clause is about the write that stores a
        // record, and it is not a rule about what a refused write leaves behind.
        assert!(
            !path_for(&root, "k4mz7qta").exists(),
            "the refused document is not stored"
        );
        if root.exists() {
            assert_eq!(
                json_stems(&root),
                Vec::<String>::new(),
                "the refused write leaves no document in the root"
            );
        }
        assert_eq!(
            scan(&root).expect("a registry with no documents scans to nothing"),
            Vec::new(),
            "the refused document is not a record a scan finds"
        );
    }

    #[test]
    fn a_landed_record_reads_like_an_open_one() {
        let root = scratch("landed");
        let mut document = record("k4mz7qta");
        document["state"] = json!("landed");
        write(&root, &document).expect("a landed record is written");
        assert_eq!(read(&root, "k4mz7qta").unwrap(), document);
    }

    #[test]
    fn scan_of_an_absent_directory_is_empty() {
        let root = scratch("scan-absent");
        let absent = root.join("not-yet-created");
        assert_eq!(
            scan(&absent).expect("an absent registry holds no records"),
            Vec::new()
        );
    }

    #[test]
    fn scan_finds_every_json_document_keyed_by_its_handle() {
        let root = scratch("scan-all");
        write(&root, &record("k4mz7qta")).expect("a record is written");
        let mut landed = record("uqwnzmtb");
        landed["state"] = json!("landed");
        write(&root, &landed).expect("a landed record is written");
        // A document placed by hand, not through `write`, is still a record: discovery
        // reads the directory, not an index or a record of previous writes.
        put_raw(&root, "vbnmklpo", &record("vbnmklpo"));

        let found: BTreeMap<String, Value> = scan(&root)
            .expect("the registry scans")
            .into_iter()
            .collect();
        let mut expected = BTreeMap::new();
        expected.insert("k4mz7qta".to_string(), record("k4mz7qta"));
        expected.insert("uqwnzmtb".to_string(), landed);
        expected.insert("vbnmklpo".to_string(), record("vbnmklpo"));
        assert_eq!(found, expected, "every `.json` document is found, keyed by its handle");
    }

    #[test]
    fn scan_refuses_a_non_conforming_document_instead_of_dropping_it() {
        let root = scratch("scan-invalid");
        write(&root, &record("k4mz7qta")).expect("a conforming record is written");
        // A document that does not conform to the record schema sits beside a conforming
        // one. `read` refuses it; discovery goes through the same read for every document,
        // so `scan` refuses the directory instead of returning the good one and dropping
        // the bad one.
        let mut broken = record("uqwnzmtb");
        broken.as_object_mut().unwrap().remove("state");
        put_raw(&root, "uqwnzmtb", &broken);
        let path = path_for(&root, "uqwnzmtb");
        assert!(
            read(&root, "uqwnzmtb").is_err(),
            "the fixture is a document `read` refuses"
        );

        let error = scan(&root)
            .expect_err("a document `read` refuses is refused by `scan`, not silently dropped");
        match &error {
            Error::Invalid(reported, _) => {
                assert_eq!(reported, &path, "the refusal carries the path it refused")
            }
            other => panic!("expected the refusal `read` gives, got {other:?}"),
        }
        assert!(
            format!("{error}").contains(&path.display().to_string()),
            "the refusal message names the offending path: {error}"
        );
    }

    #[test]
    fn scan_refuses_a_document_whose_handle_is_not_its_filename() {
        let root = scratch("scan-handle-mismatch");
        write(&root, &record("k4mz7qta")).expect("a conforming record is written");
        // The document parses and looks conforming, but its `handle` is not the handle its
        // filename gives, so `read` refuses it. Discovery must not accept it unchecked.
        let mut mismatched = record("uqwnzmtb");
        mismatched["handle"] = json!("vbnmklpo");
        put_raw(&root, "uqwnzmtb", &mismatched);
        let path = path_for(&root, "uqwnzmtb");
        assert!(
            read(&root, "uqwnzmtb").is_err(),
            "the filename is the handle, so `read` refuses the document"
        );

        let error = scan(&root)
            .expect_err("a document `read` refuses is refused by `scan`, not returned unchecked");
        match &error {
            Error::HandleMismatch { path: reported, .. } => {
                assert_eq!(reported, &path, "the refusal carries the path it refused")
            }
            other => panic!("expected HandleMismatch, got {other:?}"),
        }
        assert!(
            format!("{error}").contains(&path.display().to_string()),
            "the refusal message names the offending path: {error}"
        );
    }

    #[test]
    fn a_missing_record_is_a_named_refusal() {
        let root = scratch("missing");
        let error = read(&root, "k4mz7qta").expect_err("an absent record is refused");
        assert!(
            matches!(&error, Error::Missing(_)),
            "an absent record is a Missing refusal: {error:?}"
        );
    }

    #[test]
    fn a_document_missing_a_required_field_is_refused_naming_the_path() {
        let root = scratch("missing-field");
        let mut document = record("k4mz7qta");
        document.as_object_mut().unwrap().remove("state");
        put_raw(&root, "k4mz7qta", &document);

        let path = path_for(&root, "k4mz7qta");
        let error = read(&root, "k4mz7qta").expect_err("a non-conforming record is refused");
        match &error {
            Error::Invalid(reported, _) => {
                assert_eq!(reported, &path, "the refusal carries the path it refused")
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        assert!(
            format!("{error}").contains(&path.display().to_string()),
            "the refusal message names the path: {error}"
        );
    }

    #[test]
    fn a_document_with_an_unknown_field_is_refused() {
        let root = scratch("unknown-field");
        let mut document = record("k4mz7qta");
        document["version"] = json!(1);
        put_raw(&root, "k4mz7qta", &document);
        let error = read(&root, "k4mz7qta").expect_err("the record schema is closed");
        assert!(
            matches!(&error, Error::Invalid(_, _)),
            "an unknown field is a schema refusal: {error:?}"
        );
    }

    #[test]
    fn a_document_whose_handle_is_not_its_filename_is_refused() {
        let root = scratch("handle-mismatch");
        let mut document = record("k4mz7qta");
        document["handle"] = json!("uqwnzmtb");
        put_raw(&root, "k4mz7qta", &document);

        let error = read(&root, "k4mz7qta").expect_err("the filename is the handle");
        match &error {
            Error::HandleMismatch {
                path,
                filename,
                handle,
            } => {
                assert_eq!(path, &path_for(&root, "k4mz7qta"));
                assert_eq!(filename, "k4mz7qta");
                assert_eq!(handle, "uqwnzmtb");
            }
            other => panic!("expected HandleMismatch, got {other:?}"),
        }
    }

    #[test]
    fn a_refusal_is_an_error_value_not_a_panic_and_not_a_skip() {
        let root = scratch("refusal");
        assert!(read(&root, "k4mz7qta").is_err(), "absent is Err");

        put_raw(&root, "k4mz7qta", &json!({"handle": "k4mz7qta"}));
        assert!(read(&root, "k4mz7qta").is_err(), "missing fields are Err");

        let path = path_for(&root, "k4mz7qta");
        fs::write(&path, b"not json").expect("the malformed fixture is written");
        let error = read(&root, "k4mz7qta").expect_err("malformed JSON is refused");
        assert!(
            matches!(&error, Error::NotJson(_, _)),
            "malformed JSON is a NotJson refusal: {error:?}"
        );

        // The refusal is stable: a second read of the same directory still answers Err and
        // does not panic or silently hand back a record.
        assert!(read(&root, "k4mz7qta").is_err(), "a refusal is repeatable");
    }
}
