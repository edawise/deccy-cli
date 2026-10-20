#[cfg(test)]
mod gate {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::journal::{self, JournalError};
    use crate::store::Store;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-journal-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn content(pairs: &[(&str, Option<&[u8]>)]) -> BTreeMap<String, Option<Vec<u8>>> {
        pairs
            .iter()
            .map(|(path, bytes)| (path.to_string(), bytes.map(|b| b.to_vec())))
            .collect()
    }

    fn objects(dir: &Path) -> Vec<String> {
        fs::read_dir(dir.join(".deccy/objects"))
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_put_get_round_trip_keeps_both_sides() {
        let dir = scratch("round-trip");
        let store = Store::new(&dir);
        let before = content(&[
            ("nodes/kqxyzmrn.json", Some(b"old node")),
            ("notes.txt", None),
        ]);
        let after = content(&[
            ("nodes/kqxyzmrn.json", Some(b"new node")),
            ("notes.txt", Some(b"the edit")),
        ]);
        let id = journal::put(&store, "record", "kqxyzmrn", &before, &after).unwrap();
        assert_eq!(id.len(), 64, "the id is a SHA-256");
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));

        let entry = journal::get(&store, &id).unwrap();
        assert_eq!(entry.target, "kqxyzmrn");
        assert_eq!(entry.kind, "record");
        assert_eq!(entry.before["notes.txt"], None, "absent is null");
        let node_hash = entry.before["nodes/kqxyzmrn.json"]
            .as_deref()
            .expect("the node file has content");
        assert_eq!(journal::content(&store, node_hash).unwrap(), b"old node");
        let edit_hash = entry.after["notes.txt"].as_deref().unwrap();
        assert_eq!(journal::content(&store, edit_hash).unwrap(), b"the edit");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn identical_content_dedupes_to_one_blob() {
        let dir = scratch("dedupe");
        let store = Store::new(&dir);
        let side = content(&[("a.txt", Some(b"same"))]);
        journal::put(&store, "record", "kqxyzmrn", &side, &side).unwrap();
        journal::put(&store, "record", "rnkqxyzm", &side, &side).unwrap();
        assert_eq!(
            objects(&dir).len(),
            1,
            "one blob holds the identical content"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn binary_content_survives_the_round_trip() {
        let dir = scratch("binary");
        let store = Store::new(&dir);
        let bytes: Vec<u8> = vec![0, 159, 146, 150, 255, 10, 13];
        let side = content(&[("blob.bin", Some(&bytes))]);
        let id = journal::put(&store, "record", "kqxyzmrn", &side, &side).unwrap();
        let entry = journal::get(&store, &id).unwrap();
        let hash = entry.before["blob.bin"].as_deref().unwrap();
        assert_eq!(journal::content(&store, hash).unwrap(), bytes);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_entry_refuses_naming_it() {
        let dir = scratch("corrupt");
        let store = Store::new(&dir);
        let id = "a".repeat(64);
        fs::create_dir_all(dir.join(".deccy/journal")).unwrap();
        fs::write(dir.join(format!(".deccy/journal/{id}.json")), "{not json").unwrap();
        match journal::get(&store, &id) {
            Err(JournalError::NotJson(reason)) => assert!(
                reason.contains(&id) || !reason.is_empty(),
                "the corruption is named, got {reason}"
            ),
            other => panic!("a corrupt entry must refuse, got {other:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_entry_whose_name_does_not_match_its_content_refuses() {
        let dir = scratch("mismatch");
        let store = Store::new(&dir);
        let side = content(&[("a.txt", Some(b"content"))]);
        let id = journal::put(&store, "record", "kqxyzmrn", &side, &side).unwrap();
        let text = fs::read_to_string(dir.join(format!(".deccy/journal/{id}.json"))).unwrap();
        let other = "b".repeat(64);
        fs::write(dir.join(format!(".deccy/journal/{other}.json")), text).unwrap();
        match journal::get(&store, &other) {
            Err(JournalError::Invalid(reason)) => assert!(!reason.is_empty()),
            other => panic!("a mismatched id must refuse, got {other:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn gc_removes_unreferenced_entries_and_blobs() {
        let dir = scratch("gc");
        let store = Store::new(&dir);
        let kept = journal::put(
            &store,
            "record",
            "kqxyzmrn",
            &content(&[("kept.txt", Some(b"kept"))]),
            &content(&[("kept.txt", Some(b"kept"))]),
        )
        .unwrap();
        journal::put(
            &store,
            "record",
            "rnkqxyzm",
            &content(&[("gone.txt", Some(b"gone"))]),
            &content(&[("gone.txt", Some(b"gone"))]),
        )
        .unwrap();
        journal::gc(&store, &BTreeSet::from([kept.clone()])).unwrap();
        assert!(dir.join(format!(".deccy/journal/{kept}.json")).exists());
        assert_eq!(objects(&dir).len(), 1, "the unreferenced blob is gone");
        let entries = fs::read_dir(dir.join(".deccy/journal")).unwrap().count();
        assert_eq!(entries, 1, "the unreferenced entry is gone");
        fs::remove_dir_all(&dir).unwrap();
    }
}
