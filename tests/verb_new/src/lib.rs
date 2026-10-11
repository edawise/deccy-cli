//! Gate for M-V-new (the new verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj workspace whose
//! node files are named by real change ids — it reaches node_load, store_write and the marker
//! file, so this is a crate of its own, needing serde, serde_json and the schema validator. The
//! compile-time `NEW_ROW` is the tie to M-route (a Verb row for the handler).

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
#[path = "../../../src/store_write.rs"]
mod store_write;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/journal.rs"]
mod journal;
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
#[path = "../../../src/markers_capture.rs"]
mod markers_capture;
#[path = "../../../src/jj_new.rs"]
mod jj_new;
#[path = "../../../src/jj_abandon.rs"]
mod jj_abandon;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_new.rs"]
mod verb_new;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a Verb row for `new` (M-route, ynvwvzrm) compiles.
const NEW_ROW: Verb<Context> = Verb {
    name: "new",
    run: crate::verb_new::run,
};

#[cfg(test)]
mod gate {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::Value;

    use crate::jj::Jj;
    use crate::context::resolve;
    use crate::markers_persist;
    use crate::refusal::Refusal;
    use crate::store::Store;
    use crate::node_load;
    use crate::store_path::Id;
    use crate::verb_new::run;

    /// A bare scratch root under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-new-{}-{}",
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

