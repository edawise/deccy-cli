//! Gate for M-V-remove (the remove verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj workspace whose
//! node files are named by real change ids and live in real changes — it reaches node_load
//! and the marker file, so this is a crate of its own, needing serde, serde_json and the
//! schema validator. The compile-time `REMOVE_ROW` is the tie to M-route (a Verb row for the
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
#[path = "../../../src/node_set.rs"]
mod node_set;
#[path = "../../../src/refusal.rs"]
mod refusal;
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
#[path = "../../../src/jj_abandon.rs"]
mod jj_abandon;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_remove.rs"]
mod verb_remove;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a Verb row for `remove` (M-route, ynvwvzrm) compiles.
const REMOVE_ROW: Verb<Context> = Verb {
    name: "remove",
    run: crate::verb_remove::run,
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
    use crate::verb_remove::run;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-remove-{}-{}",
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

    /// A scratch jj workspace with a tracked `nodes/`, so a planted node's file lives in the
    /// change that owns it. Only the state directory stays out of jj's tracking; the schemas are
    /// carried in the binary (`include_str!`), never read from the repo.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".gitignore"), ".deccy/\n").unwrap();
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        dir
    }

    /// Plant a real change owning a valid node document with the given edges; the working copy
    /// is left on a fresh change above it, so abandoning the planted change removes its file.
    /// Returns the planted change id.
    fn plant(dir: &Path, kind: &str, depends_on: &[&str], evaluates: &[&str]) -> String {
        jj(dir, &["describe", "-m", "planted"]);
        let id = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        let mut fields = String::new();
        if kind == "evaluation" {
            fields.push_str(&format!(
                "\"evaluates\":{}",
                serde_json::to_string(evaluates).unwrap()
            ));
            if !depends_on.is_empty() {
                fields.push_str(&format!(
                    ",\"depends_on\":{}",
                    serde_json::to_string(depends_on).unwrap()
                ));
            }
        } else {
            fields.push_str(&format!(
                "\"depends_on\":{}",
                serde_json::to_string(depends_on).unwrap()
            ));
        }
        let document = format!(
            r#"{{"kind":"{kind}",{fields},"content":{{"type":"text","ref":"inline","content":"x","rationale":"y"}}}}"#
        );
        fs::write(dir.join(format!("nodes/{id}.json")), document).unwrap();
        jj(dir, &["new"]);
        id
    }

    /// `deccy remove <args>` through the real handler over the scratch workspace.
    fn remove(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repo resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        run(&context, &owned)
    }

    /// The node files under `nodes/`, sorted; an emptied directory reads as no nodes.
    fn listing(dir: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(dir.join("nodes")) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn op_head(dir: &Path) -> String {
        jj(dir, &["op", "log", "--limit", "1", "--no-graph", "-T", "id"])
    }

    /// Every change id in history, one per line — the change listing the refusals leave put.
    fn changes(dir: &Path) -> String {
        jj(
            dir,
            &["log", "--no-graph", "-T", "change_id.shortest(8) ++ \"\\n\""],
        )
    }

    #[test]
    fn the_named_node_change_is_abandoned_and_stdout_is_empty() {
        let dir = repo("abandon");
        let target = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        assert!(listing(&dir).contains(&format!("{target}.json")));
        let lines = remove(&dir, &[&target]).expect("a dependant-free removal");
        assert!(lines.is_empty(), "O7-remove: nothing on stdout");
        assert!(
            !listing(&dir).contains(&format!("{target}.json")),
            "the abandoned node's file left the listing, got {:?}",
            listing(&dir)
        );
        assert!(
            !changes(&dir).contains(&target),
            "the abandoned change id left jj log"
        );
        assert_eq!(crate::REMOVE_ROW.name, "remove");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_node_with_dependants_refuses_naming_each() {
        let dir = repo("dependants");
        let target = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let dependant = plant(&dir, "decision", &[target.as_str()], &[]);
        let gate = plant(&dir, "evaluation", &["zzzzzzzz"], &[target.as_str(), dependant.as_str()]);
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        match remove(&dir, &[&target]) {
            Err(refusal) => {
                assert!(
                    refusal.evidence().contains(&dependant),
                    "names the decision dependant, got {refusal:?}"
                );
                assert!(
                    refusal.evidence().contains(&gate),
                    "names the wider gate, got {refusal:?}"
                );
            }
            Ok(lines) => panic!("a node with dependants must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_gate_evaluating_the_node_among_others_refuses_named() {
        let dir = repo("widergate");
        let target = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let other = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let gate = plant(&dir, "evaluation", &["zzzzzzzz"], &[target.as_str(), other.as_str()]);
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        match remove(&dir, &[&target]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&gate),
                "the wider gate is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("a gate evaluating beyond the node must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_nodes_own_gate_is_abandoned_with_it() {
        let dir = repo("orphan");
        let target = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let gate = plant(&dir, "evaluation", &["zzzzzzzz"], &[target.as_str()]);
        remove(&dir, &[&target]).expect("a dependant-free removal with its own gate");
        assert!(
            !listing(&dir).contains(&format!("{target}.json")),
            "the named node's file left the listing"
        );
        assert!(
            !listing(&dir).contains(&format!("{gate}.json")),
            "no gate of the removed node is left behind"
        );
        assert!(!changes(&dir).contains(&target), "the node's change left");
        assert!(!changes(&dir).contains(&gate), "its own gate's change left with it");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unreadable_node_refuses_named() {
        let dir = repo("unreadable");
        let target = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        // A real change owning a node document that is not valid JSON — the read reaches it.
        jj(&dir, &["describe", "-m", "broken"]);
        let broken = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        fs::write(dir.join(format!("nodes/{broken}.json")), "{ not json").unwrap();
        jj(&dir, &["new"]);
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        match remove(&dir, &[&target]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&broken),
                "the node that did not load is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("an unreadable node must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_target_whose_document_cannot_be_read_refuses_naming_it() {
        let dir = repo("unreadable-target");
        jj(&dir, &["describe", "-m", "broken target"]);
        let target = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        fs::write(dir.join(format!("nodes/{target}.json")), "{ not json").unwrap();
        jj(&dir, &["new"]);
        let head = op_head(&dir);
        let before = listing(&dir);
        match remove(&dir, &[&target]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&target),
                "the target that could not be read is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("an unreadable target must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_root_refuses_because_it_names_no_change() {
        let dir = repo("root");
        let _ = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        match remove(&dir, &["zzzzzzzz"]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"zzzzzzzz".to_string()),
                "the root is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("the root names no change and must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_document_at_an_id_owning_no_change_is_not_a_node_and_the_id_is_unknown() {
        let dir = repo("nochange");
        let _ = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        // A node is the change its document is read from (S2-read, S2-set), so a document on
        // disk at an id that owns no change sits at no change: it is not a node, the id
        // resolves to nothing, and it refuses as an unknown id — D21's refusal — never as a
        // node whose change is gone.
        let orphan = "aaaaaaaa";
        let document = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"x","rationale":"y"}}"#;
        fs::write(dir.join(format!("nodes/{orphan}.json")), document).unwrap();
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        match remove(&dir, &[orphan]) {
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
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unknown_id_refuses_naming_it_with_nothing_removed() {
        let dir = repo("unknown");
        let _ = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        match remove(&dir, &["abcdefgh"]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"abcdefgh".to_string()),
                "the unknown id is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("an unknown id must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_id_refuses() {
        let dir = repo("noid");
        let _ = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        match remove(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("no id must refuse, got {lines:?}"),
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_argument_beyond_the_id_refuses_naming_it() {
        let dir = repo("beyond");
        let target = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        let head = op_head(&dir);
        let before = listing(&dir);
        let log = changes(&dir);
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec![target.as_str(), "extra"], "extra"),
            (vec![target.as_str(), "second"], "second"),
            (vec!["--bogus", target.as_str()], "--bogus"),
            (vec!["-x"], "-x"),
        ];
        for (args, token) in cases {
            match remove(&dir, &args) {
                Err(refusal) => assert!(
                    refusal.evidence().contains(&token.to_string()),
                    "{args:?} must name {token:?}, got {refusal:?}"
                ),
                Ok(lines) => panic!("{args:?} must refuse, got {lines:?}"),
            }
        }
        assert_eq!(op_head(&dir), head, "nothing moved");
        assert_eq!(listing(&dir), before, "nothing was removed");
        assert_eq!(changes(&dir), log, "no change moved");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_successful_removal_appends_one_marker_pair() {
        let dir = repo("markers");
        let target = plant(&dir, "decision", &["zzzzzzzz"], &[]);
        assert!(!dir.join(".deccy/markers.json").exists());
        remove(&dir, &[&target]).expect("a dependant-free removal");
        let state = markers_persist::load(&Store::new(&dir)).expect("the marker file loads");
        assert_eq!(state.markers.len(), 1, "exactly one bracketed pair");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_prints_one_line() {
        let dir = repo("help");
        let lines = remove(&dir, &["--help"]).expect("help succeeds");
        assert_eq!(lines.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
