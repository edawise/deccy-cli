//! Gate for M-V-record (the record verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj workspace whose
//! node files are named by real change ids — it reaches node_load and the
//! marker file, so this is a crate of its own, needing serde, serde_json and the schema
//! validator. The compile-time `RECORD_ROW` is the tie to M-route (a Verb row for the
//! handler).

#[path = "../../../src/draft_registry.rs"]
mod draft_registry;
#[path = "../../../src/draft_identity.rs"]
mod draft_identity;
#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/jjread.rs"]
mod jjread;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/store_read.rs"]
mod store_read;
#[path = "../../../src/node_load.rs"]
mod node_load;
#[path = "../../../src/node_resolve.rs"]
mod node_resolve;
#[path = "../../../src/node_own.rs"]
mod node_own;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/status.rs"]
mod status;
#[path = "../../../src/jj_place.rs"]
mod jj_place;
#[path = "../../../src/state_root.rs"]
mod state_root;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/markers.rs"]
mod markers;
#[path = "../../../src/markers_file.rs"]
mod markers_file;
#[path = "../../../src/markers_persist.rs"]
mod markers_persist;
#[path = "../../../src/markers_append.rs"]
mod markers_append;
#[path = "../../../src/oplog.rs"]
mod oplog;
#[path = "../../../src/journal.rs"]
mod journal;
#[path = "../../../src/markers_capture.rs"]
mod markers_capture;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_record.rs"]
mod verb_record;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a Verb row for `record` (M-route, ynvwvzrm) compiles.
const RECORD_ROW: Verb<Context> = Verb {
    name: "record",
    run: crate::verb_record::run,
};

#[cfg(test)]
mod gate {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::{json, Value};

    use crate::context::resolve;
    use crate::draft_identity::{base as identity_base, identity};
    use crate::draft_registry::{read as read_record, write as write_record};
    use crate::journal;
    use crate::markers::Kind;
    use crate::markers_persist;
    use crate::refusal::Refusal;
    use crate::store::Store;
    use crate::verb_record::run;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-record-{}-{}",
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

