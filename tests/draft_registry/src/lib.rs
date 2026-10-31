//! Gate for M-draft-registry (the draft record store).
//!
//! A crate of its own: the store needs serde_json and the in-process JSON Schema
//! validator, which plain `rustc --test` cannot supply. Self-contained per M-tests — the
//! shim pulls in the flat module under test and this crate defines its own fixtures and
//! helpers. The gate drives the module over a fresh temporary directory, so the store's
//! storage, naming, discovery, removal and refusal clauses are observed as files on disk: one JSON
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

    use crate::draft_registry::{path_for, read, remove, scan, write, Error};

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

    /// A handle is any non-empty string, not a filename stem: a handle carrying a dot is
    /// stored under exactly `<handle>.json`. A path builder that treats the handle as a stem
    /// (`with_extension`) would strip the handle's last dotted part and name another file.
    #[test]
    fn path_for_keeps_a_dotted_handle_whole() {
        let root = scratch("path-dotted");
        assert_eq!(path_for(&root, "a.b"), root.join("a.b.json"));
        assert_eq!(
            path_for(&root, "a.b").file_name().unwrap(),
            std::ffi::OsStr::new("a.b.json")
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
    fn remove_takes_exactly_the_document_the_handle_names() {
        let root = scratch("remove-one");
        write(&root, &record("k4mz7qta")).expect("the record is written");
        let path = path_for(&root, "k4mz7qta");
        assert!(path.is_file(), "the record is there before the removal");

        remove(&root, "k4mz7qta").expect("the open record is removed");

        assert!(
            !path.exists(),
            "the one document at <root>/<handle>.json is removed"
        );
        assert_eq!(
            scan(&root).expect("the registry scans after the removal"),
            Vec::new(),
            "the removed record is not a record a scan finds"
        );
        assert!(
            matches!(read(&root, "k4mz7qta"), Err(Error::Missing(_))),
            "the removed record is absent, not a record of some third state"
        );
    }

    /// Removal deletes exactly the document the handle names and nothing else — not even the
    /// registry root it empties. The decision makes the document, not the directory, the unit
    /// of removal, so a removal that trims the now-empty root changes state the caller did not
    /// name. A scan of an absent root is `Ok(Vec::new())`, so deleting the root is invisible
    /// to a scan; the check observes the root itself, then that the same root still works for
    /// a later record.
    #[test]
    fn remove_leaves_the_registry_root_in_place_when_the_last_record_goes() {
        let root = absent_root("remove-keeps-root");
        write(&root, &record("k4mz7qta")).expect("the first write creates the root and the record");
        assert!(root.is_dir(), "the fixture root exists before the removal");

        remove(&root, "k4mz7qta").expect("the one record is removed");

        assert!(
            root.exists(),
            "removal deletes the document at the handle's path, not the registry root"
        );
        assert!(
            root.is_dir(),
            "the registry root is still the directory it was after its last record goes"
        );
        assert_eq!(
            scan(&root).expect("the emptied registry still scans"),
            Vec::new(),
            "the registry now holds no records"
        );

        // The root that survived is still a working registry: a later record is stored, read,
        // and removed under the same root.
        write(&root, &record("uqwnzmtb")).expect("a later write lands in the surviving root");
        assert_eq!(
            read(&root, "uqwnzmtb").expect("the later record reads"),
            record("uqwnzmtb")
        );
        remove(&root, "uqwnzmtb").expect("the later record is removed");
        assert!(
            !path_for(&root, "uqwnzmtb").exists(),
            "the later removal takes its document"
        );
        assert!(
            root.is_dir(),
            "the root still exists after a second removal empties it"
        );
    }

    #[test]
    fn remove_of_a_dotted_handle_takes_exactly_its_document() {
        let root = scratch("remove-dotted");
        // The handle `a.b` names `<root>/a.b.json`; the handle `a` names `<root>/a.json`.
        // A path builder that treats the handle as a filename stem would collapse the first
        // onto the second, so both documents are placed independently of `path_for`.
        let dotted = record("a.b");
        let plain = record("a");
        fs::write(
            root.join("a.b.json"),
            serde_json::to_vec_pretty(&dotted).expect("the dotted record is JSON"),
        )
        .expect("the dotted handle's document is placed");
        fs::write(
            root.join("a.json"),
            serde_json::to_vec_pretty(&plain).expect("the plain record is JSON"),
        )
        .expect("the plain handle's document is placed");
        assert_eq!(
            read(&root, "a.b").expect("the dotted record reads"),
            dotted
        );

        remove(&root, "a.b").expect("the dotted handle names its own document");

        assert!(
            !root.join("a.b.json").exists(),
            "`a.b.json` is the document the dotted handle names and is removed"
        );
        assert!(
            root.join("a.json").is_file(),
            "`a.json` belongs to the handle `a`, not to `a.b`, and stays where it was"
        );
        assert_eq!(
            read(&root, "a").expect("the plain record still reads"),
            plain,
            "the removal did not take the plain handle's document"
        );
        assert!(
            matches!(read(&root, "a.b"), Err(Error::Missing(_))),
            "the dotted record is absent after the removal, not a different document"
        );
    }

    #[test]
    fn remove_takes_a_hand_placed_document_too() {
        let root = scratch("remove-hand-placed");
        let document = record("k4mz7qta");
        put_raw(&root, "k4mz7qta", &document);
        assert_eq!(
            read(&root, "k4mz7qta").expect("the hand-placed record reads"),
            document,
            "the fixture is a record the store finds"
        );

        remove(&root, "k4mz7qta")
            .expect("removal names the handle's path, not a list of previous writes");

        assert!(
            !path_for(&root, "k4mz7qta").exists(),
            "the hand-placed document is removed"
        );
    }

    #[test]
    fn remove_does_not_read_the_rest_of_the_directory() {
        let root = scratch("remove-path-only");
        write(&root, &record("k4mz7qta")).expect("the record to remove is written");
        // A sibling `read` refuses. Removal names one path and leaves every other document
        // where it was, so it neither fails on nor repairs a document it was not asked about.
        let mut broken = record("uqwnzmtb");
        broken.as_object_mut().unwrap().remove("state");
        put_raw(&root, "uqwnzmtb", &broken);
        let broken_path = path_for(&root, "uqwnzmtb");
        assert!(
            read(&root, "uqwnzmtb").is_err(),
            "the sibling is a document `read` refuses"
        );

        remove(&root, "k4mz7qta")
            .expect("the removal is a path operation and does not read the directory");

        assert!(
            !path_for(&root, "k4mz7qta").exists(),
            "the document the handle names is removed"
        );
        assert!(
            broken_path.is_file(),
            "the refused sibling is left exactly where it was"
        );
        assert!(
            read(&root, "uqwnzmtb").is_err(),
            "the refused sibling is still refused, not silently repaired"
        );
    }

    #[test]
    fn remove_discards_a_non_json_target_without_interpreting_it() {
        let root = scratch("remove-non-json-target");
        // The target is a document `read` refuses: it is not JSON at all. A discarded draft's
        // record may be in any state, so removal is a path operation and must not read,
        // parse or validate the document before deleting it.
        let target = path_for(&root, "k4mz7qta");
        fs::write(&target, b"this is not a draft record").expect("the malformed target is written");
        assert!(
            read(&root, "k4mz7qta").is_err(),
            "the target is a document `read` refuses"
        );

        // A conforming sibling is the observable that the removal touched only its one path.
        let sibling = record("uqwnzmtb");
        put_raw(&root, "uqwnzmtb", &sibling);
        let sibling_path = path_for(&root, "uqwnzmtb");
        let sibling_bytes = fs::read(&sibling_path).expect("the sibling reads as bytes");

        remove(&root, "k4mz7qta").expect(
            "the document at the path the handle names is removed without being interpreted",
        );

        assert!(!target.exists(), "the non-conforming target is removed");
        assert_eq!(
            fs::read(&sibling_path).expect("the sibling still reads as bytes"),
            sibling_bytes,
            "the conforming sibling is left byte-identical"
        );
        assert_eq!(
            read(&root, "uqwnzmtb").expect("the sibling reads"),
            sibling,
            "the conforming sibling is still a record"
        );
    }

    #[test]
    fn remove_discards_a_schema_invalid_target_without_interpreting_it() {
        let root = scratch("remove-invalid-target");
        // The target parses but does not conform to `schemas/work.json`. Removal must not turn
        // a malformed discard into a refusal: the caller discards the record, form or no form.
        let mut invalid = record("k4mz7qta");
        invalid.as_object_mut().unwrap().remove("state");
        put_raw(&root, "k4mz7qta", &invalid);
        assert!(
            read(&root, "k4mz7qta").is_err(),
            "the target is a document that fails the record schema"
        );

        let sibling = record("uqwnzmtb");
        put_raw(&root, "uqwnzmtb", &sibling);
        let sibling_path = path_for(&root, "uqwnzmtb");
        let sibling_bytes = fs::read(&sibling_path).expect("the sibling reads as bytes");

        remove(&root, "k4mz7qta")
            .expect("a target the schema refuses is still the document the handle names");

        assert!(
            !path_for(&root, "k4mz7qta").exists(),
            "the schema-invalid target is removed"
        );
        assert_eq!(
            fs::read(&sibling_path).expect("the sibling still reads as bytes"),
            sibling_bytes,
            "the conforming sibling is left byte-identical"
        );
        assert_eq!(
            read(&root, "uqwnzmtb").expect("the sibling reads"),
            sibling,
            "the conforming sibling is still a record"
        );
    }

    #[test]
    fn a_non_missing_filesystem_failure_is_io_not_missing() {
        let root = scratch("remove-io");
        // A directory sits at the path the handle names, so `fs::remove_file` fails with an
        // is-a-directory-style error: not `NotFound`. Only absence is `Missing`; every other
        // filesystem failure is `Io`. Mapping every io error to `Missing` would report a false
        // absence for a document that is still there.
        let target = path_for(&root, "k4mz7qta");
        fs::create_dir(&target).expect("the blocking directory is created");

        let sibling = record("uqwnzmtb");
        put_raw(&root, "uqwnzmtb", &sibling);
        let sibling_path = path_for(&root, "uqwnzmtb");
        let sibling_bytes = fs::read(&sibling_path).expect("the sibling reads as bytes");

        let error = remove(&root, "k4mz7qta")
            .expect_err("a target that cannot be removed as a file is refused");

        assert!(
            matches!(&error, Error::Io(_)),
            "a non-NotFound filesystem failure is Io, not Missing: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "only absence is Missing: {error:?}"
        );
        assert!(
            format!("{error}").contains(&target.display().to_string()),
            "the Io refusal names the path it failed at: {error}"
        );
        assert!(
            target.is_dir(),
            "the failure removes nothing at the target path"
        );
        assert_eq!(
            fs::read(&sibling_path).expect("the sibling still reads as bytes"),
            sibling_bytes,
            "the failure leaves every other document exactly where it was"
        );
        assert_eq!(
            read(&root, "uqwnzmtb").expect("the sibling reads"),
            sibling,
            "the failure leaves every other record readable"
        );
    }

    /// A removal failure that is neither absence nor a directory is still `Io`. A registry
    /// root the process cannot write makes unlinking the document fail with permission-denied
    /// on Unix, a kind the gate's other non-`NotFound` removal fixture (a directory at the
    /// target, EISDIR) does not exercise. An implementation that special-cases only
    /// is-a-directory as `Io` and maps every other non-`NotFound` kind to `Missing` reports a
    /// false absence for a document that is still there.
    #[cfg(unix)]
    #[test]
    fn a_remove_failure_of_any_other_kind_is_io_not_missing() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("remove-permission");
        put_raw(&root, "k4mz7qta", &record("k4mz7qta"));
        let target = path_for(&root, "k4mz7qta");
        assert!(target.is_file(), "the record is there before the removal");

        let original = fs::metadata(&root)
            .expect("the registry root's metadata reads")
            .permissions();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555))
            .expect("the registry root is made read-only");

        let outcome = remove(&root, "k4mz7qta");

        // Restore the root before inspecting the outcome, so a failed assertion still leaves
        // a directory the test harness can clean up.
        fs::set_permissions(&root, original)
            .expect("the registry root's permissions are restored");

        let error = match outcome {
            Ok(()) => {
                // A process that may unlink from a read-only directory (for example root)
                // cannot provoke permission-denied; the platform, not the module, makes the
                // check inapplicable, so the case is skipped rather than weakened.
                return;
            }
            Err(error) => error,
        };

        assert!(
            matches!(&error, Error::Io(_)),
            "a non-NotFound removal failure is Io whatever its kind, not Missing: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "only absence is Missing: {error:?}"
        );
        assert!(
            format!("{error}").contains(&target.display().to_string()),
            "the Io refusal names the path it failed at: {error}"
        );
        assert!(
            target.is_file(),
            "the failed removal leaves the document where it was"
        );
    }

    /// A removal must not ask the path whether it exists before unlinking it. `Path::exists`
    /// (and `is_file`, and `metadata`) swallows the stat failure — `EACCES` on a directory the
    /// process cannot search — and reports `false`, so a pre-check turns a document that is
    /// still there into a false absence and answers `Missing` where `fs::remove_file` would
    /// fail with permission-denied. The document is placed first, then the registry root is
    /// made unsearchable, so the path cannot be examined: the only way to learn the document
    /// is there is to call `remove_file` and read its error. A path that cannot be examined is
    /// `Io`, not absence.
    #[cfg(unix)]
    #[test]
    fn a_path_that_cannot_be_examined_is_io_not_a_false_absence() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("remove-unsearchable");
        let handle = "k4mz7qta";
        put_raw(&root, handle, &record(handle));
        let target = path_for(&root, handle);
        assert!(target.is_file(), "the record is there before the removal");

        fs::set_permissions(&root, fs::Permissions::from_mode(0o000))
            .expect("the registry root is made unsearchable");

        let outcome = remove(&root, handle);

        // Restore the root before inspecting the outcome, so the document can be examined and
        // a failed assertion still leaves a directory the test harness can clean up.
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755))
            .expect("the registry root's permissions are restored");

        match outcome {
            Ok(()) => {
                // A process that can unlink through an unsearchable directory (for example
                // root) cannot provoke permission-denied; the platform, not the module, makes
                // the check inapplicable, so the case is skipped — it is never weakened to
                // accept `Missing`.
                assert!(
                    !target.exists(),
                    "the removal that succeeded took exactly the document the handle names"
                );
            }
            Err(error) => {
                assert!(
                    !matches!(&error, Error::Missing(_)),
                    "a path that cannot be examined is a document still there, not an absent \
                     record: {error:?}"
                );
                assert!(
                    matches!(&error, Error::Io(_)),
                    "a target that exists but cannot be examined is Io, not a false absence: \
                     {error:?}"
                );
                assert!(
                    format!("{error}").contains(&target.display().to_string()),
                    "the Io refusal names the path it failed at: {error}"
                );
                // The failed removal left the document where it was: once the directory can be
                // searched again the record is still there, and removal then succeeds.
                assert!(
                    target.is_file(),
                    "the failed removal leaves the document where it was"
                );
                assert_eq!(
                    read(&root, handle)
                        .expect("the record reads once the registry root is searchable"),
                    record(handle),
                    "the failed removal left the record itself untouched"
                );
                remove(&root, handle)
                    .expect("the removal succeeds once the registry root can be searched");
                assert!(
                    !target.exists(),
                    "the removal takes the document when the path can be examined"
                );
            }
        }
    }

    /// A removal failure that is neither absence nor permission-denied is still `Io`. The
    /// gate's other non-`NotFound` removal fixtures can all fail with permission-denied on a
    /// platform where unlinking a directory and unlinking through a read-only or unsearchable
    /// root are denied (a directory at the target is EPERM on macOS, EISDIR on Linux; a
    /// read-only or unsearchable root is EACCES). A registry root that is a regular file makes
    /// `remove_file(<root>/<handle>.json)` fail with not-a-directory (ENOTDIR), a different
    /// kind, and an implementation that maps only permission-denied to `Io` and every other
    /// non-`NotFound` kind to `Missing` reports the document path as absent when the root it
    /// would live under is not a directory at all.
    #[cfg(unix)]
    #[test]
    fn a_remove_failure_because_the_root_is_a_regular_file_is_io_not_missing() {
        let parent = scratch("remove-root-file");
        let root = parent.join("work");
        let root_bytes = b"the registry root is a regular file";
        fs::write(&root, root_bytes).expect("the registry root is made a regular file");
        let handle = "k4mz7qta";
        let target = path_for(&root, handle);

        let error = remove(&root, handle).expect_err(
            "a document path under a root that is not a directory cannot be unlinked",
        );

        assert!(
            matches!(&error, Error::Io(_)),
            "a non-NotFound removal failure is Io whatever its kind, not Missing: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "the root exists as a file, so its document path is not an absent record: {error:?}"
        );
        assert!(
            format!("{error}").contains(&target.display().to_string()),
            "the Io refusal names the path it failed at: {error}"
        );
        assert!(
            root.is_file(),
            "the failed removal leaves the root file exactly where it was"
        );
        assert_eq!(
            fs::read(&root).expect("the root file still reads as bytes"),
            root_bytes,
            "the failed removal does not change the root file"
        );
    }

    #[test]
    fn remove_leaves_every_other_record_at_its_handle() {
        let root = scratch("remove-sibling");
        write(&root, &record("k4mz7qta")).expect("the record to remove is written");
        let mut landed = record("uqwnzmtb");
        landed["state"] = json!("landed");
        write(&root, &landed).expect("the landed sibling is written");
        let hand_placed = record("vbnmklpo");
        put_raw(&root, "vbnmklpo", &hand_placed);

        let landed_path = path_for(&root, "uqwnzmtb");
        let landed_bytes = fs::read(&landed_path).expect("the landed sibling reads as bytes");
        let hand_path = path_for(&root, "vbnmklpo");
        let hand_bytes = fs::read(&hand_path).expect("the hand-placed sibling reads as bytes");

        remove(&root, "k4mz7qta").expect("the open record is removed");

        assert!(
            !path_for(&root, "k4mz7qta").exists(),
            "the document the handle names is gone"
        );
        assert_eq!(
            fs::read(&landed_path).expect("the landed sibling still reads as bytes"),
            landed_bytes,
            "the landed record is left exactly where it was and is never removed"
        );
        assert_eq!(
            fs::read(&hand_path).expect("the hand-placed sibling still reads as bytes"),
            hand_bytes,
            "every other document is left where it was"
        );
        assert_eq!(
            read(&root, "uqwnzmtb").expect("the landed sibling reads"),
            landed,
            "the landed record stays at its handle"
        );
        assert_eq!(
            read(&root, "vbnmklpo").expect("the hand-placed sibling reads"),
            hand_placed,
            "a record placed by hand stays at its handle"
        );

        let found: BTreeMap<String, Value> = scan(&root)
            .expect("the registry scans after the removal")
            .into_iter()
            .collect();
        let mut expected = BTreeMap::new();
        expected.insert("uqwnzmtb".to_string(), landed);
        expected.insert("vbnmklpo".to_string(), hand_placed);
        assert_eq!(
            found, expected,
            "a scan after the removal finds every remaining record and not the removed one"
        );
    }

    #[test]
    fn removing_an_absent_handle_is_a_named_refusal() {
        let root = scratch("remove-absent");
        write(&root, &record("uqwnzmtb")).expect("a remaining record is written");
        let absent = "k4mz7qta";
        let path = path_for(&root, absent);

        let error = remove(&root, absent).expect_err("an absent record is refused");

        match &error {
            Error::Missing(reported) => assert_eq!(
                reported.as_path(),
                path.as_path(),
                "the refusal carries the path the absent handle names"
            ),
            other => panic!("expected Missing, got {other:?}"),
        }
        assert!(
            format!("{error}").contains(&path.display().to_string()),
            "the refusal message names the path: {error}"
        );
        assert!(!path.exists(), "the refusal removes nothing at the absent path");
        assert_eq!(
            read(&root, "uqwnzmtb").expect("the remaining record still reads"),
            record("uqwnzmtb"),
            "the refusal leaves every other record at its handle"
        );
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

    /// Only a directory that is not there is an empty registry. When the path handed to
    /// `scan` is a regular file, the directory exists but cannot be listed: `fs::read_dir`
    /// fails with a non-`NotFound` error (not-a-directory), so the listing must be `Io`
    /// naming the root and never `Ok(Vec::new())`. A scan that answers an empty list for
    /// every `read_dir` error turns an existing-but-unlistable registry into a false empty
    /// registry, indistinguishable from one that holds no records.
    #[test]
    fn scan_of_a_root_that_is_a_regular_file_is_io_not_empty() {
        let root = scratch("scan-file-root");
        let not_a_directory = root.join("not-a-directory");
        fs::write(&not_a_directory, b"not a directory").expect("the fixture file is written");

        let error = scan(&not_a_directory)
            .expect_err("a root that is not a directory cannot be listed as a registry");

        assert!(
            matches!(&error, Error::Io(_)),
            "a root that exists but cannot be listed is Io, not an empty registry: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "the root exists; absence is not a listing failure: {error:?}"
        );
        assert!(
            format!("{error}").contains(&not_a_directory.display().to_string()),
            "the Io refusal names the path it failed at: {error}"
        );
    }

    /// An existing registry root the process cannot search is not an absent registry. With
    /// the root's mode `0o000`, reading the directory fails with permission-denied, so `scan`
    /// must answer `Io` naming the root rather than report the registry as empty. A record is
    /// placed first, so a successful listing — only possible where the platform lets the
    /// process read through the mode, for example as root — must find it: the skip path never
    /// accepts `Ok(Vec::new())` as the answer for a registry that is there.
    #[cfg(unix)]
    #[test]
    fn scan_of_an_unsearchable_root_is_io_not_empty() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("scan-unsearchable");
        put_raw(&root, "k4mz7qta", &record("k4mz7qta"));
        let original = fs::metadata(&root)
            .expect("the registry root's metadata reads")
            .permissions();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000))
            .expect("the registry root is made unsearchable");

        let outcome = scan(&root);

        // Restore the root before inspecting the outcome, so a failed assertion still leaves
        // a directory the test harness can clean up.
        fs::set_permissions(&root, original)
            .expect("the registry root's permissions are restored");

        let error = match outcome {
            Ok(records) => {
                // A process that can list through an unsearchable directory (for example
                // root) cannot provoke permission-denied; the platform, not the module, makes
                // the check inapplicable. It is skipped only when the listing actually
                // succeeded; an empty listing for a registry that is there is never accepted.
                assert!(
                    !records.is_empty(),
                    "an existing registry is never a false empty list: if the process can \
                     list through the mode, the record placed under the root is found"
                );
                return;
            }
            Err(error) => error,
        };

        assert!(
            matches!(&error, Error::Io(_)),
            "a root that exists but cannot be listed is Io, not an empty registry: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "the root exists; absence is not a listing failure: {error:?}"
        );
        assert!(
            format!("{error}").contains(&root.display().to_string()),
            "the Io refusal names the path it failed at: {error}"
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
    fn a_record_path_that_cannot_be_read_is_io_not_missing() {
        let root = scratch("read-io");
        // A directory sits where the record's document would be. `fs::read` refuses it with
        // an is-a-directory-style error, not `NotFound`, so the record is not absent: it is
        // there and unreadable. Only absence is `Missing`; every other read failure is `Io`
        // naming the path. Mapping every io error to `Missing` reports a false absence.
        let target = path_for(&root, "k4mz7qta");
        fs::create_dir(&target).expect("the blocking directory is created");

        let error =
            read(&root, "k4mz7qta").expect_err("a path that cannot be read as a document is refused");

        assert!(
            matches!(&error, Error::Io(_)),
            "a non-NotFound read failure is Io, not Missing: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "only absence is Missing: {error:?}"
        );
        assert!(
            format!("{error}").contains(&target.display().to_string()),
            "the Io refusal names the path it failed at: {error}"
        );
        assert!(
            target.is_dir(),
            "the unreadable record path is left as it was"
        );
    }

    /// A read failure that is neither absence nor a directory is still `Io`. A symlink loop at
    /// the record's path makes `fs::read` fail with ELOOP on Unix, a kind the gate's other
    /// non-`NotFound` read fixture (a directory, EISDIR) does not exercise. An implementation
    /// that special-cases only is-a-directory as `Io` and maps every other non-`NotFound` kind
    /// to `Missing` reports a false absence for a record path that is there and unreadable.
    #[cfg(unix)]
    #[test]
    fn a_read_failure_of_any_other_kind_is_io_not_missing() {
        use std::os::unix::fs::symlink;

        let root = scratch("read-eloop");
        let target = path_for(&root, "k4mz7qta");
        // The link names itself relative to its own directory, so resolving it never
        // terminates: ELOOP, not ENOENT and not EISDIR.
        symlink("k4mz7qta.json", &target).expect("the self-referential symlink is created");

        let error = read(&root, "k4mz7qta")
            .expect_err("a path that cannot be resolved as a document is refused");

        assert!(
            matches!(&error, Error::Io(_)),
            "a non-NotFound read failure is Io whatever its kind, not Missing: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "only absence is Missing: {error:?}"
        );
        assert!(
            format!("{error}").contains(&target.display().to_string()),
            "the Io refusal names the path it failed at: {error}"
        );
    }

    /// A document is JSON or it is not: bytes that are not UTF-8 cannot be a JSON text
    /// document, so they are `NotJson`, not `Io`. An implementation that reads the file as
    /// text (`fs::read_to_string`) fails before parsing and reports `Io`, hiding a malformed
    /// record as an unreadable path.
    #[test]
    fn a_non_utf8_document_is_not_json_not_io() {
        let root = scratch("read-non-utf8");
        let target = path_for(&root, "k4mz7qta");
        // `\xff` is not valid UTF-8, so these bytes are not a JSON text document at all.
        fs::write(&target, b"{\"handle\": \"k4mz7qta\", \"state\": \"\xff\"}")
            .expect("the non-UTF-8 document is written");

        let error = read(&root, "k4mz7qta")
            .expect_err("a document that is not UTF-8 is not a record");

        assert!(
            matches!(&error, Error::NotJson(_, _)),
            "a document that is not JSON is NotJson, including bytes that are not UTF-8: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Io(_)),
            "non-UTF-8 bytes are a parse refusal, not an io failure: {error:?}"
        );
        assert!(
            !matches!(&error, Error::Missing(_)),
            "the document is there, so it is not Missing: {error:?}"
        );
        assert!(
            format!("{error}").contains(&target.display().to_string()),
            "the NotJson refusal names the path: {error}"
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