    /// A scratch jj workspace with a tracked `nodes/` (role="a real workspace"). The schemas are
    /// carried in the binary (`include_str!`), never read from the repo. Node files are tracked
    /// because the read reaches a document at the change that owns it; only the state directory
    /// stays out of jj's tracking, recorded in `.git/info/exclude` the way `deccy init` records
    /// it — a root-position birth checks out a sibling tree, which drops an untracked
    /// `.gitignore` and would then snapshot and delete the marker file.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".git/info/exclude"), ".deccy/\n").unwrap();
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        dir
    }

    /// Create `n` real changes, each carrying a valid node file named by its change id; the
    /// last created is the working copy. Returns the ids in creation order.
    fn dependencies(dir: &Path, n: usize) -> Vec<String> {
        let mut ids = Vec::new();
        for i in 0..n {
            jj(dir, &["describe", "-m", &format!("dependency {i}")]);
            let id = jj(
                dir,
                &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
            );
            let document = format!(
                r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"dep {i}","rationale":"y"}}}}"#
            );
            fs::write(dir.join(format!("nodes/{id}.json")), document).unwrap();
            ids.push(id);
            if i + 1 < n {
                jj(dir, &["new"]);
            }
        }
        ids
    }

    /// Write a minimal valid node file named by `id`.
    fn write_node(dir: &Path, id: &str, label: &str) {
        let document = format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"{label}","rationale":"y"}}}}"#
        );
        fs::write(dir.join(format!("nodes/{id}.json")), document).unwrap();
    }

    /// Create two changes that diverge off one base, each carrying a valid node file; the
    /// `right` sibling is the working copy. Returns (base, left, right).
    fn forked_dependencies(dir: &Path) -> (String, String, String) {
        let base = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        jj(dir, &["describe", "-m", "base"]);
        write_node(dir, &base, "base");
        jj(dir, &["new", "-m", "left"]);
        let left = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        write_node(dir, &left, "left");
        jj(dir, &["new", &base, "-m", "right"]);
        let right = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        write_node(dir, &right, "right");
        (base, left, right)
    }

    /// `deccy new <args>` through the real handler over the scratch workspace.
    fn new(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
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

    fn load(dir: &Path, id: &str) -> Value {
        node_load::load(&Jj::new("jj", dir), &Id::parse(id).unwrap())
            .expect("the born document loads")
    }

    #[test]
    fn the_born_file_exists_and_validates() {
        let dir = repo("born");
        let deps = dependencies(&dir, 1);
        let lines = new(&dir, &["-m", "a summary", "--on", &deps[0]]).expect("a placed birth");
        assert_eq!(lines.len(), 1, "one line (O6)");
        let id = &lines[0];
        assert!(dir.join(format!("nodes/{id}.json")).is_file());
        let _ = load(&dir, id); // node_load validates against schemas/node.json
        assert_eq!(crate::NEW_ROW.name, "new");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_skeleton_is_the_fixed_placeholder() {
        let dir = repo("skeleton");
        let deps = dependencies(&dir, 2);
        let lines = new(&dir, &["--on", &deps[0], "-m", "a summary", "--on", &deps[1]])
            .expect("a placed birth");
        let document = load(&dir, &lines[0]);
        assert_eq!(document["kind"], "decision");
        assert_eq!(document["content"]["type"], "text");
        assert_eq!(document["content"]["ref"], "inline");
        assert_eq!(document["content"]["content"], "<placeholder>");
        assert_eq!(document["content"]["rationale"], "<placeholder>");
        assert_eq!(
            document["depends_on"],
            serde_json::json!([deps[0], deps[1]]),
            "exactly the given ids, in the order given"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_change_follows_the_named_dependencies_with_the_summary() {
        let dir = repo("placement");
        let deps = dependencies(&dir, 1);
        new(&dir, &["-m", "the summary", "--on", &deps[0]]).expect("a placed birth");
        let summary = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "description.first_line()"],
        );
        assert_eq!(summary, "the summary");
        let parent = jj(
            &dir,
            &["log", "-r", "@-", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        assert_eq!(parent, deps[0], "opened after the named dependency");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_on_refuses_and_creates_nothing() {
        let dir = repo("noon");
        let _ = dependencies(&dir, 1);
        let before = listing(&dir);
        let head = op_head(&dir);
        match new(&dir, &["-m", "a summary"]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("a missing --on must refuse, got {lines:?}"),
        }
        assert_eq!(listing(&dir), before);
        assert_eq!(op_head(&dir), head, "nothing was created");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_m_refuses_and_creates_nothing() {
        let dir = repo("nom");
        let deps = dependencies(&dir, 1);
        let before = listing(&dir);
        let head = op_head(&dir);
        match new(&dir, &["--on", &deps[0]]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("a missing -m must refuse, got {lines:?}"),
        }
        assert_eq!(listing(&dir), before);
        assert_eq!(op_head(&dir), head, "nothing was created");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unnamed_argument_refuses_naming_it() {
        let dir = repo("unnamed");
        let deps = dependencies(&dir, 1);
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["--bogus", "-m", "x", "--on", &deps[0]], "--bogus"),
            (vec!["-m", "x", "--on"], "--on"),
            (vec!["-m", "x", "-m", "y", "--on", &deps[0]], "-m"),
            (vec!["bareword", "-m", "x", "--on", &deps[0]], "bareword"),
        ];
        for (args, token) in cases {
            match new(&dir, &args) {
                Err(refusal) => assert!(
                    refusal.evidence().contains(&token.to_string()),
                    "{args:?} must name {token:?}, got {refusal:?}"
                ),
                Ok(lines) => panic!("{args:?} must refuse, got {lines:?}"),
            }
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_answers_without_a_workspace() {
        // H1: the surface answers before the store is taken, so --help needs no workspace.
        let dir = scratch("help-rootless");
        let lines = new(&dir, &["--help"]).expect("help answers on a root-less context");
        assert_eq!(
            lines,
            vec![crate::verb_new::HELP.to_string()],
            "the verb's own help line"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_argument_without_a_workspace_refuses_naming_it() {
        // H1: a bad argument is refused by name, not preempted by the absent workspace.
        let dir = scratch("argument-rootless");
        match new(&dir, &["--frob"]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"--frob".to_string()),
                "the refusal names the argument, got {refusal:?}"
            ),
            Ok(lines) => panic!("the argument must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn flags_are_recognized_in_any_order() {
        let dir = repo("interleaved");
        let deps = dependencies(&dir, 2);
        let lines = new(
            &dir,
            &["--on", &deps[0], "-m", "interleaved", "--on", &deps[1]],
        )
        .expect("the flags interleave");
        assert_eq!(lines.len(), 1);
        let document = load(&dir, &lines[0]);
        assert_eq!(document["depends_on"], serde_json::json!([deps[0], deps[1]]));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unknown_on_id_refuses_before_birth_naming_each() {
        let dir = repo("unknown");
        let deps = dependencies(&dir, 1);
        let before = listing(&dir);
        let head = op_head(&dir);
        match new(
            &dir,
            &[
                "-m", "x", "--on", "abcdefgh", "--on", "12345678", "--on", &deps[0],
            ],
        ) {
            Err(refusal) => {
                assert!(refusal.evidence().contains(&"abcdefgh".to_string()));
                assert!(refusal.evidence().contains(&"12345678".to_string()));
            }
            Ok(lines) => panic!("an unknown --on id must refuse, got {lines:?}"),
        }
        assert_eq!(listing(&dir), before, "no file was created");
        assert_eq!(op_head(&dir), head, "no change was created");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_on_target_that_cannot_be_read_refuses_before_birth() {
        let dir = repo("unreadable-on");
        jj(&dir, &["describe", "-m", "broken dependency"]);
        let broken = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        fs::write(dir.join(format!("nodes/{broken}.json")), "{ not json").unwrap();
        jj(&dir, &["new"]);
        let before = listing(&dir);
        let head = op_head(&dir);
        match new(&dir, &["-m", "x", "--on", &broken]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&broken),
                "the --on target that could not be read is named, got {refusal:?}"
            ),
            Ok(lines) => panic!("an unreadable --on target must refuse, got {lines:?}"),
        }
        assert_eq!(listing(&dir), before, "no file was created");
        assert_eq!(op_head(&dir), head, "no change was created");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_root_id_is_accepted_and_recorded() {
        let dir = repo("rootdep");
        let lines = new(&dir, &["-m", "a root child", "--on", "zzzzzzzz"]).expect("the root places");
        let document = load(&dir, &lines[0]);
        assert_eq!(document["depends_on"], serde_json::json!(["zzzzzzzz"]));
        let parent = jj(
            &dir,
            &["log", "-r", "@-", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        assert_eq!(parent, "zzzzzzzz");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_successful_birth_appends_one_marker() {
        let dir = repo("markers");
        let deps = dependencies(&dir, 1);
        assert!(!dir.join(".deccy/markers.json").exists());
        let lines = new(&dir, &["-m", "a summary", "--on", &deps[0]]).expect("a placed birth");
        let state = markers_persist::load(&Store::new(&dir)).expect("the marker file loads");
        assert_eq!(state.markers.len(), 1, "exactly one marker");
        assert_eq!(
            state.markers[0].kind,
            crate::markers::Kind::Create,
            "a birth is recorded as a creation"
        );
        assert_eq!(
            state.markers[0].target, lines[0],
            "the created change is the marker's target"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_forked_on_set_refuses_naming_each_tied_id_and_creates_nothing() {
        let dir = repo("forked");
        let (_base, left, right) = forked_dependencies(&dir);
        let before = listing(&dir);
        let head = op_head(&dir);
        match new(&dir, &["-m", "x", "--on", &left, "--on", &right]) {
            Err(refusal) => {
                assert!(
                    refusal.evidence().contains(&left),
                    "names the left head {left}: {refusal:?}"
                );
                assert!(
                    refusal.evidence().contains(&right),
                    "names the right head {right}: {refusal:?}"
                );
            }
            Ok(lines) => panic!("a forked --on set must refuse, got {lines:?}"),
        }
        assert_eq!(listing(&dir), before, "no file was created");
        assert_eq!(op_head(&dir), head, "no change was created");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_birth_creates_a_missing_nodes_directory() {
        // A write creates the directory it names: with no `nodes/` directory the birth still
        // lands the skeleton at `nodes/<id>.json`, creating the directory rather than refusing.
        let dir = scratch("no-nodes-dir");
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".git/info/exclude"), ".deccy/\n").unwrap();
        fs::create_dir_all(dir.join(".deccy")).unwrap();
        assert!(!dir.join("nodes").exists(), "the workspace has no nodes/");
        let lines = new(&dir, &["-m", "a summary", "--on", "zzzzzzzz"])
            .expect("a birth creates the directory it names");
        let id = lines[0].clone();
        assert!(
            dir.join("nodes").is_dir(),
            "the document's directory is created"
        );
        assert_eq!(
            listing(&dir),
            vec![format!("{id}.json")],
            "the born document is the only one in the directory"
        );
        let _ = load(&dir, &id);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_second_root_creation_creates_the_missing_nodes_directory() {
        // S3-new-position (N1): a `--on` set whose head is the root positions the new change
        // as a sibling of the working copy. The checkout then carries no `nodes/` — the first
        // root node lives in a sibling change — so the birth must create the directory.
        let dir = repo("sibling-root");
        let first = new(&dir, &["-m", "first root", "--on", "zzzzzzzz"])
            .expect("the first root creation lands");
        let first = first[0].clone();
        assert!(
            dir.join("nodes").join(format!("{first}.json")).is_file(),
            "the first root node is on disk before the second birth"
        );
        let second = new(&dir, &["-m", "second root", "--on", "zzzzzzzz"])
            .expect("a sibling root creation creates the directory it names");
        let second = second[0].clone();
        assert!(dir.join("nodes").is_dir(), "the directory is created");
        assert_eq!(
            listing(&dir),
            vec![format!("{second}.json")],
            "the working copy holds the born document"
        );
        let _ = load(&dir, &second);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The repo's change count, for the abandoned-birth assertion.
    fn change_count(dir: &Path) -> usize {
        jj(
            dir,
            &["log", "-r", "all()", "--no-graph", "-T", "change_id ++ \"\\n\""],
        )
        .lines()
        .filter(|line| !line.is_empty())
        .count()
    }

    /// The repo's heads, for the abandoned-birth assertion.
    fn heads(dir: &Path) -> String {
        jj(
            dir,
            &["log", "-r", "heads(all())", "--no-graph", "-T", "change_id ++ \"\\n\""],
        )
    }

    #[test]
    fn a_birth_whose_write_fails_abandons_the_change_it_opened() {
        // S5-birth-fail (kvwonkzy): a birth that cannot complete abandons the change it
        // opened. With nodes/ read-only the skeleton write cannot land, so the birth refuses
        // naming the cause — and the change it opened is gone, leaving no half-born change
        // behind (only a fresh working copy stands in its place).
        let dir = repo("write-fails");
        let deps = dependencies(&dir, 1);
        fs::set_permissions(dir.join("nodes"), fs::Permissions::from_mode(0o555)).unwrap();
        let before = change_count(&dir);
        match new(&dir, &["-m", "a summary", "--on", &deps[0]]) {
            Err(refusal) => assert!(
                !refusal.cause().is_empty(),
                "the refusal carries the write's cause: {refusal:?}"
            ),
            Ok(lines) => panic!("a birth whose write fails must refuse, got {lines:?}"),
        }
        fs::set_permissions(dir.join("nodes"), fs::Permissions::from_mode(0o755)).unwrap();
        let log = jj(&dir, &["log", "--no-graph", "-T", "description"]);
        assert!(
            !log.contains("a summary"),
            "the change the birth opened is gone, got {log:?}"
        );
        assert_eq!(
            change_count(&dir),
            before + 1,
            "the opened change is replaced by a fresh working copy, not left behind"
        );
        assert_eq!(
            heads(&dir).lines().count(),
            1,
            "one head, the fresh working copy"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_refused_birth_records_no_marker() {
        // S5-record-named (wotmvzyt): a birth that refuses leaves nothing to undo, so no marker
        // lands — not even one naming an empty target.
        let dir = repo("refused-no-marker");
        let deps = dependencies(&dir, 1);
        fs::set_permissions(dir.join("nodes"), fs::Permissions::from_mode(0o555)).unwrap();
        match new(&dir, &["-m", "a summary", "--on", &deps[0]]) {
            Err(refusal) => assert!(!refusal.cause().is_empty(), "{refusal:?}"),
            Ok(lines) => panic!("a birth whose write fails must refuse, got {lines:?}"),
        }
        fs::set_permissions(dir.join("nodes"), fs::Permissions::from_mode(0o755)).unwrap();
        let state = markers_persist::load(&Store::new(&dir)).expect("the marker file loads");
        assert!(state.markers.is_empty(), "a refused birth records no marker");
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "nothing was appended, so the file is not even created"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A relation's ids read off a document, sorted: no linkage rule fixes an order, so the
    /// comparison is over the set.
    fn relation(document: &Value, key: &str) -> Vec<String> {
        let mut ids: Vec<String> = document[key]
            .as_array()
            .unwrap_or_else(|| panic!("{key} must be an array, got {document}"))
            .iter()
            .map(|id| id.as_str().expect("an id is a string").to_string())
            .collect();
        ids.sort();
        ids
    }

    /// `ids` sorted, for comparing against a relation.
    fn sorted(ids: &[String]) -> Vec<String> {
        let mut ids = ids.to_vec();
        ids.sort();
        ids
    }

    #[test]
    fn the_help_line_names_the_extended_form() {
        // The verb's own line states the form it accepts, so it carries the linkage flag this
        // layer adds alongside `-m` and `--on`; the line is the verb's own content
        // (xlvoxmyl/rwlywqrk) and answers before any workspace is taken (qtqnuqxo).
        let dir = scratch("help-extended");
        let lines = new(&dir, &["--help"]).expect("help answers on a root-less context");
        assert!(
            lines.iter().any(|line| line.contains("--evaluates")),
            "the help line states the extended form, got {lines:?}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_birth_without_evaluates_has_no_evaluates_key_at_all() {
        // rpppqkvy's unnamed case: a node born without the flag carries no `evaluates` key —
        // not an empty array — and the kind rule leaves it a decision (pzquqmrq).
        let dir = repo("no-evaluates");
        let deps = dependencies(&dir, 1);
        let lines = new(&dir, &["-m", "a summary", "--on", &deps[0]]).expect("a placed birth");
        let document = load(&dir, &lines[0]);
        assert!(
            document.get("evaluates").is_none(),
            "no evaluates key at all, got {document}"
        );
        assert_eq!(document["kind"], "decision");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_evaluates_birth_is_an_evaluation_naming_that_id() {
        // kmrmwopx: `--evaluates` is the verb's own argument, repeatable and optional, and its
        // ids are resolved as what they name, not through `--on`'s path; pzquqmrq puts the born
        // node's kind at evaluation because the invocation names the flag; rpppqkvy fixes that
        // evaluates holds exactly the given ids and that no linkage id reaches depends_on.
        let dir = repo("one-evaluates");
        let deps = dependencies(&dir, 2);
        let lines = new(
            &dir,
            &[
                "-m",
                "a check",
                "--on",
                &deps[1],
                "--evaluates",
                &deps[0],
            ],
        )
        .expect("a birth over the extended form");
        let document = load(&dir, &lines[0]);
        assert_eq!(
            document["kind"], "evaluation",
            "a named --evaluates makes the born node an evaluation"
        );
        assert_eq!(relation(&document, "evaluates"), vec![deps[0].clone()]);
        assert_eq!(
            relation(&document, "depends_on"),
            vec![deps[1].clone()],
            "no id given to --evaluates reaches depends_on"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_given_evaluates_id_is_recorded_and_no_other() {
        let dir = repo("many-evaluates");
        let deps = dependencies(&dir, 3);
        let lines = new(
            &dir,
            &[
                "-m",
                "a three-part check",
                "--on",
                &deps[2],
                "--evaluates",
                &deps[0],
                "--evaluates",
                &deps[1],
            ],
        )
        .expect("a birth over the extended form");
        let document = load(&dir, &lines[0]);
        assert_eq!(
            relation(&document, "evaluates"),
            sorted(&[deps[0].clone(), deps[1].clone()]),
            "every given id and no other"
        );
        assert_eq!(
            relation(&document, "depends_on"),
            vec![deps[2].clone()],
            "depends_on names exactly the --on ids"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_repeated_evaluates_id_is_recorded_once() {
        // rpppqkvy: an id named twice is counted once, so the born document still validates
        // (the schema makes evaluates a set).
        let dir = repo("repeated-evaluates");
        let deps = dependencies(&dir, 2);
        let lines = new(
            &dir,
            &[
                "-m",
                "a repeated check",
                "--on",
                &deps[1],
                "--evaluates",
                &deps[0],
                "--evaluates",
                &deps[0],
            ],
        )
        .expect("a birth over the extended form");
        let document = load(&dir, &lines[0]);
        assert_eq!(
            relation(&document, "evaluates"),
            vec![deps[0].clone()],
            "an id given twice lands once"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_id_given_to_both_flags_lands_in_both_relations() {
        // rpppqkvy: an id also given to `--on` is recorded in both relations; neither spelling
        // drops it from the other.
        let dir = repo("both-flags");
        let deps = dependencies(&dir, 1);
        let lines = new(
            &dir,
            &[
                "-m",
                "a shared id",
                "--on",
                &deps[0],
                "--evaluates",
                &deps[0],
            ],
        )
        .expect("a birth over the extended form");
        let document = load(&dir, &lines[0]);
        assert_eq!(relation(&document, "evaluates"), vec![deps[0].clone()]);
        assert_eq!(relation(&document, "depends_on"), vec![deps[0].clone()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_extended_form_is_recognized_in_any_order() {
        // ronsyqlz: the three flags are recognized in any order, occurrences of the repeated
        // kinds interleaving freely.
        let dir = repo("extended-interleaved");
        let deps = dependencies(&dir, 3);
        let lines = new(
            &dir,
            &[
                "--evaluates",
                &deps[0],
                "-m",
                "interleaved",
                "--on",
                &deps[2],
                "--evaluates",
                &deps[1],
            ],
        )
        .expect("the flags interleave");
        let document = load(&dir, &lines[0]);
        assert_eq!(document["kind"], "evaluation");
        assert_eq!(
            relation(&document, "evaluates"),
            sorted(&[deps[0].clone(), deps[1].clone()])
        );
        assert_eq!(relation(&document, "depends_on"), vec![deps[2].clone()]);
        let summary = jj(
            &dir,
            &[
                "log",
                "-r",
                "@",
                "--no-graph",
                "-T",
                "description.first_line()",
            ],
        );
        assert_eq!(summary, "interleaved", "the summary still lands");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_evaluates_id_may_name_an_evaluation() {
        // kmrmwopx: the flag tells no kind apart — an id naming an evaluation is accepted, with
        // no refusal and no special case.
        let dir = repo("evaluates-an-evaluation");
        let deps = dependencies(&dir, 1);
        jj(&dir, &["new", "-m", "an evaluation"]);
        let check = jj(
            &dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        fs::write(
            dir.join(format!("nodes/{check}.json")),
            r#"{"kind":"evaluation","evaluates":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"a check","rationale":"y"}}"#,
        )
        .unwrap();
        jj(&dir, &["new"]);
        let lines = new(
            &dir,
            &[
                "-m",
                "a check of a check",
                "--on",
                &deps[0],
                "--evaluates",
                &check,
            ],
        )
        .expect("an evaluation target is accepted");
        let document = load(&dir, &lines[0]);
        assert_eq!(document["kind"], "evaluation");
        assert_eq!(relation(&document, "evaluates"), vec![check.clone()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unknown_evaluates_id_refuses_naming_each_and_creates_nothing() {
        // mmrklvqs: document presence is the predicate; usvvozwn: the refusal is the
        // invocation's and precedes every creation step, so the ids that resolved are created
        // no more than the ids that named no document; ulusuxtv: every unknown id named by the
        // spelling given, and no id a document answers for named as unknown.
        let dir = repo("unknown-evaluates");
        let deps = dependencies(&dir, 2);
        let before = listing(&dir);
        let head = op_head(&dir);
        let count = change_count(&dir);
        let heads_before = heads(&dir);
        match new(
            &dir,
            &[
                "-m",
                "x",
                "--on",
                &deps[1],
                "--evaluates",
                &deps[0],
                "--evaluates",
                "abcdefgh",
                "--evaluates",
                "12345678",
            ],
        ) {
            Err(refusal) => {
                assert!(
                    refusal.evidence().contains(&"abcdefgh".to_string()),
                    "the first unknown id is named: {refusal:?}"
                );
                assert!(
                    refusal.evidence().contains(&"12345678".to_string()),
                    "the second unknown id is named: {refusal:?}"
                );
                assert!(
                    !refusal.evidence().contains(&deps[0]),
                    "the id a node document answers for is not named as unknown: {refusal:?}"
                );
            }
            Ok(lines) => panic!("an unknown --evaluates id must refuse, got {lines:?}"),
        }
        assert_eq!(listing(&dir), before, "no node document was written");
        assert_eq!(change_count(&dir), count, "no change was opened");
        assert_eq!(heads(&dir), heads_before, "the heads are as they were");
        assert_eq!(op_head(&dir), head, "no operation was recorded");
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "no marker was recorded"
        );
        assert!(
            !dir.join(".deccy/journal").exists(),
            "no journal entry was recorded"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_root_id_refuses_as_an_unknown_evaluates_id() {
        // mmrklvqs: zzzzzzzz names the root and no node document; the exception that admits it
        // belongs to depends_on, so naming it to --evaluates is an unknown id and is named
        // (ulusuxtv), with nothing created (usvvozwn).
        let dir = repo("root-evaluates");
        let deps = dependencies(&dir, 1);
        let before = listing(&dir);
        let count = change_count(&dir);
        let heads_before = heads(&dir);
        match new(
            &dir,
            &[
                "-m",
                "x",
                "--on",
                &deps[0],
                "--evaluates",
                "zzzzzzzz",
            ],
        ) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"zzzzzzzz".to_string()),
                "the root is named with the unknown ids: {refusal:?}"
            ),
            Ok(lines) => panic!(
                "the root names no node document and must refuse, got {lines:?}"
            ),
        }
        assert_eq!(listing(&dir), before, "no node document was written");
        assert_eq!(change_count(&dir), count, "no change was opened");
        assert_eq!(heads(&dir), heads_before, "the heads are as they were");
        fs::remove_dir_all(&dir).unwrap();
    }
}
