//! Gate for M-journal-apply (the buffer apply).
//!
//! Self-contained per M-tests: the shims pull in the module under test and its siblings,
//! mirroring the crate root. The apply is driven over a scratch jj workspace: a target change
//! holds the after side, a child working copy holds the after side too (the shape a recording
//! leaves), and an unrelated dirty path sits in between. It reaches the journal store, so it
//! needs serde, serde_json and the schema validator.

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/journal.rs"]
mod journal;
#[path = "../../../src/journal_apply.rs"]
mod journal_apply;

#[cfg(test)]
mod gate {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::jj::Jj;
    use crate::journal::{self, Entry};
    use crate::journal_apply::{apply, Apply, ApplyError};
    use crate::store::Store;

    type Snapshot = BTreeMap<String, Option<Vec<u8>>>;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-journal-apply-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

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

    fn change_id(dir: &Path) -> String {
        jj(dir, &["log", "-r", "@", "--no-graph", "-T", "change_id"])
    }

    /// The paths the working copy changes relative to its parent.
    fn working_copy_changes(dir: &Path) -> Vec<String> {
        jj(dir, &["diff", "--name-only"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// A target change holding `f.txt` with the given content, a child working copy that still
    /// holds it (the after side a recording leaves), and an unrelated dirty path. Returns the
    /// target id.
    fn repo_with(name: &str, target_bytes: &[u8]) -> (PathBuf, String) {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".gitignore"), "nodes/\n.deccy/\n").unwrap();
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        jj(&dir, &["describe", "-m", "target"]);
        let target = change_id(&dir);
        fs::write(dir.join("f.txt"), target_bytes).unwrap();
        jj(&dir, &["status"]);
        jj(&dir, &["new", "-m", "child"]);
        fs::write(dir.join("dirty.txt"), "unrelated\n").unwrap();
        jj(&dir, &["status"]);
        (dir, target)
    }

    fn repo(name: &str) -> (PathBuf, String) {
        repo_with(name, b"after\n")
    }

    /// The post-record shape after a change-level undo has left an empty working copy: a
    /// target change holding `f.txt` at the after side, a child change holding an unrelated
    /// dirty path, and an empty working copy on top of it. The working copy is undescribed
    /// because jj abandons an empty, undescribed working-copy change when it moves the
    /// checkout — the case the apply must survive by opening a fresh empty change on the old
    /// parent. Returns the target and the working copy's pre-apply parent.
    fn repo_with_empty_working_copy(
        name: &str,
        target_bytes: &[u8],
    ) -> (PathBuf, String, String) {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".gitignore"), "nodes/\n.deccy/\n").unwrap();
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        jj(&dir, &["describe", "-m", "target"]);
        let target = change_id(&dir);
        fs::write(dir.join("f.txt"), target_bytes).unwrap();
        jj(&dir, &["status"]);
        jj(&dir, &["new", "-m", "child"]);
        fs::write(dir.join("dirty.txt"), "unrelated\n").unwrap();
        jj(&dir, &["status"]);
        let parent = change_id(&dir);
        jj(&dir, &["new"]);
        (dir, target, parent)
    }

    /// The working copy's parent change id, the change a restore must leave the working copy
    /// on.
    fn parent_id(dir: &Path) -> String {
        jj(dir, &["log", "-r", "@-", "--no-graph", "-T", "change_id"])
    }

    /// The number of visible heads, so a restore that strands a second empty change shows.
    fn head_count(dir: &Path) -> usize {
        jj(dir, &["log", "-r", "heads(all())", "--no-graph", "-T", "change_id"])
            .lines()
            .count()
    }

    fn entry(store: &Store, target: &str, before: Option<&[u8]>, after: Option<&[u8]>) -> Entry {
        let mut b = Snapshot::new();
        b.insert("f.txt".to_string(), before.map(|bytes| bytes.to_vec()));
        let mut a = Snapshot::new();
        a.insert("f.txt".to_string(), after.map(|bytes| bytes.to_vec()));
        let id = journal::put(store, "record", target, &b, &a).unwrap();
        journal::get(store, &id).unwrap()
    }

    fn target_content(dir: &Path, target: &str) -> Option<String> {
        let out = Command::new("jj")
            .args(["file", "show", "-r", target, "f.txt"])
            .current_dir(dir)
            .output()
            .expect("jj is on PATH for the gate");
        out.status
            .success()
            .then(|| String::from_utf8(out.stdout).unwrap())
    }

    #[test]
    fn undo_applies_before_to_the_target_and_returns_after_to_the_working_copy() {
        let (dir, target) = repo("undo");
        let store = Store::new(&dir);
        let old = change_id(&dir);
        let entry = entry(&store, &target, None, Some(b"after\n"));
        apply(&store, &Jj::new("jj", &dir), &entry, Apply::Undo).unwrap();
        assert_eq!(target_content(&dir, &target), None, "the target holds before");
        assert_eq!(
            fs::read_to_string(dir.join("f.txt")).unwrap(),
            "after\n",
            "the working copy keeps the after side the recording left"
        );
        assert!(
            working_copy_changes(&dir).contains(&"f.txt".to_string()),
            "the recorded diff appears against the target's before side"
        );
        assert_eq!(
            fs::read_to_string(dir.join("dirty.txt")).unwrap(),
            "unrelated\n",
            "an unrelated dirty path is untouched"
        );
        assert_eq!(change_id(&dir), old, "the user's working copy is restored");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_applies_after_to_the_target_and_leaves_the_after_side_in_the_working_copy() {
        let (dir, target) = repo("redo");
        let store = Store::new(&dir);
        let entry = entry(&store, &target, None, Some(b"after\n"));
        let jjrun = Jj::new("jj", &dir);
        apply(&store, &jjrun, &entry, Apply::Undo).unwrap();
        apply(&store, &jjrun, &entry, Apply::Redo).unwrap();
        assert_eq!(
            target_content(&dir, &target).as_deref(),
            Some("after\n"),
            "the target holds after again"
        );
        assert_eq!(
            fs::read_to_string(dir.join("f.txt")).unwrap(),
            "after\n",
            "the working copy is left at the after side"
        );
        assert!(
            !working_copy_changes(&dir).contains(&"f.txt".to_string()),
            "the recorded diff is gone from the working copy"
        );
        assert_eq!(
            fs::read_to_string(dir.join("dirty.txt")).unwrap(),
            "unrelated\n",
            "the unrelated dirty path still stands"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_target_mismatch_refuses_naming_the_path_and_moves_nothing() {
        let (dir, target) = repo("target-mismatch");
        let store = Store::new(&dir);
        let old = change_id(&dir);
        // The target does not hold "after"; it holds "other".
        fs::write(dir.join("other.txt"), "x\n").unwrap();
        let entry = entry(&store, &target, None, Some(b"other\n"));
        match apply(&store, &Jj::new("jj", &dir), &entry, Apply::Undo) {
            Err(ApplyError::Mismatch(paths)) => {
                assert!(paths.contains(&"f.txt".to_string()), "the path is named")
            }
            other => panic!("a target mismatch must refuse, got {other:?}"),
        }
        assert_eq!(change_id(&dir), old, "nothing moved");
        assert_eq!(
            fs::read_to_string(dir.join("dirty.txt")).unwrap(),
            "unrelated\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_working_copy_mismatch_refuses_naming_the_path() {
        let (dir, target) = repo("working-mismatch");
        let store = Store::new(&dir);
        let old = change_id(&dir);
        fs::write(dir.join("f.txt"), "not the after side\n").unwrap();
        let entry = entry(&store, &target, None, Some(b"after\n"));
        match apply(&store, &Jj::new("jj", &dir), &entry, Apply::Undo) {
            Err(ApplyError::Mismatch(paths)) => {
                assert!(paths.contains(&"f.txt".to_string()), "the path is named")
            }
            other => panic!("a working-copy mismatch must refuse, got {other:?}"),
        }
        assert_eq!(change_id(&dir), old, "nothing moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn binary_content_survives_the_apply() {
        let bytes: Vec<u8> = vec![0, 159, 146, 150, 255];
        let (dir, target) = repo_with("binary", &bytes);
        let store = Store::new(&dir);
        let entry = entry(&store, &target, None, Some(&bytes));
        apply(&store, &Jj::new("jj", &dir), &entry, Apply::Undo).unwrap();
        assert_eq!(fs::read(dir.join("f.txt")).unwrap(), bytes);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undo_restores_an_empty_working_copy_on_its_old_parent() {
        let (dir, target, parent) = repo_with_empty_working_copy("undo-empty", b"after\n");
        let store = Store::new(&dir);
        let entry = entry(&store, &target, None, Some(b"after\n"));
        let heads = head_count(&dir);
        apply(&store, &Jj::new("jj", &dir), &entry, Apply::Undo).unwrap();
        assert_eq!(target_content(&dir, &target), None, "the target holds before");
        assert_eq!(
            fs::read_to_string(dir.join("f.txt")).unwrap(),
            "after\n",
            "the working copy keeps the after side the recording left"
        );
        assert!(
            jj(&dir, &["diff", "--from", &target, "--name-only"]).contains("f.txt"),
            "the recorded diff appears against the target's before side"
        );
        assert_eq!(
            fs::read_to_string(dir.join("dirty.txt")).unwrap(),
            "unrelated\n",
            "an unrelated dirty path is untouched"
        );
        assert_eq!(
            parent_id(&dir),
            parent,
            "the recreated working copy sits on the old parent"
        );
        assert_eq!(
            head_count(&dir),
            heads,
            "the recreated empty working copy does not strand a second empty head"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redo_restores_an_empty_working_copy_on_its_old_parent() {
        let (dir, target, parent) = repo_with_empty_working_copy("redo-empty", b"after\n");
        let store = Store::new(&dir);
        let entry = entry(&store, &target, None, Some(b"after\n"));
        let jjrun = Jj::new("jj", &dir);
        apply(&store, &jjrun, &entry, Apply::Undo).unwrap();
        let heads = head_count(&dir);
        apply(&store, &jjrun, &entry, Apply::Redo).unwrap();
        assert_eq!(
            target_content(&dir, &target).as_deref(),
            Some("after\n"),
            "the target holds after again"
        );
        assert_eq!(
            fs::read_to_string(dir.join("f.txt")).unwrap(),
            "after\n",
            "the working copy is left at the after side"
        );
        assert!(
            !jj(&dir, &["diff", "--from", &target, "--name-only"]).contains("f.txt"),
            "the recorded diff is gone from the working copy"
        );
        assert_eq!(
            fs::read_to_string(dir.join("dirty.txt")).unwrap(),
            "unrelated\n",
            "the unrelated dirty path still stands"
        );
        assert_eq!(
            parent_id(&dir),
            parent,
            "the recreated working copy sits on the old parent"
        );
        assert_eq!(
            head_count(&dir),
            heads,
            "the recreated empty working copy does not strand a second empty head"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