    /// A scratch jj workspace with a tracked `nodes/` (role="a real workspace"); the schemas are
    /// carried in the binary (`include_str!`), never read from the repo. The node files are
    /// tracked because the read reaches a document at the change that owns it; only the state
    /// directory stays out of jj's tracking.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".gitignore"), ".deccy/\n").unwrap();
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        dir
    }

    /// A real target change whose node file is a valid document listing `refs` as owned;
    /// the working copy is left on a fresh change above it. Returns the target's id.
    fn target(dir: &Path, refs: &[&str]) -> String {
        jj(dir, &["describe", "-m", "target"]);
        let id = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let refs_json = serde_json::to_string(refs).unwrap();
        let document = format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"rust","ref":{refs_json},"rationale":"y"}}}}"#
        );
        fs::write(dir.join(format!("nodes/{id}.json")), document).unwrap();
        jj(dir, &["new"]);
        id
    }

    /// Write `contents` to `path` in the working copy (jj snapshots it).
    fn edit(dir: &Path, path: &str, contents: &str) {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, contents).unwrap();
    }

    /// `deccy record <args>` through the real handler over the scratch workspace.
    fn record(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repo resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        run(&context, &owned)
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir.join("nodes"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn op_head(dir: &Path) -> String {
        jj(dir, &["op", "log", "--limit", "1", "--no-graph", "-T", "id"])
    }

    fn working_copy(dir: &Path) -> String {
        jj(dir, &["diff", "-r", "@", "--summary"])
    }

    // --- the source form's landing: `record --from <handle> --to <node> [path...]` ---

    /// The registry directory the record documents live in (twpvqstu: `.deccy/work/`).
    fn registry(dir: &Path) -> PathBuf {
        dir.join(".deccy/work")
    }

    /// A scratch repository whose untracked registry exists, so a record can be written.
    fn landing_repo(name: &str) -> PathBuf {
        let dir = repo(name);
        fs::create_dir_all(registry(&dir)).unwrap();
        dir
    }

    /// One path's bytes at `rev`, read through jj so a base is exactly the owned content.
    fn file_bytes(dir: &Path, rev: &str, path: &str) -> Vec<u8> {
        let out = Command::new("jj")
            .args(["file", "show", "--ignore-working-copy", "-r", rev, "--", path])
            .current_dir(dir)
            .output()
            .expect("jj is on PATH for the gate");
        assert!(
            out.status.success(),
            "jj file show {rev} {path} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    /// The content-identity base over `paths` at `rev` — the node's owned set as it stands
    /// there (qrqsxyzz), computed through the identity module the opening uses (ttkuyvxp).
    fn owned_base(dir: &Path, rev: &str, paths: &[String]) -> BTreeMap<String, String> {
        let contents: Vec<(String, Vec<u8>)> = paths
            .iter()
            .map(|path| (path.clone(), file_bytes(dir, rev, path)))
            .collect();
        identity_base(contents.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())))
    }

    /// The owned set the ownership rule gives a node (xzxtxymq): its document plus its array
    /// `content.ref` paths, or the document alone.
    fn owned_set(target: &str, refs: &[&str]) -> Vec<String> {
        let mut paths = vec![format!("nodes/{target}.json")];
        paths.extend(refs.iter().map(|path| path.to_string()));
        paths
    }

    /// A real target change whose node document owns `refs`, each holding `contents` at the
    /// target; returns the target's change id and leaves the working copy above it.
    fn target_with(dir: &Path, refs: &[(&str, &str)]) -> String {
        jj(dir, &["describe", "-m", "target"]);
        let id = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let paths: Vec<&str> = refs.iter().map(|(path, _)| *path).collect();
        let refs_json = serde_json::to_string(&paths).unwrap();
        let document = format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"rust","ref":{refs_json},"rationale":"y"}}}}"#
        );
        for (path, contents) in refs {
            edit(dir, path, contents);
        }
        fs::write(dir.join(format!("nodes/{id}.json")), document).unwrap();
        jj(dir, &["new"]);
        id
    }

    /// A draft workspace on top of `parent`, holding the parent's content until edited.
    fn draft_workspace(dir: &Path, name: &str, parent: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "deccy-verb-record-draft-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&path);
        let destination = path.to_string_lossy().into_owned();
        let out = Command::new("jj")
            .args(["workspace", "add", "--name", name, "-r", parent, &destination])
            .current_dir(dir)
            .output()
            .expect("jj is on PATH for the gate");
        assert!(
            out.status.success(),
            "jj workspace add {name} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        path
    }

    /// Forget and remove a draft workspace the test opened.
    fn drop_workspace(dir: &Path, name: &str, path: &Path) {
        let _ = Command::new("jj")
            .args(["workspace", "forget", name])
            .current_dir(dir)
            .output();
        let _ = fs::remove_dir_all(path);
    }

    /// One record document of the shape the registry schema fixes (schemas/work.json).
    fn record_document(
        handle: &str,
        state: &str,
        node: &str,
        edit_id: &str,
        workspace: (&str, &Path),
        base: &BTreeMap<String, String>,
    ) -> Value {
        json!({
            "handle": handle,
            "state": state,
            "node": node,
            "edit": edit_id,
            "workspace": {
                "name": workspace.0,
                "path": workspace.1.to_string_lossy(),
            },
            "base": base,
        })
    }

    /// Write a record through the registry module (rroqqrzq) at `.deccy/work/<handle>.json`.
    fn put_record(dir: &Path, document: &Value) -> String {
        let handle = document["handle"].as_str().unwrap().to_string();
        write_record(&registry(dir), document).expect("the record is written");
        handle
    }

    /// A refusal names `token` in its cause or its evidence.
    fn names(refusal: &Refusal, token: &str) -> bool {
        refusal.cause().contains(token)
            || refusal.evidence().iter().any(|entry| entry.contains(token))
    }

    /// An editor that leaves a marker and fails, returned as the script's path and the
    /// marker's path. `JJ_EDITOR` names it: the landing places the edits without asking anyone
    /// to combine or choose a description, so an editor that is opened at all fails the
    /// crossing and its marker records that it was opened.
    fn failing_editor(dir: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let script = dir.join(".deccy/gate-editor.sh");
        let marker = dir.join(".deccy/editor-launched");
        fs::write(&script, format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display())).unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
        (script, marker)
    }

    /// Point `JJ_EDITOR` at `script` for the enclosing scope, restoring the previous value when
    /// it ends — failed runs included. `JJ_EDITOR` outranks `ui.editor` and `$VISUAL`/`$EDITOR`,
    /// so no configuration on the machine can turn the landing interactive or hang it.
    struct EditorEnv(Option<std::ffi::OsString>);

    impl Drop for EditorEnv {
        fn drop(&mut self) {
            match self.0.take() {
                Some(previous) => std::env::set_var("JJ_EDITOR", previous),
                None => std::env::remove_var("JJ_EDITOR"),
            }
        }
    }

    fn editor_env(script: &Path) -> EditorEnv {
        let previous = std::env::var_os("JJ_EDITOR");
        std::env::set_var("JJ_EDITOR", script);
        EditorEnv(previous)
    }

    /// The description jj carries for the change `rev` — the source of the summaries the log
    /// verb lists (S3-log).
    fn description(dir: &Path, rev: &str) -> String {
        jj(dir, &["log", "-r", rev, "--no-graph", "-T", "description"])
    }

    #[test]
    fn the_named_edit_lands_in_the_target_and_stdout_is_empty() {
        let dir = repo("place");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "the edit\n");
        let lines = record(&dir, &["src/a.rs", "--to", &target]).expect("a placed recording");
        assert!(lines.is_empty(), "O7-record: nothing on stdout");
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            "the edit"
        );
        assert_eq!(crate::RECORD_ROW.name, "record");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unnamed_edit_stays_while_the_named_one_moves() {
        let dir = repo("subset");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        edit(&dir, "src/b.rs", "b\n");
        record(&dir, &["src/a.rs", "--to", &target]).expect("a placed recording");
        let remaining = working_copy(&dir);
        assert!(
            remaining.contains("src/b.rs"),
            "the unnamed edit stays in the working copy, got {remaining:?}"
        );
        assert!(
            !remaining.contains("src/a.rs"),
            "the named edit moved out, got {remaining:?}"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            "a"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_to_refuses_and_nothing_moves() {
        let dir = repo("noto");
        let _ = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        let head = op_head(&dir);
        let before = listing(&dir);
        match record(&dir, &["src/a.rs"]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("a missing --to must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        assert!(working_copy(&dir).contains("src/a.rs"), "the edit stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_path_refuses_and_nothing_moves() {
        let dir = repo("nopath");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        let head = op_head(&dir);
        let before = listing(&dir);
        match record(&dir, &["--to", &target]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("a path-less record must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        assert!(working_copy(&dir).contains("src/a.rs"), "the edit stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_unnameable_argument_refuses_naming_it() {
        let dir = repo("unnamed");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        let head = op_head(&dir);
        let before = listing(&dir);
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["src/a.rs", "--bogus", "--to", &target], "--bogus"),
            (vec!["src/a.rs", "--to"], "--to"),
            (vec!["src/a.rs", "--to", &target, "--to", &target], "--to"),
            (vec!["src/a.rs", "--help", "--to", &target], "--help"),
        ];
        for (args, token) in cases {
            match record(&dir, &args) {
                Err(refusal) => assert!(
                    refusal.evidence().contains(&token.to_string()),
                    "{args:?} must name {token:?}, got {refusal:?}"
                ),
                Ok(lines) => panic!("{args:?} must refuse, got {lines:?}"),
            }
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn paths_interleave_with_to() {
        let dir = repo("interleave");
        let target = target(&dir, &["src/a.rs", "src/b.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        edit(&dir, "src/b.rs", "b\n");
        record(&dir, &["src/a.rs", "--to", &target, "src/b.rs"]).expect("the paths interleave");
        assert_eq!(jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]), "a");
        assert_eq!(jj(&dir, &["file", "show", "-r", &target, "src/b.rs"]), "b");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_rename_is_placed_when_the_ref_covers_both_paths() {
        let dir = repo("rename");
        // Track the old path in an older change, so the move is a rename jj names once.
        edit(&dir, "src/a.rs", "the edit\n");
        jj(&dir, &["new", "-m", "base"]);
        let target = target(&dir, &["src/a.rs", "src/b.rs"]);
        fs::rename(dir.join("src/a.rs"), dir.join("src/b.rs")).unwrap();
        record(&dir, &["src/a.rs", "src/b.rs", "--to", &target]).expect("a placed rename");
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/b.rs"]),
            "the edit"
        );
        let files = jj(&dir, &["file", "list", "-r", &target]);
        assert!(
            !files.lines().any(|line| line == "src/a.rs"),
            "the old path moved out of the target, got {files:?}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unknown_id_refuses_naming_it_with_nothing_moved() {
        let dir = repo("unknown");
        let _ = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        let head = op_head(&dir);
        let before = listing(&dir);
        match record(&dir, &["src/a.rs", "--to", "abcdefgh"]) {
            Err(refusal) => assert!(refusal.evidence().contains(&"abcdefgh".to_string())),
            Ok(lines) => panic!("an unknown id must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        assert!(working_copy(&dir).contains("src/a.rs"), "the edit stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_target_that_cannot_be_read_refuses_before_placing() {
        let dir = repo("unreadable");
        jj(&dir, &["describe", "-m", "broken target"]);
        let target = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        fs::write(dir.join(format!("nodes/{target}.json")), "{ not json").unwrap();
        jj(&dir, &["new"]);
        edit(&dir, "src/a.rs", "a\n");
        let head = op_head(&dir);
        let before = listing(&dir);
        match record(&dir, &["src/a.rs", "--to", &target]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&target),
                "the target that could not be read is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("an unreadable target must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_document_at_an_id_owning_no_change_is_not_a_node_and_the_id_is_unknown() {
        let dir = repo("nochange");
        let _ = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        // A node is the change its document is read from (S2-read, S2-set), so a document on
        // disk at an id that owns no change sits at no change: it is not a node, the id
        // resolves to nothing, and it refuses as an unknown id — D21's refusal — never as a
        // node whose change is missing.
        let orphan = "aaaaaaaa";
        let document = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"rust","ref":["src/a.rs"],"rationale":"y"}}"#;
        fs::write(dir.join(format!("nodes/{orphan}.json")), document).unwrap();
        let head = op_head(&dir);
        let before = listing(&dir);
        match record(&dir, &["src/a.rs", "--to", orphan]) {
            Err(refusal) => {
                assert_eq!(
                    refusal.cause(),
                    "unknown id",
                    "an id owning no change is not a node and refuses as unknown, got {refusal:?}"
                );
                assert!(
                    refusal.evidence().contains(&orphan.to_string()),
                    "the refusal names the id, got {refusal:?}"
                );
            }
            Ok(lines) => panic!("an id owning no change is not a node, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        assert!(working_copy(&dir).contains("src/a.rs"), "the edit stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_id_naming_an_existing_change_with_no_node_document_refuses_naming_it() {
        let dir = repo("nodoc");
        // A real change that carries no node document: described, but with no
        // `nodes/<id>.json`. The id maps to no node (uzlvzrpv), apart from D21's unknown
        // id — the root and this case are the two map failures the verb refuses.
        jj(&dir, &["describe", "-m", "a change with no node document"]);
        let bare = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        jj(&dir, &["new"]);
        edit(&dir, "src/a.rs", "a\n");
        let head = op_head(&dir);
        let before = listing(&dir);
        match record(&dir, &["src/a.rs", "--to", &bare]) {
            Err(refusal) => {
                assert_eq!(
                    refusal.cause(),
                    "no node for the id",
                    "an id that maps to no node refuses, got {refusal:?}"
                );
                assert!(
                    refusal.evidence().contains(&bare.to_string()),
                    "the refusal names the id, got {refusal:?}"
                );
            }
            Ok(lines) => panic!(
                "an id naming an existing change with no node document must refuse, got {lines:?}"
            ),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        assert!(working_copy(&dir).contains("src/a.rs"), "the edit stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_root_id_refuses_because_it_names_no_change() {
        let dir = repo("root");
        edit(&dir, "src/a.rs", "a\n");
        let head = op_head(&dir);
        let before = listing(&dir);
        match record(&dir, &["src/a.rs", "--to", "zzzzzzzz"]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"zzzzzzzz".to_string()),
                "the refusal names the id, got {refusal:?}"
            ),
            Ok(lines) => panic!("the root names no change and must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert_eq!(listing(&dir), before);
        assert!(working_copy(&dir).contains("src/a.rs"), "the edit stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_path_naming_no_edit_refuses_naming_it() {
        let dir = repo("noedit");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/b.rs", "b\n");
        let head = op_head(&dir);
        match record(&dir, &["src/a.rs", "--to", &target]) {
            Err(refusal) => assert!(refusal.evidence().contains(&"src/a.rs".to_string())),
            Ok(lines) => panic!("an unchanged path must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_path_the_target_does_not_own_refuses_naming_it() {
        let dir = repo("notowned");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/b.rs", "b\n");
        let head = op_head(&dir);
        match record(&dir, &["src/b.rs", "--to", &target]) {
            Err(refusal) => assert!(refusal.evidence().contains(&"src/b.rs".to_string())),
            Ok(lines) => panic!("an unowned path must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing was recorded");
        assert!(working_copy(&dir).contains("src/b.rs"), "the edit stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_successful_record_appends_one_marker() {
        let dir = repo("markers");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        assert!(!dir.join(".deccy/markers.json").exists());
        record(&dir, &["src/a.rs", "--to", &target]).expect("a placed recording");
        let state = markers_persist::load(&Store::new(&dir)).expect("the marker file loads");
        assert_eq!(state.markers.len(), 1, "exactly one marker");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_record_into_the_working_copy_appends_no_marker() {
        // S5-record-named (wotmvzyt): the target is the change the working copy is already
        // on, so the placement moves nothing and no marker is appended.
        let dir = repo("self");
        jj(&dir, &["describe", "-m", "self"]);
        let id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let document = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"rust","ref":["src/a.rs"],"rationale":"y"}}"#;
        fs::write(dir.join(format!("nodes/{id}.json")), document).unwrap();
        edit(&dir, "src/a.rs", "a\n");
        assert!(!dir.join(".deccy/markers.json").exists());
        record(&dir, &["src/a.rs", "--to", &id]).expect("a no-op recording succeeds");
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "no marker for a no-op recording"
        );
        let state =
            markers_persist::load(&Store::new(&dir)).expect("the marker file loads as empty");
        assert!(state.markers.is_empty(), "no marker is appended");
        assert_eq!(state.pos, 0, "the position stands");
        assert!(
            working_copy(&dir).contains("src/a.rs"),
            "the edit stays the working copy's"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_prints_one_line() {
        let dir = repo("help");
        let lines = record(&dir, &["--help"]).expect("help succeeds");
        assert_eq!(lines.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_answers_without_a_workspace() {
        // H1: the surface answers before the store is taken, so --help needs no workspace.
        let dir = scratch("help-rootless");
        let lines = record(&dir, &["--help"]).expect("help answers on a root-less context");
        assert_eq!(
            lines,
            vec![crate::verb_record::HELP.to_string()],
            "the verb's own help line"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_argument_without_a_workspace_refuses_naming_it() {
        // H1: a bad argument is refused by name, not preempted by the absent workspace.
        let dir = scratch("argument-rootless");
        match record(&dir, &["--frob"]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"--frob".to_string()),
                "the refusal names the argument, got {refusal:?}"
            ),
            Ok(lines) => panic!("the argument must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_open_draft_lands_its_owned_edits_turns_landed_and_leaves_other_records_alone() {
        // uvrltuvo: the source form is the recording the path form already is. One open draft
        // lands: the changed owned paths enter the node's change, the record's one document
        // turns `landed`, no node is created, and every other record stands untouched.
        let dir = landing_repo("land");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let other = target_with(&dir, &[("src/b.rs", "the other node's content\n")]);
        let draft = draft_workspace(&dir, "land-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let handle = "k4mz7qta";
        let document =
            record_document(handle, "open", &target, &edit_id, ("land-draft", &draft), &base);
        put_record(&dir, &document);

        // Another node's open draft: landing the first must not touch it.
        let other_handle = "uqwnzmtb";
        let other_base = owned_base(&dir, &other, &owned_set(&other, &["src/b.rs"]));
        let other_document =
            record_document(other_handle, "open", &other, &edit_id, ("other", &dir), &other_base);
        put_record(&dir, &other_document);

        let before_nodes = listing(&dir);
        assert!(!dir.join(".deccy/markers.json").exists());

        let lines = record(&dir, &["--from", handle, "--to", &target]).expect("the open draft lands");
        assert!(lines.is_empty(), "O7-record: the landing writes nothing to stdout");

        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            "the draft's edit",
            "the draft's changed owned path is in the node's change"
        );
        let landed = read_record(&registry(&dir), handle).expect("the landed record is retained");
        let mut expected = document.clone();
        expected["state"] = json!("landed");
        assert_eq!(
            landed, expected,
            "the one document says `landed` and keeps its key, references and base"
        );
        assert_eq!(
            read_record(&registry(&dir), other_handle).expect("the other record is retained"),
            other_document,
            "landing one draft leaves every other record exactly as it was"
        );
        assert_eq!(listing(&dir), before_nodes, "the landing creates no node");
        let markers = markers_persist::load(&Store::new(&dir)).expect("the marker file loads");
        assert_eq!(
            markers.markers.len(),
            1,
            "the landing is one recording: the record's write rides inside it, not beside it"
        );

        drop_workspace(&dir, "land-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_to_naming_another_node_refuses_places_nothing_and_keeps_the_records_node() {
        // uvrltuvo: `--to` must be the node the record names; the source never re-points the
        // draft and nothing is placed into any change.
        let dir = landing_repo("wrong-to");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let other = target_with(&dir, &[("src/b.rs", "the other node's content\n")]);
        let draft = draft_workspace(&dir, "wrong-to-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let handle = "k4mz7qta";
        let document =
            record_document(handle, "open", &target, &edit_id, ("wrong-to-draft", &draft), &base);
        put_record(&dir, &document);

        let before_target = jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]);
        let before_other = jj(&dir, &["file", "show", "-r", &other, "src/b.rs"]);
        match record(&dir, &["--from", handle, "--to", &other]) {
            Err(refusal) => assert!(!refusal.cause().is_empty(), "the refusal carries a cause"),
            Ok(lines) => panic!("a --to naming another node must refuse, got {lines:?}"),
        }

        assert_eq!(
            read_record(&registry(&dir), handle).expect("the record is retained"),
            document,
            "the record still names its own node and stays open"
        );
        assert_eq!(
            read_record(&registry(&dir), handle).unwrap()["node"], json!(target),
            "the source never re-points the draft at the caller's node"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            before_target,
            "nothing is placed in the record's node"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &other, "src/b.rs"]),
            before_other,
            "nothing is placed in the caller's node"
        );
        assert!(
            jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the draft's edit stays in its workspace"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a refused landing is not an operation"
        );

        drop_workspace(&dir, "wrong-to-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_absent_handle_refuses_and_places_nothing() {
        // uvrltuvo: the handle is the only value that picks a record out; a handle the
        // directory does not name is a record that is not there, so the invocation refuses.
        let dir = landing_repo("absent");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let draft = draft_workspace(&dir, "absent-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        // A record exists, under another handle: the directory was consulted and this handle
        // simply names nothing.
        let present_handle = "uqwnzmtb";
        let document = record_document(
            present_handle,
            "open",
            &target,
            &edit_id,
            ("absent-draft", &draft),
            &base,
        );
        put_record(&dir, &document);

        let before_target = jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]);
        match record(&dir, &["--from", "k4mz7qta", "--to", &target]) {
            Err(refusal) => assert!(!refusal.cause().is_empty(), "the refusal carries a cause"),
            Ok(lines) => panic!("an absent handle must refuse, got {lines:?}"),
        }

        assert_eq!(
            read_record(&registry(&dir), present_handle).expect("the present record is retained"),
            document,
            "the present record is not the source and is untouched"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            before_target,
            "nothing is placed"
        );
        assert!(
            jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the draft's edit stays in its workspace"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a refused landing is not an operation"
        );

        drop_workspace(&dir, "absent-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_stale_open_record_refuses_naming_its_handle_and_differing_paths() {
        // ptlmptoy, qrqsxyzz: a record whose base no longer matches the owned set refuses
        // landing, places nothing, and names the handle with the paths that differ.
        let dir = landing_repo("stale");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let draft = draft_workspace(&dir, "stale-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let mut base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        base.insert(
            "src/a.rs".to_string(),
            identity(b"content the target's owned set no longer holds\n"),
        );
        let handle = "k4mz7qta";
        let document =
            record_document(handle, "open", &target, &edit_id, ("stale-draft", &draft), &base);
        put_record(&dir, &document);

        let before_target = jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]);
        match record(&dir, &["--from", handle, "--to", &target]) {
            Err(refusal) => {
                assert!(
                    names(&refusal, handle),
                    "the stale record is named by its handle: {refusal:?}"
                );
                assert!(
                    names(&refusal, "src/a.rs"),
                    "the differing path is named: {refusal:?}"
                );
            }
            Ok(lines) => panic!("a stale open record must refuse, got {lines:?}"),
        }

        assert_eq!(
            read_record(&registry(&dir), handle).expect("the stale record is retained"),
            document,
            "the stale record's state is not moved"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            before_target,
            "nothing is placed"
        );
        assert!(
            jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the draft's edit stays in its workspace"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a refused landing is not an operation"
        );

        drop_workspace(&dir, "stale-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_stale_sibling_refuses_the_fresh_records_landing() {
        // qrqsxyzz: the rule holds for each open record naming the node, so a fresh addressed
        // record is refused by a stale sibling and the refusal names the sibling, not itself.
        let dir = landing_repo("stale-sibling");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let draft = draft_workspace(&dir, "stale-sibling-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let fresh_handle = "k4mz7qta";
        let fresh =
            record_document(fresh_handle, "open", &target, &edit_id, ("stale-sibling-draft", &draft), &base);
        put_record(&dir, &fresh);
        let stale_handle = "uqwnzmtb";
        let mut stale_base = base.clone();
        stale_base.insert(
            "src/a.rs".to_string(),
            identity(b"content the target's owned set no longer holds\n"),
        );
        let stale = record_document(
            stale_handle,
            "open",
            &target,
            &edit_id,
            ("stale-sibling", &draft),
            &stale_base,
        );
        put_record(&dir, &stale);

        let before_target = jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]);
        match record(&dir, &["--from", fresh_handle, "--to", &target]) {
            Err(refusal) => {
                assert!(
                    names(&refusal, stale_handle),
                    "the stale sibling is named by its handle: {refusal:?}"
                );
                assert!(
                    names(&refusal, "src/a.rs"),
                    "the differing path is named: {refusal:?}"
                );
            }
            Ok(lines) => panic!("a stale sibling must refuse the landing, got {lines:?}"),
        }

        assert_eq!(
            read_record(&registry(&dir), fresh_handle).expect("the fresh record is retained"),
            fresh,
            "the fresh record stays open"
        );
        assert_eq!(
            read_record(&registry(&dir), stale_handle).expect("the stale record is retained"),
            stale,
            "the stale record's state is not moved"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            before_target,
            "nothing is placed"
        );
        assert!(
            jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the draft's edit stays in its workspace"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a refused landing is not an operation"
        );

        drop_workspace(&dir, "stale-sibling-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_record_update_is_part_of_the_one_recording() {
        // qvkuluyw: the registry write is not an operation. A landing keeps exactly one marker
        // and one journal entry — the recording — so the `landed` write is not bracketed apart.
        let dir = landing_repo("one-recording");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let draft = draft_workspace(&dir, "one-recording-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let handle = "k4mz7qta";
        put_record(
            &dir,
            &record_document(handle, "open", &target, &edit_id, ("one-recording-draft", &draft), &base),
        );

        record(&dir, &["--from", handle, "--to", &target]).expect("the open draft lands");

        let store = Store::new(&dir);
        let markers = markers_persist::load(&store).expect("the marker file loads");
        assert_eq!(markers.markers.len(), 1, "the landing is exactly one recording");
        assert_eq!(markers.markers[0].kind, Kind::Record, "the recording case");
        assert_eq!(markers.markers[0].target, target, "the marker names the node");
        assert_eq!(markers.pos, 1, "the marker is applied");
        assert_eq!(
            markers.markers[0].entries.len(),
            1,
            "the recording kept one journal entry, not one for the placement and one for the write"
        );
        let entry = journal::get(&store, &markers.markers[0].entries[0])
            .expect("the kept journal entry loads and verifies");
        assert_eq!(entry.kind, "record");
        assert_eq!(entry.target, target);

        drop_workspace(&dir, "one-recording-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn named_paths_must_state_the_changed_owned_set_exactly() {
        // uvrltuvo: the source form's `[path...]` arguments state the changed owned set the
        // caller expects; they neither narrow the landing to a subset nor license a path that
        // is not a changed owned path. Any disagreement refuses and places nothing.
        let dir = landing_repo("paths-exact");
        let target = target_with(
            &dir,
            &[
                ("src/a.rs", "a at the target\n"),
                ("src/b.rs", "b at the target\n"),
                ("src/c.rs", "c at the target\n"),
            ],
        );
        let draft = draft_workspace(&dir, "paths-exact-draft", &target);
        // The draft changes two of the three owned paths; src/c.rs it leaves alone.
        edit(&draft, "src/a.rs", "a in the draft\n");
        edit(&draft, "src/b.rs", "b in the draft\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let refs = &["src/a.rs", "src/b.rs", "src/c.rs"];
        let base = owned_base(&dir, &target, &owned_set(&target, refs));
        let handle = "k4mz7qta";
        let document = record_document(
            handle,
            "open",
            &target,
            &edit_id,
            ("paths-exact-draft", &draft),
            &base,
        );
        put_record(&dir, &document);

        let before_a = jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]);
        let before_b = jj(&dir, &["file", "show", "-r", &target, "src/b.rs"]);
        let before_c = jj(&dir, &["file", "show", "-r", &target, "src/c.rs"]);
        // A named subset: src/a.rs alone leaves the changed src/b.rs unstated, and a draft
        // lands as a whole. A named path that is not a changed owned path: src/c.rs is owned
        // but the draft did not change it. Both disagree with the changed owned set.
        let cases: Vec<Vec<&str>> = vec![
            vec!["--from", handle, "--to", &target, "src/a.rs"],
            vec!["--from", handle, "--to", &target, "src/a.rs", "src/b.rs", "src/c.rs"],
        ];
        for args in cases {
            match record(&dir, &args) {
                Err(refusal) => assert!(
                    !refusal.cause().is_empty(),
                    "{args:?} must refuse with a cause, got {refusal:?}"
                ),
                Ok(lines) => panic!("{args:?} disagrees with the changed owned set, got {lines:?}"),
            }
        }

        assert_eq!(
            read_record(&registry(&dir), handle).expect("the record is retained"),
            document,
            "a refused landing leaves the record open and unchanged"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            before_a,
            "nothing is placed for the subset"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/b.rs"]),
            before_b,
            "the unstated changed path is not placed either"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/c.rs"]),
            before_c,
            "the unchanged owned path is not placed"
        );
        let draft_diff = jj(&draft, &["diff", "--summary"]);
        assert!(
            draft_diff.contains("src/a.rs") && draft_diff.contains("src/b.rs"),
            "the draft's edits stay in its workspace, got {draft_diff:?}"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a refused landing is not an operation"
        );

        // The control: the same draft, with the changed owned set stated exactly, does land —
        // so the refusals above are the disagreement with the set, not the form answering no.
        record(
            &dir,
            &["--from", handle, "--to", &target, "src/a.rs", "src/b.rs"],
        )
        .expect("the stated set is the changed owned set");
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            "a in the draft",
            "the stated changed owned path is placed"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/b.rs"]),
            "b in the draft",
            "the other stated changed owned path is placed"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/c.rs"]),
            before_c,
            "the unchanged owned path is untouched"
        );
        assert_eq!(
            read_record(&registry(&dir), handle).unwrap()["state"],
            json!("landed"),
            "the stated set lands the draft"
        );

        drop_workspace(&dir, "paths-exact-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_source_that_is_the_target_change_refuses_and_places_nothing() {
        // uvrltuvo: a source that is the target change is the change into itself. The record's
        // `edit` is the target's owning change id, so the source reaches the change `--to`
        // names and the recording refuses.
        let dir = landing_repo("source-is-target");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let draft = draft_workspace(&dir, "source-is-target-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let handle = "k4mz7qta";
        let document = record_document(
            handle,
            "open",
            &target,
            &target,
            ("source-is-target-draft", &draft),
            &base,
        );
        put_record(&dir, &document);

        let before = jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]);
        match record(&dir, &["--from", handle, "--to", &target]) {
            Err(refusal) => assert!(
                !refusal.cause().is_empty(),
                "the refusal carries a cause, got {refusal:?}"
            ),
            Ok(lines) => panic!("a source that is the target change must refuse, got {lines:?}"),
        }

        assert_eq!(
            read_record(&registry(&dir), handle).expect("the record is retained"),
            document,
            "the source is not landed and its state is not written"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            before,
            "nothing is placed"
        );
        assert!(
            jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the draft's edit stays in its workspace"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a refused landing is not an operation"
        );

        // The control: the same draft with its own edit named as the source lands, so the
        // refusal above is the target-change identity, not the form answering no.
        let own_edit = jj(
            &draft,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let mut corrected = document.clone();
        corrected["edit"] = json!(own_edit);
        put_record(&dir, &corrected);
        record(&dir, &["--from", handle, "--to", &target])
            .expect("a source that is not the target change lands");
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            "the draft's edit",
            "the draft's edit is placed once the source is its own change"
        );
        assert_eq!(
            read_record(&registry(&dir), handle).unwrap()["state"],
            json!("landed"),
            "the landing turns the record `landed`"
        );

        drop_workspace(&dir, "source-is-target-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_landed_record_refuses_and_places_nothing() {
        // uvrltuvo, vkkwusty: a record has two states and one transition, and a landed record
        // is retained. A source naming a landed record has no landing left; it refuses and
        // changes nothing, the record included.
        let dir = landing_repo("landed");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let draft = draft_workspace(&dir, "landed-draft", &target);
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let handle = "k4mz7qta";
        let document = record_document(
            handle,
            "landed",
            &target,
            &edit_id,
            ("landed-draft", &draft),
            &base,
        );
        put_record(&dir, &document);

        let before = jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]);
        match record(&dir, &["--from", handle, "--to", &target]) {
            Err(refusal) => assert!(
                !refusal.cause().is_empty(),
                "the refusal carries a cause, got {refusal:?}"
            ),
            Ok(lines) => panic!("a landed record must refuse, got {lines:?}"),
        }

        assert_eq!(
            read_record(&registry(&dir), handle).expect("the landed record is retained"),
            document,
            "the record stays `landed`: there is no transition out of it"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            before,
            "nothing is placed"
        );
        assert!(
            jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the draft's edit stays in its workspace"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a refused landing is not an operation"
        );

        // The control: the same draft, open, lands — so the refusal above is the record's
        // state, not the form answering no.
        let mut reopened = document.clone();
        reopened["state"] = json!("open");
        put_record(&dir, &reopened);
        record(&dir, &["--from", handle, "--to", &target]).expect("an open record lands");
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            "the draft's edit",
            "the open draft's change is placed"
        );
        assert_eq!(
            read_record(&registry(&dir), handle).unwrap()["state"],
            json!("landed"),
            "the landing turns the open record `landed`"
        );

        drop_workspace(&dir, "landed-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_draft_deletion_of_an_owned_path_is_recorded() {
        // uvrltuvo, xzxtxymq: within the owned set a path is changed when the draft's content
        // for it differs from the target's, and absence differs from presence. A deletion the
        // draft carries is therefore a changed owned path and it is recorded.
        let dir = landing_repo("deletion");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let draft = draft_workspace(&dir, "deletion-draft", &target);
        fs::remove_file(draft.join("src/a.rs")).unwrap();
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let handle = "k4mz7qta";
        let document = record_document(
            handle,
            "open",
            &target,
            &edit_id,
            ("deletion-draft", &draft),
            &base,
        );
        put_record(&dir, &document);
        let before_nodes = listing(&dir);

        let lines = record(&dir, &["--from", handle, "--to", &target])
            .expect("a removed owned path is a change the landing records");
        assert!(lines.is_empty(), "O7-record: nothing on stdout");

        let files = jj(&dir, &["file", "list", "-r", &target]);
        assert!(
            !files.lines().any(|line| line == "src/a.rs"),
            "the owned path the draft removed is placed into the target change, got {files:?}"
        );
        let landed = read_record(&registry(&dir), handle).expect("the record is retained");
        let mut expected = document.clone();
        expected["state"] = json!("landed");
        assert_eq!(
            landed, expected,
            "the deletion is recorded and the record turns `landed`"
        );
        assert_eq!(listing(&dir), before_nodes, "the landing creates no node");
        assert!(
            !jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the removed path moved out of the draft"
        );

        drop_workspace(&dir, "deletion-draft", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_described_draft_lands_without_an_editor_and_keeps_the_nodes_description() {
        // The landing is the recording it already is: it places the changed owned paths into
        // the node's change and nothing else (rvyrosmp, uvrltuvo, qvkuluyw). The node's change
        // carries its own description — the summary the graph lists — and the draft's working
        // copy carries one too; moving the edits must not stop to ask a person to combine the
        // two descriptions, and must leave the node's change with the description it had.
        let dir = landing_repo("described-draft");
        let target = target_with(&dir, &[("src/a.rs", "the target's content\n")]);
        let node_description = description(&dir, &target);
        assert_eq!(
            node_description, "target",
            "premise: the node's change carries its own description"
        );

        let draft = draft_workspace(&dir, "described-draft-ws", &target);
        // The draft's working copy is described, so the move joins two described changes.
        jj(&draft, &["describe", "-m", "the draft's own summary"]);
        assert_eq!(
            description(&draft, "@"),
            "the draft's own summary",
            "premise: the draft's working copy carries a description of its own"
        );
        edit(&draft, "src/a.rs", "the draft's edit\n");
        let edit_id = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let base = owned_base(&dir, &target, &owned_set(&target, &["src/a.rs"]));
        let handle = "k4mz7qta";
        let document = record_document(
            handle,
            "open",
            &target,
            &edit_id,
            ("described-draft-ws", &draft),
            &base,
        );
        put_record(&dir, &document);

        let (script, editor_marker) = failing_editor(&dir);
        let _editor = editor_env(&script);
        let before_nodes = listing(&dir);

        let outcome = record(&dir, &["--from", handle, "--to", &target]);
        let lines = match outcome {
            Ok(lines) => lines,
            Err(refusal) => panic!(
                "a described draft lands without an editor, got {refusal:?}"
            ),
        };
        assert!(lines.is_empty(), "O7-record: nothing on stdout");

        assert!(
            !editor_marker.exists(),
            "the landing opened no editor: a described draft is placed, not merged by hand"
        );
        assert_eq!(
            description(&dir, &target),
            node_description,
            "the node's change keeps its own description; the draft's is not merged into it"
        );
        assert_eq!(
            jj(&dir, &["file", "show", "-r", &target, "src/a.rs"]),
            "the draft's edit",
            "the draft's changed owned path is in the node's change"
        );
        assert_eq!(
            read_record(&registry(&dir), handle).expect("the landed record is retained")["state"],
            json!("landed"),
            "the landing turns the record `landed`"
        );
        assert_eq!(listing(&dir), before_nodes, "the landing creates no node");
        let markers = markers_persist::load(&Store::new(&dir)).expect("the marker file loads");
        assert_eq!(
            markers.markers.len(),
            1,
            "the landing is the one recording (qvkuluyw)"
        );
        assert!(
            !jj(&draft, &["diff", "--summary"]).contains("src/a.rs"),
            "the placed edit moved out of the draft's workspace"
        );

        drop_workspace(&dir, "described-draft-ws", &draft);
        fs::remove_dir_all(&dir).unwrap();
    }
}
