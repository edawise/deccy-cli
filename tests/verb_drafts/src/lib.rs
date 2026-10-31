//! Gate for M-V-drafts (the drafts verb).
//!
//! code-to-code: the dependent is the flat module `src/verb_drafts.rs`, a code artifact, and
//! every prerequisite this check leans on is code or a code-shaped artifact — the registry
//! module `src/draft_registry.rs`, the workspace module `src/jj_workspace.rs`, the jj seam,
//! the context, the refusal value and the route table. The check is therefore a test: a
//! self-contained crate per M-tests, pulling in the module under test and the siblings it
//! calls through `#[path]` shims and driving `verb_drafts::run` over scratch jj repositories.
//!
//! What the crate observes is the verb's use of the registry and of jj. Every record under
//! `.deccy/work/` appears once, by handle, with its state and the workspace it names; an open
//! record whose named workspace jj does not have is reported as drifted, while the same
//! absence under a landed record is not drift and changes nothing in its line; an empty
//! registry yields no record lines; the argument surface answers before the workspace is
//! read; and the read-only stance is checked by comparing the registry's bytes and `nodes/`
//! before and after a call, and by confirming the missing workspace is not created.
//!
//! The linked-workspace cases pin the one registry the verb names. A repository has one
//! registry, in the trunk's `.deccy/work/`, and a linked workspace (`jj workspace add`) is
//! another checkout of that repository, not a second registry. `drafts` is a read and not a
//! graph-writing verb, so it must answer from a linked workspace rather than refuse there,
//! listing the trunk's records and never the linked tree's `.deccy/work/`. When the trunk
//! holds no registry directory the scan finds no records, so the listing is empty: the
//! invocation tree is not a fallback registry.
//!
//! The crate pulls the module under test and its siblings in through `#[path]` shims, so it
//! compiles against the repository's real modules and drives them over scratch jj
//! repositories; the linked-workspace cases add a second workspace with real jj.

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/state_root.rs"]
mod state_root;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/draft_registry.rs"]
mod draft_registry;
#[path = "../../../src/jj_workspace.rs"]
mod jj_workspace;
#[path = "../../../src/verb_drafts.rs"]
mod verb_drafts;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a `Verb` row for `drafts` (M-route, ynvwvzrm) compiles,
/// and the row names the one lowercase word the decision settles (nzrnrlky).
const DRAFTS_ROW: Verb<Context> = Verb {
    name: "drafts",
    run: crate::verb_drafts::run,
};

#[cfg(test)]
mod gate {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::{json, Value};

    use crate::context::resolve;
    use crate::refusal::Refusal;
    use crate::verb_drafts::run;

    /// A fresh directory under the system temp dir; the previous run's leftovers are removed
    /// first so a rerun starts clean.
    fn scratch(kind: &str, name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-drafts-{kind}-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A scratch jj repository with a registry directory and a `nodes/` directory. `.deccy/`
    /// stays out of jj's tracking, as it does in a real repository.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch("repo", name);
        jj(&dir, &["git", "init"]);
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git/info/exclude"), ".deccy/\n").unwrap();
        fs::create_dir_all(dir.join(".deccy/work")).unwrap();
        fs::create_dir_all(dir.join("nodes")).unwrap();
        dir
    }

    /// The trunk and a linked workspace added under a scratch root with `jj workspace add`:
    /// the real two-workspace shape of one repository, the main working copy holding
    /// `.deccy/` and the linked one another checkout of the same repository.
    fn linked_workspace(name: &str, workspace_name: &str) -> (PathBuf, PathBuf) {
        let trunk = repo(name);
        let linked = workspace_root(name, "workspaces").join(workspace_name);
        jj(
            &trunk,
            &[
                "workspace",
                "add",
                "--name",
                workspace_name,
                linked.to_str().unwrap(),
            ],
        );
        (trunk, linked)
    }

