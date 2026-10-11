//! Gate for M-V-record (the record verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj workspace whose
//! node files are named by real change ids — it reaches node_load and the
//! marker file, so this is a crate of its own, needing serde, serde_json and the schema
//! validator. The compile-time `RECORD_ROW` is the tie to M-route (a Verb row for the
//! handler).

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
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::context::resolve;
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
    fn a_successful_record_appends_one_marker_pair() {
        let dir = repo("markers");
        let target = target(&dir, &["src/a.rs"]);
        edit(&dir, "src/a.rs", "a\n");
        assert!(!dir.join(".deccy/markers.json").exists());
        record(&dir, &["src/a.rs", "--to", &target]).expect("a placed recording");
        let state = markers_persist::load(&Store::new(&dir)).expect("the marker file loads");
        assert_eq!(state.markers.len(), 1, "exactly one bracketed pair");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_prints_one_line() {
        let dir = repo("help");
        let lines = record(&dir, &["--help"]).expect("help succeeds");
        assert_eq!(lines.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