    /// A directory that is not a jj workspace, for the root-less argument and help checks.
    fn rootless(name: &str) -> PathBuf {
        scratch("rootless", name)
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

    /// A scratch directory for draft workspaces, outside any repository.
    fn workspace_root(kind: &str, name: &str) -> PathBuf {
        scratch(kind, name)
    }

    /// A conforming record of the shape `schemas/work.json` fixes.
    fn record(
        handle: &str,
        state: &str,
        node: &str,
        workspace_name: &str,
        workspace_path: &Path,
    ) -> Value {
        json!({
            "handle": handle,
            "state": state,
            "node": node,
            "edit": "uplqqqrs",
            "workspace": {
                "name": workspace_name,
                "path": workspace_path.to_string_lossy(),
            },
            "base": {
                "src/verb_drafts.rs": "0f1e2d3c",
            },
        })
    }

    /// Store `document` at `.deccy/work/<handle>.json`, the registry's storage shape.
    fn put(repo: &Path, document: &Value) {
        let handle = document["handle"].as_str().expect("the fixture names its handle");
        let text = serde_json::to_vec_pretty(document).expect("the fixture is JSON");
        fs::write(repo.join(".deccy/work").join(format!("{handle}.json")), text)
            .expect("the record is written");
    }

    /// `deccy drafts <args>` through the real handler over the scratch workspace.
    fn run_at(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repository resolves");
        let owned: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        run(&context, &owned)
    }

    /// The one line that names `handle`; the listing gives each record exactly one row.
    fn line<'a>(lines: &'a [String], handle: &str) -> &'a String {
        let mut matches = lines.iter().filter(|line| line.contains(handle));
        let line = matches
            .next()
            .unwrap_or_else(|| panic!("no line names {handle}: {lines:?}"));
        assert!(
            matches.next().is_none(),
            "more than one line names {handle}: {lines:?}"
        );
        line
    }

    /// Every file under `.deccy/work`, by name, with its exact bytes.
    fn registry_bytes(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let root = dir.join(".deccy/work");
        let mut files = BTreeMap::new();
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries {
                let entry = entry.expect("the registry entry is readable");
                if entry.file_type().expect("the entry type is readable").is_file() {
                    files.insert(
                        entry.file_name().to_string_lossy().into_owned(),
                        fs::read(entry.path()).expect("the record is readable"),
                    );
                }
            }
        }
        files
    }

    /// The names under `nodes/`.
    fn node_listing(dir: &Path) -> BTreeSet<String> {
        fs::read_dir(dir.join("nodes"))
            .expect("the node directory is readable")
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn every_record_is_one_line_naming_its_handle_state_and_workspace() {
        let dir = repo("listing");
        let workspaces = workspace_root("listing", "workspaces");
        let open = record(
            "k4mz7qta",
            "open",
            "rroqqrzq",
            "draft-open",
            &workspaces.join("open"),
        );
        let landed = record(
            "uqwnzmtb",
            "landed",
            "rroqqrzq",
            "draft-landed",
            &workspaces.join("landed"),
        );
        put(&dir, &open);
        put(&dir, &landed);

        let lines = run_at(&dir, &[]).expect("a listing over the registry answers");
        assert_eq!(
            lines.len(),
            2,
            "one line per record and nothing outside the scan: {lines:?}"
        );

        for document in [&open, &landed] {
            let handle = document["handle"].as_str().unwrap();
            let state = document["state"].as_str().unwrap();
            let name = document["workspace"]["name"].as_str().unwrap();
            let path = document["workspace"]["path"].as_str().unwrap();
            let line = line(&lines, handle);
            assert!(
                line.contains(state),
                "the line names the record's state: {line:?}"
            );
            assert!(
                line.contains(name),
                "the line names the workspace by name: {line:?}"
            );
            assert!(
                line.contains(path),
                "the line names the workspace by path: {line:?}"
            );
        }

        // Both records name the same node; the listing keeps them apart by handle.
        assert_ne!(line(&lines, "k4mz7qta"), line(&lines, "uqwnzmtb"));
        assert_eq!(crate::DRAFTS_ROW.name, "drafts");
    }

    #[test]
    fn a_linked_workspace_lists_the_trunk_registry_and_does_not_refuse() {
        // One repository has one registry, in the trunk's `.deccy/work/`; a linked workspace
        // is another checkout of that repository, not a second registry. `drafts` is a read
        // and not a graph-writing verb, so it answers from the linked workspace instead of
        // refusing there (mrwtusss confines the non-trunk refusal to graph-writing verbs),
        // and the records it lists are the trunk's.
        let workspace_name = "draft-linked";
        let (trunk, linked) = linked_workspace("linked-listing", workspace_name);
        put(
            &trunk,
            &record("trunkrc7", "open", "rroqqrzq", workspace_name, &linked),
        );

        // The linked workspace starts with no registry of its own.
        assert!(
            !linked.join(".deccy/work").exists(),
            "the linked workspace holds no registry of its own: {}",
            linked.display()
        );

        let lines = run_at(&linked, &[])
            .expect("drafts answers from a linked workspace; it is a read and does not refuse there");
        assert!(
            !lines.is_empty(),
            "the trunk's registry holds a record, so the listing from the linked workspace is not empty: {lines:?}"
        );
        let trunk_line = line(&lines, "trunkrc7");
        assert!(
            trunk_line.contains("open"),
            "the trunk record's line names its state: {trunk_line:?}"
        );
        assert!(
            trunk_line.contains(workspace_name),
            "the trunk record's line names its workspace by name: {trunk_line:?}"
        );
        assert!(
            trunk_line.contains(linked.to_str().unwrap()),
            "the trunk record's line names its workspace by path: {trunk_line:?}"
        );

        // A registry placed in the linked workspace's own tree must change nothing: the
        // linked tree is not the registry.
        fs::create_dir_all(linked.join(".deccy/work")).unwrap();
        put(
            &linked,
            &record(
                "localln2",
                "open",
                "rroqqrzq",
                "draft-local",
                &linked.join("local"),
            ),
        );
        let still = run_at(&linked, &[])
            .expect("drafts answers from the linked workspace after its tree gains a registry");
        assert!(
            still.iter().any(|line| line.contains("trunkrc7")),
            "the trunk record is still listed when the linked tree holds a record: {still:?}"
        );
        assert!(
            !still.iter().any(|line| line.contains("localln2")),
            "the listing reads the trunk's one registry, not the linked workspace's tree: {still:?}"
        );
    }

    #[test]
    fn a_linked_workspace_with_no_trunk_registry_lists_nothing() {
        // A repository has one registry, the trunk's `.deccy/work/`, and the store root names
        // it. When the trunk holds no registry directory the scan finds no records
        // (rroqqrzq: an absent directory holds none), so the listing from a linked workspace
        // is empty — even while the linked workspace's own `.deccy/work/` holds a record. The
        // invocation tree is never read as the registry, and in particular is not a fallback
        // when the trunk's registry directory is absent.
        let workspace_name = "draft-only-linked";
        let (trunk, linked) = linked_workspace("linked-without-registry", workspace_name);

        // The trunk registry is absent: the repository holds no records at all.
        fs::remove_dir_all(trunk.join(".deccy/work")).unwrap();
        assert!(
            !trunk.join(".deccy/work").exists(),
            "the trunk has no registry directory: {}",
            trunk.join(".deccy/work").display()
        );

        // One conforming record, filed only in the linked workspace's own tree.
        fs::create_dir_all(linked.join(".deccy/work")).unwrap();
        put(
            &linked,
            &record("localln9", "open", "rroqqrzq", workspace_name, &linked),
        );

        let lines = run_at(&linked, &[]).expect(
            "drafts answers from a linked workspace; it is a read and does not refuse there",
        );
        assert!(
            lines.is_empty(),
            "the trunk's registry is absent, so the scan finds no records and the listing is empty; the record localln9 is in the invocation tree, which is not the registry: {lines:?}"
        );

        // An empty trunk registry is the same answer: the directory jj and the caller name is
        // the only registry, so a directory that exists and holds nothing still lists nothing.
        fs::create_dir_all(trunk.join(".deccy/work")).unwrap();
        let still = run_at(&linked, &[])
            .expect("drafts answers from the linked workspace with an empty trunk registry");
        assert!(
            still.is_empty(),
            "an empty trunk registry lists no record; the invocation tree is not read as a fallback registry: {still:?}"
        );
    }

    #[test]
    fn an_open_record_whose_workspace_is_gone_is_drift_and_a_landed_one_is_not() {
        let dir = repo("drift");
        let workspaces = workspace_root("drift", "workspaces");
        let open_path = workspaces.join("open");
        let landed_path = workspaces.join("landed");
        put(
            &dir,
            &record("k4mz7qta", "open", "rroqqrzq", "draft-open", &open_path),
        );
        put(
            &dir,
            &record(
                "uqwnzmtb",
                "landed",
                "rroqqrzq",
                "draft-landed",
                &landed_path,
            ),
        );

        let absent = run_at(&dir, &[]).expect("a listing over missing workspaces answers");
        let open_absent = line(&absent, "k4mz7qta").clone();
        let landed_absent = line(&absent, "uqwnzmtb").clone();

        jj(
            &dir,
            &["workspace", "add", "--name", "draft-open", open_path.to_str().unwrap()],
        );
        jj(
            &dir,
            &[
                "workspace",
                "add",
                "--name",
                "draft-landed",
                landed_path.to_str().unwrap(),
            ],
        );

        let present = run_at(&dir, &[]).expect("a listing over present workspaces answers");
        let open_present = line(&present, "k4mz7qta").clone();
        let landed_present = line(&present, "uqwnzmtb").clone();

        assert_ne!(
            open_absent, open_present,
            "an open record whose workspace is missing is reported as drifted, so its line changes when the workspace appears: absent {open_absent:?}, present {open_present:?}"
        );
        assert_eq!(
            landed_absent, landed_present,
            "a landed record whose workspace is missing is not drift, so its absence changes nothing in its line: absent {landed_absent:?}, present {landed_present:?}"
        );

        // Pin the direction without spelling the report: the word the open record gains when
        // its workspace is absent is the drift report (vuopwuyo). It must be the absent line
        // that carries it and the present line that lacks it, and it must never appear on the
        // landed record in either case. Reading the word off the two cases leaves the line's
        // wording open (nzrnrlky).
        let absent_words: BTreeSet<&str> = open_absent.split_whitespace().collect();
        let present_words: BTreeSet<&str> = open_present.split_whitespace().collect();
        let drift_marks: Vec<&str> = absent_words.difference(&present_words).copied().collect();
        assert!(
            !drift_marks.is_empty(),
            "an open record whose workspace is absent reports drift the present case lacks: absent {open_absent:?}, present {open_present:?}"
        );
        for mark in &drift_marks {
            assert!(
                !present_words.contains(mark),
                "the drift report is absent, not present, when the workspace exists: {mark:?} in {open_present:?}"
            );
            for landed in [&landed_absent, &landed_present] {
                assert!(
                    !landed.split_whitespace().any(|word| word == *mark),
                    "a landed record reports no drift even when its workspace is gone: {mark:?} in {landed:?}"
                );
            }
        }

        jj(&dir, &["workspace", "forget", "draft-open"]);
        let forgotten = run_at(&dir, &[]).expect("a listing over a forgotten workspace answers");
        let open_forgotten = line(&forgotten, "k4mz7qta").clone();
        assert_ne!(
            open_forgotten, open_present,
            "a forgotten workspace is as absent as one never created, so the open record is drifted again: {open_forgotten:?} vs {open_present:?}"
        );
        let forgotten_words: BTreeSet<&str> = open_forgotten.split_whitespace().collect();
        for mark in &drift_marks {
            assert!(
                forgotten_words.contains(mark),
                "a forgotten workspace is as absent as one never created, so the same drift report returns: {mark:?} in {open_forgotten:?}"
            );
        }
    }

    #[test]
    fn an_empty_registry_lists_no_record_lines() {
        let dir = repo("empty");
        let lines = run_at(&dir, &[]).expect("an empty registry answers");
        assert!(
            lines.is_empty(),
            "an empty registry yields no record lines: {lines:?}"
        );

        // The scan is the list: a directory the scan does not find holds no records either.
        let absent = scratch("repo", "no-registry");
        jj(&absent, &["git", "init"]);
        let lines = run_at(&absent, &[]).expect("a repository with no registry answers");
        assert!(
            lines.is_empty(),
            "a registry that does not exist holds no records: {lines:?}"
        );
    }

    #[test]
    fn an_argument_is_refused_at_the_surface_before_the_workspace_is_read() {
        // A root-less context makes the ordering observable: a workspace read would refuse
        // for the absent root, with no evidence, so a refusal naming the argument can only
        // come from the argument surface answering first.
        let dir = rootless("args");
        let context = resolve(&dir).expect("a root-less directory resolves");
        let refusal = run(&context, &["x".to_string()])
            .expect_err("the verb declares no arguments in v1, so an argument refuses");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == "x"),
            "the refusal names the argument at the argument surface, not the absent workspace: {refusal:?}"
        );

        // In a repository holding a record, the same refusal leaves the registry untouched.
        let dir = repo("args-in-repo");
        let workspaces = workspace_root("args-in-repo", "workspaces");
        put(
            &dir,
            &record("k4mz7qta", "open", "rroqqrzq", "draft-open", &workspaces.join("open")),
        );
        let before = registry_bytes(&dir);
        let refusal = run_at(&dir, &["x"]).expect_err("an argument refuses in a repository too");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == "x"),
            "{refusal:?}"
        );
        assert_eq!(
            registry_bytes(&dir),
            before,
            "the refusal reads nothing and writes nothing"
        );
    }

    #[test]
    fn one_and_two_arguments_are_both_refused_at_the_surface_before_the_registry_is_read() {
        // The verb declares no arguments in v1 (nzrnrlky, qtqnuqxo), so one argument and two
        // are refused alike, naming a supplied argument. A root-less context keeps the
        // ordering observable: a workspace read would refuse for the absent root without
        // evidence, so a refusal naming a supplied argument can only come from the surface.
        let rootless_dir = rootless("args-many");
        let context = resolve(&rootless_dir).expect("a root-less directory resolves");
        for args in [
            vec!["a".to_string()],
            vec!["a".to_string(), "b".to_string()],
        ] {
            let refusal = run(&context, &args)
                .expect_err("the verb declares no arguments in v1, so every argument refuses");
            assert!(
                refusal.evidence().iter().any(|evidence| args.contains(evidence)),
                "the refusal names a supplied argument at the surface, not the absent workspace: {args:?} -> {refusal:?}"
            );
        }

        // In a repository holding a record, both refusal shapes leave the registry untouched:
        // the surface answers before the registry is read.
        let dir = repo("args-many-in-repo");
        let workspaces = workspace_root("args-many-in-repo", "workspaces");
        put(
            &dir,
            &record("k4mz7qta", "open", "rroqqrzq", "draft-open", &workspaces.join("open")),
        );
        let before = registry_bytes(&dir);
        for args in [
            vec!["a".to_string()],
            vec!["a".to_string(), "b".to_string()],
        ] {
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            let refusal = run_at(&dir, &borrowed)
                .expect_err("an argument refuses in a repository too");
            assert!(
                refusal.evidence().iter().any(|evidence| args.contains(evidence)),
                "{args:?} -> {refusal:?}"
            );
            assert_eq!(
                registry_bytes(&dir),
                before,
                "the refusal reads nothing and writes nothing, for any argument count: {args:?}"
            );
        }
    }

    #[test]
    fn help_answers_without_a_repository() {
        let dir = rootless("help");
        let context = resolve(&dir).expect("a root-less directory resolves");
        let lines = run(&context, &["--help".to_string()])
            .expect("--help answers from a root-less context");
        assert!(
            !lines.is_empty(),
            "--help answers with the verb's own line: {lines:?}"
        );
        assert!(
            lines.join("\n").contains("drafts"),
            "--help names the verb it describes: {lines:?}"
        );
    }

    #[test]
    fn a_listing_changes_nothing() {
        let dir = repo("read-only");
        let workspaces = workspace_root("read-only", "workspaces");
        let drifted_path = workspaces.join("open");
        put(
            &dir,
            &record("k4mz7qta", "open", "rroqqrzq", "draft-open", &drifted_path),
        );
        put(
            &dir,
            &record(
                "uqwnzmtb",
                "landed",
                "rroqqrzq",
                "draft-landed",
                &workspaces.join("landed"),
            ),
        );
        fs::write(
            dir.join("nodes/zzzzzzzz.json"),
            r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"root","rationale":"y"}}"#,
        )
        .unwrap();

        let before_registry = registry_bytes(&dir);
        let before_nodes = node_listing(&dir);

        let lines = run_at(&dir, &[]).expect("the listing answers");
        assert_eq!(lines.len(), 2, "one line per record: {lines:?}");

        assert_eq!(
            registry_bytes(&dir),
            before_registry,
            "a drifted record stays exactly as it stands: still open and still naming its workspace"
        );
        assert_eq!(
            node_listing(&dir),
            before_nodes,
            "the listing reads the registry, not the node space, and changes neither"
        );
        assert!(
            !drifted_path.exists(),
            "the verb repairs nothing: it does not create the missing workspace"
        );
        let kept: Value = serde_json::from_slice(&registry_bytes(&dir)["k4mz7qta.json"])
            .expect("the record is JSON");
        assert_eq!(kept["state"], json!("open"));
        assert_eq!(
            kept["workspace"]["path"],
            json!(drifted_path.to_string_lossy())
        );
    }
}
