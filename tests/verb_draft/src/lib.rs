//! Gate for M-verb-draft (the `draft` verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root so `crate::` resolves exactly as it does in the binary.
//! The handler is driven over scratch jj repositories made with `jj git init` in the system
//! temp directory, with node documents named by real change ids; nothing here touches the
//! round's own repositories.
//!
//! The fixed surface (the module is `src/verb_draft.rs`, M-route's handler shape):
//!
//! ```text
//! pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal>
//! ```
//!
//! The crate calls only that surface, so any implementation that opens one record `open` per
//! named node, in a workspace of its own at the handle-derived path, reports handle and path,
//! and writes no graph state satisfies it. How the verb reaches the record, the handle, the
//! workspace and the base is the implementation's; the observables are read off `.deccy/work/`,
//! `jj workspace list` and the working copies themselves.
//!
//! The check reads the use of each prerequisite: the surface (one draft per named node, the
//! edit, the report, nothing of the graph), the refusal of a call naming no node, the record
//! (handle key, one node, one edit, the workspace by name and path), the state `open`, the
//! content-identity base over the node's owned set, the workspace's place under the per-user
//! root at the handle's path, the open order's record-last write, the handler's shape, its
//! returned lines and its silence, the argument surface answered before the workspace, the
//! verb's own help, and the flat module / self-contained test layout the shims themselves
//! satisfy. What it does not check is left to the gates that own it: the handle's minting, the
//! record schema, the digest, the jj workspace's name and the path's derivation, the refusal's
//! rendering and exit status, and the per-user root's literal location, which no recorded
//! prerequisite fixes.

#[path = "../../../src/verb_draft.rs"]
mod verb_draft;
#[path = "../../../src/state_root.rs"]
mod state_root;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/draft_handle.rs"]
mod draft_handle;
#[path = "../../../src/draft_identity.rs"]
mod draft_identity;
#[path = "../../../src/draft_registry.rs"]
mod draft_registry;
#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/jj_workspace.rs"]
mod jj_workspace;
#[path = "../../../src/jjlog.rs"]
mod jjlog;
#[path = "../../../src/jjread.rs"]
mod jjread;
#[path = "../../../src/journal.rs"]
mod journal;
#[path = "../../../src/node_load.rs"]
mod node_load;
#[path = "../../../src/node_own.rs"]
mod node_own;
#[path = "../../../src/node_resolve.rs"]
mod node_resolve;
#[path = "../../../src/node_set.rs"]
mod node_set;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/store_bulk.rs"]
mod store_bulk;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/store_read.rs"]
mod store_read;
#[path = "../../../src/store_write.rs"]
mod store_write;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a `Verb` row for `draft` (M-route, ynvwvzrm) compiles.
const DRAFT_ROW: Verb<Context> = Verb {
    name: "draft",
    run: crate::verb_draft::run,
};

#[cfg(test)]
mod gate {
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::Value;

    use crate::context::{self, Context};
    use crate::refusal::Refusal;
    use crate::verb_draft::run;

    /// A fresh directory under the system temp dir, named for the test; leftovers are removed
    /// first.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-draft-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the scratch directory is created");
        dir
    }

    /// Run a real jj in `dir` and hand back its raw output.
    fn jj_run(dir: &Path, args: &[&str]) -> std::process::Output {
        Command::new("jj")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("jj is on PATH for the gate")
    }

    /// Run a real jj in `dir` and return trimmed stdout, failing the test on a bad exit.
    fn jj(dir: &Path, args: &[&str]) -> String {
        let out = jj_run(dir, args);
        assert!(
            out.status.success(),
            "jj {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// A scratch jj repository with a tracked `nodes/` and an untracked `.deccy/work/`. The
    /// state directory is excluded the way `init` records it, so jj never snapshots the
    /// registry.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        jj(&dir, &["git", "init"]);
        fs::write(dir.join(".git/info/exclude"), ".deccy/\n").expect("the exclude is written");
        fs::create_dir_all(dir.join("nodes")).expect("the node directory is created");
        fs::create_dir_all(dir.join(".deccy/work")).expect("the registry is created");
        dir
    }

    /// The shortest change id jj reports for `rev`.
    fn change_id(dir: &Path, rev: &str) -> String {
        jj(
            dir,
            &[
                "log",
                "-r",
                rev,
                "--no-graph",
                "-T",
                "change_id.shortest(8)",
            ],
        )
    }

    /// The full commit id `rev` names, read through jj so any unambiguous id spelling counts.
    fn commit_of(dir: &Path, rev: &str) -> String {
        let out = jj_run(
            dir,
            &["log", "-r", rev, "--no-graph", "-T", "commit_id"],
        );
        assert!(
            out.status.success(),
            "jj could not resolve {rev:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// Describe the working copy as a change carrying a valid node document named by its own
    /// change id, and return that id. The ref is inline, so the node's owned set is the
    /// document alone.
    fn node_change(dir: &Path, message: &str) -> String {
        jj(dir, &["describe", "-m", message]);
        let id = change_id(dir, "@");
        let document = format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"{message}","rationale":"y"}}}}"#
        );
        fs::write(dir.join(format!("nodes/{id}.json")), document).expect("the node document is written");
        id
    }

    /// Describe the working copy as a change carrying a valid node document named by its own
    /// change id, with `ref_path` holding `content` and listed in the document's ref array, and
    /// return that id. The node's owned set is the document and `ref_path`.
    fn node_change_with_ref(dir: &Path, message: &str, ref_path: &str, content: &str) -> String {
        let target = dir.join(ref_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("the ref path's directory is created");
        }
        fs::write(&target, content).expect("the ref file is written");
        jj(dir, &["describe", "-m", message]);
        let id = change_id(dir, "@");
        let document = format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":["{ref_path}"],"rationale":"y"}}}}"#
        );
        fs::write(dir.join(format!("nodes/{id}.json")), document).expect("the node document is written");
        id
    }

    /// The `.deccy/work/<handle>.json` documents, as (filename stem, document) in node order.
    fn records(dir: &Path) -> Vec<(String, Value)> {
        let mut found: Vec<(String, Value)> = fs::read_dir(dir.join(".deccy/work"))
            .expect("the registry directory is readable")
            .map(|entry| {
                let entry = entry.expect("the registry entry is readable");
                let name = entry.file_name().to_string_lossy().into_owned();
                let stem = name
                    .strip_suffix(".json")
                    .unwrap_or(&name)
                    .to_string();
                let text = fs::read_to_string(entry.path()).expect("the record is readable");
                let document: Value = serde_json::from_str(&text)
                    .unwrap_or_else(|error| panic!("the record {name} is JSON: {error}"));
                (stem, document)
            })
            .collect();
        found.sort_by(|left, right| {
            let left_node = left.1["node"].as_str().unwrap_or("");
            let right_node = right.1["node"].as_str().unwrap_or("");
            left_node.cmp(right_node).then(left.0.cmp(&right.0))
        });
        found
    }

    /// The node documents' names and bytes, to prove the call wrote no graph state.
    fn nodes(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        fs::read_dir(dir.join("nodes"))
            .expect("the node directory is readable")
            .map(|entry| {
                let entry = entry.expect("the node entry is readable");
                let name = entry.file_name().to_string_lossy().into_owned();
                let bytes = fs::read(entry.path()).expect("the node document is readable");
                (name, bytes)
            })
            .collect()
    }

    /// The workspaces jj reports, name to root path.
    fn workspaces(dir: &Path) -> BTreeMap<String, String> {
        jj(
            dir,
            &[
                "workspace",
                "list",
                "-T",
                "name ++ \"\\t\" ++ root ++ \"\\n\"",
            ],
        )
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.splitn(2, '\t');
            let name = parts.next().unwrap_or("").to_string();
            let root = parts.next().unwrap_or("").to_string();
            (name, root)
        })
        .collect()
    }

    /// `deccy draft <args>` through the real handler over the scratch repository.
    fn draft(context: &Context, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let owned: Vec<String> = args.iter().map(|argument| argument.to_string()).collect();
        run(context, &owned)
    }

    /// Every field the record must carry, and the working copy the path names.
    fn assert_record(dir: &Path, handle: &str, document: &Value, root: &Path) {
        assert_eq!(
            document["handle"].as_str(),
            Some(handle),
            "the handle is the record's key and its filename"
        );
        assert_eq!(
            document["state"], "open",
            "an opened draft has landed nothing"
        );
        let node = document["node"]
            .as_str()
            .expect("the record names exactly one target node");
        assert!(!node.is_empty(), "the target node is named");

        let workspace = document["workspace"]
            .as_object()
            .expect("the record carries the workspace that holds the draft");
        let name = workspace["name"]
            .as_str()
            .expect("the workspace is named by name");
        assert!(!name.is_empty(), "the workspace has a name");
        let raw = workspace["path"]
            .as_str()
            .expect("the workspace is named by path");
        let path = Path::new(raw);
        assert!(path.is_absolute(), "the workspace path is absolute: {raw}");
        assert!(path.is_dir(), "a working copy exists at {raw}");
        assert_eq!(
            path.file_name(),
            Some(OsStr::new(handle)),
            "the path's last component is the handle: {raw}"
        );
        let canonical =
            fs::canonicalize(path).unwrap_or_else(|error| panic!("{raw} resolves: {error}"));
        assert!(
            !canonical.starts_with(root),
            "the workspace lies outside the repository: {raw}"
        );
        let reported = jj(&canonical, &["root"]);
        assert_eq!(
            fs::canonicalize(&reported).expect("the reported root resolves"),
            canonical,
            "the draft's path is its own working-copy root: {raw}"
        );
        let names = workspaces(dir);
        let listed = names
            .get(name)
            .unwrap_or_else(|| panic!("jj names the workspace {name}: {names:?}"));
        assert_eq!(
            fs::canonicalize(listed).expect("the listed root resolves"),
            canonical,
            "the workspace's name points at the record's path"
        );

        let base = document["base"]
            .as_object()
            .expect("the record carries a content-identity base");
        let owned: BTreeSet<String> = base.keys().cloned().collect();
        let expected: BTreeSet<String> =
            std::iter::once(format!("nodes/{node}.json")).collect();
        assert_eq!(
            owned, expected,
            "the base holds one entry for every path the node's owned set holds"
        );
        for (path, identity) in base {
            let identity = identity
                .as_str()
                .unwrap_or_else(|| panic!("{path}'s base entry is a content identity"));
            assert!(
                !identity.is_empty(),
                "{path}'s base entry is not empty"
            );
        }
    }

    /// Forget and remove the workspaces a test opened, then the repository.
    fn cleanup(dir: &Path, records: &[(String, Value)]) {
        for (_, document) in records {
            if let Some(name) = document["workspace"]["name"].as_str() {
                let _ = jj_run(dir, &["workspace", "forget", name]);
            }
            if let Some(path) = document["workspace"]["path"].as_str() {
                let _ = fs::remove_dir_all(path);
            }
        }
        let _ = fs::remove_dir_all(dir);
    }

    /// A scratch repository with a tracked `nodes/` and an untracked `.deccy/work/`, plus a
    /// second jj workspace beside it created with `jj workspace add` that shares the repository.
    struct Linked {
        base: PathBuf,
        trunk: PathBuf,
        linked: PathBuf,
    }

    /// Make a scratch repository, describe a node document into the trunk's working copy, then
    /// add a linked workspace beside it. The workspace addition snapshots the trunk, so the
    /// node document is part of the node's change before the linked working copy branches off
    /// it; both working copies then see the node. Both trees carry an untracked `.deccy/work/`,
    /// so a record written into either registry is visible to the test.
    fn linked_repo(name: &str, message: &str) -> (Linked, String) {
        let base = scratch(name);
        let trunk = base.join("trunk");
        let linked = base.join("linked");
        fs::create_dir_all(&trunk).expect("the trunk directory is created");
        jj(&trunk, &["git", "init"]);
        fs::write(trunk.join(".git/info/exclude"), ".deccy/\n").expect("the exclude is written");
        fs::create_dir_all(trunk.join("nodes")).expect("the node directory is created");
        fs::create_dir_all(trunk.join(".deccy/work")).expect("the trunk registry is created");
        let node = node_change(&trunk, message);
        let destination = linked.to_str().expect("the linked path is valid UTF-8");
        jj(&trunk, &["workspace", "add", destination, "--name", "linked"]);
        fs::create_dir_all(linked.join(".deccy/work")).expect("the linked registry is created");
        (
            Linked {
                base,
                trunk,
                linked,
            },
            node,
        )
    }

    /// The namespace directory a record's workspace path sits under. The derivation's spelling
    /// is the implementation's; the check compares only which directory two records share.
    fn namespace_of_record(document: &Value) -> PathBuf {
        let path = PathBuf::from(
            document["workspace"]["path"]
                .as_str()
                .expect("the record names its workspace path"),
        );
        path.parent()
            .expect("the workspace path carries the repository's namespace")
            .to_path_buf()
    }

    /// Forget and remove every workspace the drafts opened plus the linked workspace itself,
    /// then the namespace directories the draft paths sat under and the scratch tree.
    fn cleanup_linked(linked: &Linked, records: &[(String, Value)]) {
        let mut namespaces: BTreeSet<PathBuf> = BTreeSet::new();
        for (_, document) in records {
            if let Some(name) = document["workspace"]["name"].as_str() {
                let _ = jj_run(&linked.trunk, &["workspace", "forget", name]);
            }
            if let Some(path) = document["workspace"]["path"].as_str() {
                let path = PathBuf::from(path);
                if let Some(namespace) = path.parent() {
                    namespaces.insert(namespace.to_path_buf());
                }
                let _ = fs::remove_dir_all(&path);
            }
        }
        let _ = jj_run(&linked.trunk, &["workspace", "forget", "linked"]);
        for namespace in namespaces {
            let _ = fs::remove_dir_all(namespace);
        }
        let _ = fs::remove_dir_all(&linked.linked);
        let _ = fs::remove_dir_all(&linked.base);
    }

    /// A directory made unwritable for the guard's lifetime, restored on drop.
    struct ReadOnly {
        path: PathBuf,
        mode: u32,
    }

    impl ReadOnly {
        fn new(path: &Path) -> Self {
            let mode = fs::metadata(path)
                .expect("the directory is readable")
                .permissions()
                .mode();
            let mut permissions = fs::metadata(path)
                .expect("the directory is readable")
                .permissions();
            permissions.set_mode(0o555);
            fs::set_permissions(path, permissions).expect("the directory is made read-only");
            ReadOnly {
                path: path.to_path_buf(),
                mode,
            }
        }
    }

    impl Drop for ReadOnly {
        fn drop(&mut self) {
            if let Ok(metadata) = fs::metadata(&self.path) {
                let mut permissions = metadata.permissions();
                permissions.set_mode(self.mode);
                let _ = fs::set_permissions(&self.path, permissions);
            }
        }
    }

    /// The deepest directory both paths share.
    fn common_ancestor(left: &Path, right: &Path) -> Option<PathBuf> {
        let left = fs::canonicalize(left).ok()?;
        let right = fs::canonicalize(right).ok()?;
        let mut prefix = PathBuf::new();
        let mut shared = None;
        for (left, right) in left.components().zip(right.components()) {
            if left != right {
                break;
            }
            prefix.push(left.as_os_str());
            shared = Some(prefix.clone());
        }
        shared
    }

    #[test]
    fn one_record_and_one_workspace_per_named_node() {
        let dir = repo("per-node");
        let a = node_change(&dir, "node a");
        jj(&dir, &["new", "-m", "between"]);
        let b = node_change(&dir, "node b");
        jj(&dir, &["new", "-m", "the edit"]);
        let edit = change_id(&dir, "@");
        let root = fs::canonicalize(&dir).expect("the repository root resolves");
        let before_nodes = nodes(&dir);
        let before_workspaces = workspaces(&dir);
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        let lines = draft(&context, &[a.as_str(), b.as_str()]).expect("both named nodes open");

        assert_eq!(lines.len(), 2, "one line per draft: its handle and its path");
        let found = records(&dir);
        assert_eq!(found.len(), 2, "one record per named node");
        for (handle, document) in &found {
            assert_record(&dir, handle, document, &root);
            let recorded_edit = document["edit"]
                .as_str()
                .expect("the record names exactly one edit");
            assert_eq!(
                commit_of(&dir, recorded_edit),
                commit_of(&dir, &edit),
                "the edit is the one the invocation is made in"
            );
        }
        let named: BTreeSet<String> = found
            .iter()
            .map(|(_, document)| document["node"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            named,
            [a.clone(), b.clone()].into_iter().collect::<BTreeSet<String>>(),
            "the records name exactly the nodes the call named"
        );

        for (index, node) in [a.as_str(), b.as_str()].iter().enumerate() {
            let (handle, document) = found
                .iter()
                .find(|(_, document)| document["node"].as_str() == Some(*node))
                .expect("the record for the named node");
            let path = document["workspace"]["path"].as_str().unwrap();
            assert!(
                lines[index].contains(handle.as_str()),
                "line {index} names the draft's handle {handle}: {:?}",
                lines[index]
            );
            assert!(
                lines[index].contains(path),
                "line {index} names the workspace path {path}: {:?}",
                lines[index]
            );
        }
        assert_eq!(
            workspaces(&dir).len(),
            before_workspaces.len() + 2,
            "one workspace per draft, and none shared"
        );
        assert_eq!(nodes(&dir), before_nodes, "the graph is unchanged");
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "opening a draft appends no operation marker, so undo and redo get no unit"
        );
        assert_eq!(crate::DRAFT_ROW.name, "draft");

        cleanup(&dir, &found);
    }

    #[test]
    fn a_node_named_twice_yields_two_drafts() {
        let dir = repo("named-twice");
        let a = node_change(&dir, "node a");
        jj(&dir, &["new", "-m", "the edit"]);
        let root = fs::canonicalize(&dir).expect("the repository root resolves");
        let before_nodes = nodes(&dir);
        let before_workspaces = workspaces(&dir);
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        let lines = draft(&context, &[a.as_str(), a.as_str()]).expect("a repeated node opens twice");

        assert_eq!(lines.len(), 2, "two drafts, two lines");
        let found = records(&dir);
        assert_eq!(found.len(), 2, "the model counts drafts, not nodes");
        for (handle, document) in &found {
            assert_eq!(
                document["node"].as_str(),
                Some(a.as_str()),
                "both drafts name the node the call named"
            );
            assert_record(&dir, handle, document, &root);
        }
        let handles: BTreeSet<&String> = found.iter().map(|(handle, _)| handle).collect();
        assert_eq!(handles.len(), 2, "two distinct handles");
        let paths: BTreeSet<&str> = found
            .iter()
            .map(|(_, document)| document["workspace"]["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths.len(), 2, "two distinct workspaces");
        assert_eq!(
            found[0].1["base"], found[1].1["base"],
            "unchanged content gives an equal base"
        );

        let mut reported: BTreeSet<String> = BTreeSet::new();
        for line in &lines {
            let mut matches = 0;
            for (handle, document) in &found {
                let path = document["workspace"]["path"].as_str().unwrap();
                if line.contains(handle.as_str()) && line.contains(path) {
                    matches += 1;
                    reported.insert(handle.clone());
                }
            }
            assert_eq!(matches, 1, "each line names exactly one draft: {line:?}");
        }
        assert_eq!(reported, handles.into_iter().cloned().collect());

        assert_eq!(
            workspaces(&dir).len(),
            before_workspaces.len() + 2,
            "each draft is written in its own workspace"
        );
        assert_eq!(nodes(&dir), before_nodes, "the graph is unchanged");

        cleanup(&dir, &found);
    }

    #[test]
    fn a_call_naming_no_node_refuses_before_anything_is_written() {
        let dir = repo("no-node");
        let _ = node_change(&dir, "node a");
        jj(&dir, &["new", "-m", "the edit"]);
        let edit = change_id(&dir, "@");
        let before_nodes = nodes(&dir);
        let before_workspaces = workspaces(&dir);
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        let cases: Vec<Vec<&str>> = vec![vec![], vec!["--edit", edit.as_str()]];
        for args in cases {
            match draft(&context, &args) {
                Err(refusal) => assert!(
                    !refusal.cause().is_empty(),
                    "the refusal carries a cause"
                ),
                Ok(lines) => panic!("a call naming no node must refuse, got {lines:?}"),
            }
        }

        assert!(
            records(&dir).is_empty(),
            "a refused call wrote no record"
        );
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "a refused call created no workspace"
        );
        assert_eq!(nodes(&dir), before_nodes, "the graph is unchanged");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_edit_flag_is_carried_into_the_record() {
        let dir = repo("edit");
        let a = node_change(&dir, "node a");
        jj(&dir, &["new", "-m", "between"]);
        let b = node_change(&dir, "node b");
        jj(&dir, &["new", "-m", "the edit"]);
        let edit = change_id(&dir, "@");
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        draft(&context, &[b.as_str(), "--edit", a.as_str()])
            .expect("the flag names the edit");
        let first = records(&dir);
        assert_eq!(first.len(), 1);
        let recorded = first[0].1["edit"].as_str().expect("the record names its edit");
        assert_eq!(
            commit_of(&dir, recorded),
            commit_of(&dir, &a),
            "--edit names the edit the draft belongs to"
        );

        draft(&context, &[b.as_str()]).expect("an omitted flag falls back to the working copy");
        let both = records(&dir);
        assert_eq!(both.len(), 2, "a redraft is a second record");
        let defaults = both
            .iter()
            .filter(|(_, document)| {
                let recorded = document["edit"].as_str().expect("the record names its edit");
                commit_of(&dir, recorded) == commit_of(&dir, &edit)
            })
            .count();
        assert_eq!(
            defaults, 1,
            "an omitted --edit is the working copy's current edit"
        );
        let flagged = both
            .iter()
            .filter(|(_, document)| {
                let recorded = document["edit"].as_str().expect("the record names its edit");
                commit_of(&dir, recorded) == commit_of(&dir, &a)
            })
            .count();
        assert_eq!(flagged, 1, "the flagged draft keeps its edit");

        cleanup(&dir, &both);
    }

    #[test]
    fn an_unnamed_argument_refuses_naming_it_before_anything_is_written() {
        let dir = repo("unnamed");
        let a = node_change(&dir, "node a");
        jj(&dir, &["new", "-m", "the edit"]);
        let before_nodes = nodes(&dir);
        let before_workspaces = workspaces(&dir);
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        match draft(&context, &["--frobnicate", a.as_str()]) {
            Err(refusal) => assert!(
                refusal.evidence().contains(&"--frobnicate".to_string()),
                "the refusal names the argument, got {refusal:?}"
            ),
            Ok(lines) => panic!("an unnamed argument must refuse, got {lines:?}"),
        }
        assert!(records(&dir).is_empty(), "no record was written");
        assert_eq!(workspaces(&dir), before_workspaces, "no workspace was created");
        assert_eq!(nodes(&dir), before_nodes, "the graph is unchanged");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_help_line_answers_without_a_workspace() {
        let dir = scratch("help-rootless");
        let context =
            context::resolve(&dir).expect("a directory with no workspace resolves root-less");

        let lines = draft(&context, &["--help"]).expect("help answers on a root-less context");

        assert_eq!(lines.len(), 1, "the verb's own line");
        assert!(!lines[0].trim().is_empty(), "the line is not empty");
        assert!(
            lines[0].contains("draft"),
            "the line names the verb: {:?}",
            lines[0]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_workspace_is_created_before_the_record_is_written() {
        let dir = repo("order");
        let a = node_change(&dir, "node a");
        jj(&dir, &["new", "-m", "the edit"]);
        let context = context::resolve(&dir).expect("the scratch repository resolves");
        draft(&context, &[a.as_str()]).expect("the first draft opens");
        let first = records(&dir);
        assert_eq!(first.len(), 1);
        let first_path = PathBuf::from(
            first[0].1["workspace"]["path"]
                .as_str()
                .expect("the record names its workspace path"),
        );
        let namespace = first_path
            .parent()
            .expect("the path carries the repository's namespace")
            .to_path_buf();

        let other = repo("order-other");
        let o = node_change(&other, "other node");
        jj(&other, &["new", "-m", "the other edit"]);
        let other_context = context::resolve(&other).expect("the other repository resolves");
        draft(&other_context, &[o.as_str()]).expect("the other draft opens");
        let other_records = records(&other);
        assert_eq!(other_records.len(), 1);
        let other_path = PathBuf::from(
            other_records[0].1["workspace"]["path"]
                .as_str()
                .expect("the record names its workspace path"),
        );
        let other_namespace = other_path
            .parent()
            .expect("the path carries the repository's namespace")
            .to_path_buf();

        let root_shared = common_ancestor(&namespace, &other_namespace)
            .expect("both drafts lie under one per-user root");
        assert_ne!(
            namespace, other_namespace,
            "each repository has its own namespace under the root"
        );
        assert_ne!(
            namespace, root_shared,
            "the path is under the repository's namespace, not at the root"
        );
        assert_ne!(
            other_namespace, root_shared,
            "the other path is under its namespace, not at the root"
        );

        let b = node_change(&dir, "node b");
        jj(&dir, &["new", "-m", "the second edit"]);
        let before_nodes = nodes(&dir);
        let outcome = {
            let _guard = ReadOnly::new(&namespace);
            draft(&context::resolve(&dir).expect("the repository resolves"), &[b.as_str()])
        };
        match outcome {
            Err(refusal) => assert!(!refusal.cause().is_empty(), "the refusal carries a cause"),
            Ok(lines) => panic!(
                "an unwritable namespace must refuse before the record is written, got {lines:?}"
            ),
        }
        assert_eq!(
            records(&dir).len(),
            1,
            "no record names a workspace that was never created"
        );
        // Whether a workspace left behind by a failed creation is removed is not fixed by
        // the order rule, so only the record (which must be written last) is asserted.
        assert_eq!(nodes(&dir), before_nodes, "the graph is unchanged");

        cleanup(&dir, &first);
        cleanup(&other, &other_records);
        let _ = fs::remove_dir_all(&namespace);
        let _ = fs::remove_dir_all(&other_namespace);
    }

    /// The draft's workspace starts at the change that owns the named node, not at the change
    /// the invocation is made in: the node's owned file holds one content, a later change holds
    /// another, and the working copy the record names must hold the node's.
    #[test]
    fn a_draft_starts_at_the_node_change_not_a_later_change() {
        let dir = repo("start-revision");
        let a = node_change_with_ref(&dir, "node a", "src/lib.rs", "base");
        jj(&dir, &["new", "-m", "later"]);
        fs::write(dir.join("src/lib.rs"), "later").expect("the later content is written");
        jj(&dir, &["describe", "-m", "later"]);
        jj(&dir, &["new", "-m", "the edit"]);
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        draft(&context, &[a.as_str()]).expect("the named node opens");
        let found = records(&dir);
        assert_eq!(found.len(), 1, "one record for the one named node");
        let document = &found[0].1;
        let path = document["workspace"]["path"]
            .as_str()
            .expect("the record names its workspace path");
        let workspace = Path::new(path);
        assert!(workspace.is_dir(), "the record names a working copy that exists");

        let bytes = fs::read(workspace.join("src/lib.rs"))
            .expect("the workspace holds the node's owned file");
        assert_eq!(
            bytes,
            b"base".to_vec(),
            "the workspace starts at the node's change, not at the later change the edit sits on"
        );

        let base = document["base"]
            .as_object()
            .expect("the record carries a content-identity base");
        let identity = crate::draft_identity::identity(&bytes);
        assert_eq!(
            base.get("src/lib.rs").and_then(Value::as_str),
            Some(identity.as_str()),
            "the working copy's content matches the base entry for the owned file"
        );
        assert_eq!(
            base.keys().cloned().collect::<BTreeSet<String>>(),
            [format!("nodes/{a}.json"), "src/lib.rs".to_string()]
                .into_iter()
                .collect::<BTreeSet<String>>(),
            "the base holds one entry for every path of the node's owned set"
        );
        for (owned, entry) in base {
            let entry = entry.as_str().expect("a base entry is a content identity");
            let held = fs::read(workspace.join(owned))
                .unwrap_or_else(|error| panic!("the workspace holds {owned}: {error}"));
            assert_eq!(
                crate::draft_identity::identity(&held),
                entry,
                "the working copy holds {owned} as the base records it"
            );
        }

        cleanup(&dir, &found);
    }

    /// The same start holds when the invocation's parent is the empty root: the node's change
    /// is not the working copy's parent, and the draft still holds the node's owned set.
    #[test]
    fn a_flat_graph_draft_holds_the_node_owned_set() {
        let dir = repo("flat-graph");
        let a = node_change_with_ref(&dir, "node a", "src/lib.rs", "base");
        jj(&dir, &["new", "zzzzzzzz"]);
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        draft(&context, &[a.as_str()]).expect("the named node opens");
        let found = records(&dir);
        assert_eq!(found.len(), 1, "one record for the one named node");
        let document = &found[0].1;
        let path = document["workspace"]["path"]
            .as_str()
            .expect("the record names its workspace path");
        let workspace = Path::new(path);

        assert!(
            workspace.join(format!("nodes/{a}.json")).is_file(),
            "the workspace holds the node document"
        );
        assert!(
            workspace.join("src/lib.rs").is_file(),
            "the workspace holds the node's owned file"
        );
        assert_eq!(
            fs::read(workspace.join("src/lib.rs")).ok(),
            Some(b"base".to_vec()),
            "the flat graph's workspace starts at the node's change"
        );

        let base = document["base"]
            .as_object()
            .expect("the record carries a content-identity base");
        assert_eq!(
            base.keys().cloned().collect::<BTreeSet<String>>(),
            [format!("nodes/{a}.json"), "src/lib.rs".to_string()]
                .into_iter()
                .collect::<BTreeSet<String>>(),
            "the base holds one entry for every path of the node's owned set"
        );
        for (owned, entry) in base {
            let entry = entry.as_str().expect("a base entry is a content identity");
            let held = fs::read(workspace.join(owned))
                .unwrap_or_else(|error| panic!("the workspace holds {owned}: {error}"));
            assert_eq!(
                crate::draft_identity::identity(&held),
                entry,
                "the working copy holds {owned} as the base records it"
            );
        }

        cleanup(&dir, &found);
    }

    /// An explicit `--edit` names the unit the draft's work belongs to, not the content the
    /// draft starts from: a later change on top of the node's change holds different content,
    /// and the workspace the record names must still hold the node's owned set.
    #[test]
    fn a_draft_with_an_explicit_edit_still_starts_at_the_node_change() {
        let dir = repo("explicit-edit-start");
        let a = node_change_with_ref(&dir, "node a", "src/lib.rs", "base");
        jj(&dir, &["new", "-m", "explicit edit"]);
        fs::write(dir.join("src/lib.rs"), "edited").expect("the edit's content is written");
        jj(&dir, &["describe", "-m", "explicit edit"]);
        let edit = change_id(&dir, "@");
        assert_ne!(
            commit_of(&dir, &edit),
            commit_of(&dir, &a),
            "the explicit edit is a different change from the node's change"
        );
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        draft(&context, &[a.as_str(), "--edit", edit.as_str()])
            .expect("the named node opens with the explicit edit");

        let found = records(&dir);
        assert_eq!(found.len(), 1, "one record for the one named node");
        let document = &found[0].1;
        let recorded = document["edit"].as_str().expect("the record names its edit");
        assert_eq!(
            commit_of(&dir, recorded),
            commit_of(&dir, &edit),
            "the explicit --edit is carried into the record"
        );

        let path = document["workspace"]["path"]
            .as_str()
            .expect("the record names its workspace path");
        let workspace = Path::new(path);
        assert!(workspace.is_dir(), "the record names a working copy that exists");

        let bytes = fs::read(workspace.join("src/lib.rs"))
            .expect("the workspace holds the node's owned file");
        assert_eq!(
            bytes,
            b"base".to_vec(),
            "the workspace starts at the node's change even when --edit names a later change, not at the edit's content"
        );

        let base = document["base"]
            .as_object()
            .expect("the record carries a content-identity base");
        let identity = crate::draft_identity::identity(&bytes);
        assert_eq!(
            base.get("src/lib.rs").and_then(Value::as_str),
            Some(identity.as_str()),
            "the working copy's content matches the base entry for the owned file"
        );
        assert_eq!(
            base.keys().cloned().collect::<BTreeSet<String>>(),
            [format!("nodes/{a}.json"), "src/lib.rs".to_string()]
                .into_iter()
                .collect::<BTreeSet<String>>(),
            "the base holds one entry for every path of the node's owned set"
        );
        for (owned, entry) in base {
            let entry = entry.as_str().expect("a base entry is a content identity");
            let held = fs::read(workspace.join(owned))
                .unwrap_or_else(|error| panic!("the workspace holds {owned}: {error}"));
            assert_eq!(
                crate::draft_identity::identity(&held),
                entry,
                "the working copy holds {owned} as the base records it"
            );
        }

        cleanup(&dir, &found);
    }

    /// Each draft in a multi-node call starts at its own node's change: the working copy at
    /// `@` holds neither node's owned file, and each workspace the records name must hold its
    /// own node's content, in the order the nodes were named and independently of the others.
    #[test]
    fn two_drafts_each_start_at_their_own_node_change() {
        let dir = repo("two-start");
        let a = node_change_with_ref(&dir, "node a", "src/a.txt", "A");
        jj(&dir, &["new", "-m", "between"]);
        let b = node_change_with_ref(&dir, "node b", "src/b.txt", "B");
        // Neither node's change is the invocation's edit: the working copy is an empty change
        // on the root, holding neither owned file.
        jj(&dir, &["new", "zzzzzzzz"]);
        assert_ne!(
            commit_of(&dir, &a),
            commit_of(&dir, &b),
            "the two nodes are owned by different changes"
        );
        let context = context::resolve(&dir).expect("the scratch repository resolves");

        draft(&context, &[a.as_str(), b.as_str()]).expect("both named nodes open");

        let found = records(&dir);
        assert_eq!(found.len(), 2, "one record per named node");
        for (node, owned, content) in [
            (&a, "src/a.txt", b"A".to_vec()),
            (&b, "src/b.txt", b"B".to_vec()),
        ] {
            let document = found
                .iter()
                .find(|(_, document)| document["node"].as_str() == Some(node.as_str()))
                .map(|(_, document)| document)
                .unwrap_or_else(|| panic!("a record names the node {node}"));
            let path = document["workspace"]["path"]
                .as_str()
                .expect("the record names its workspace path");
            let workspace = Path::new(path);
            assert!(
                workspace.join(owned).is_file(),
                "the draft for {node} holds its node's owned file {owned}, whichever draft was opened first"
            );
            let bytes = fs::read(workspace.join(owned))
                .unwrap_or_else(|error| panic!("the draft for {node} reads {owned}: {error}"));
            assert_eq!(
                bytes, content,
                "the draft for {node} starts at its own node's change, not at the invocation's edit"
            );

            let base = document["base"]
                .as_object()
                .expect("the record carries a content-identity base");
            let identity = crate::draft_identity::identity(&bytes);
            assert_eq!(
                base.get(owned).and_then(Value::as_str),
                Some(identity.as_str()),
                "the draft for {node} holds {owned} as its base records it"
            );
            assert_eq!(
                base.keys().cloned().collect::<BTreeSet<String>>(),
                [format!("nodes/{node}.json"), owned.to_string()]
                    .into_iter()
                    .collect::<BTreeSet<String>>(),
                "the base for {node} holds one entry for every path of its owned set"
            );
        }

        cleanup(&dir, &found);
    }

    /// The registry is one for the repository: a draft opened from a linked workspace is filed
    /// in the trunk working copy's `.deccy/work/`, not in the linked workspace's tree, and the
    /// call returns its lines instead of refusing outside the trunk.
    #[test]
    fn a_draft_opened_from_a_linked_workspace_is_filed_in_the_trunk_registry() {
        let (linked, node) = linked_repo("linked-registry", "node a");
        let before_nodes = nodes(&linked.trunk);
        let context = context::resolve(&linked.linked).expect("the linked workspace resolves");

        let lines = match draft(&context, &[node.as_str()]) {
            Ok(lines) => lines,
            Err(refusal) => panic!(
                "draft must keep opening outside the trunk — it writes no graph state — but it refused: {refusal:?}"
            ),
        };

        assert_eq!(lines.len(), 1, "one line for the one draft");
        let in_trunk = records(&linked.trunk);
        assert_eq!(
            in_trunk.len(),
            1,
            "the record is filed in the trunk working copy's .deccy/work, the repository's one registry"
        );
        let (handle, document) = &in_trunk[0];
        assert_eq!(
            document["node"].as_str(),
            Some(node.as_str()),
            "the record names the node the call named"
        );
        assert!(
            linked
                .trunk
                .join(format!(".deccy/work/{handle}.json"))
                .is_file(),
            "the record is the document its handle names under the trunk's registry"
        );
        assert!(
            !linked
                .linked
                .join(format!(".deccy/work/{handle}.json"))
                .exists(),
            "the linked workspace's tree holds no record for the draft"
        );
        assert!(
            records(&linked.linked).is_empty(),
            "the linked workspace's .deccy/work stays empty: the registry is the repository's"
        );
        assert_eq!(nodes(&linked.trunk), before_nodes, "the graph is unchanged");

        cleanup_linked(&linked, &in_trunk);
    }

    /// The draft's path is derived from the repository's namespace, not the invocation
    /// workspace's: the same repository opened from its trunk and from a linked workspace puts
    /// both drafts under one namespace directory.
    #[test]
    fn a_draft_opened_from_a_linked_workspace_takes_the_repository_namespace() {
        let (linked, node) = linked_repo("linked-namespace", "node a");

        let trunk_context = context::resolve(&linked.trunk).expect("the trunk resolves");
        draft(&trunk_context, &[node.as_str()]).expect("the trunk draft opens");
        let first = records(&linked.trunk);
        assert_eq!(first.len(), 1, "one draft opened from the trunk");
        let first_handles: BTreeSet<String> =
            first.iter().map(|(handle, _)| handle.clone()).collect();
        let trunk_document = first[0].1.clone();

        let linked_context =
            context::resolve(&linked.linked).expect("the linked workspace resolves");
        draft(&linked_context, &[node.as_str()]).expect("the linked draft opens");

        let mut after = records(&linked.trunk);
        after.extend(records(&linked.linked));
        assert_eq!(after.len(), 2, "two drafts opened in the one repository");
        let linked_document = after
            .iter()
            .find(|(handle, _)| !first_handles.contains(handle))
            .map(|(_, document)| document)
            .expect("the draft the linked invocation opened");

        assert_eq!(
            namespace_of_record(linked_document),
            namespace_of_record(&trunk_document),
            "a draft opened from a linked workspace takes the repository's namespace, the same one a trunk invocation takes, not a namespace derived from the linked workspace's path"
        );

        cleanup_linked(&linked, &after);
    }

    /// `draft` is not one of the graph-writing verbs the trunk-only door governs: invoked from
    /// a linked workspace it returns its lines, and the refusal stays with the verbs that write
    /// graph state.
    #[test]
    fn a_draft_opens_from_a_linked_workspace_without_refusing() {
        let (linked, node) = linked_repo("linked-opens", "node a");
        let context = context::resolve(&linked.linked).expect("the linked workspace resolves");

        let lines = draft(&context, &[node.as_str()]).unwrap_or_else(|refusal| {
            panic!(
                "the verb opens drafts outside the trunk, so the graph-writing refusal must not reach it: {refusal:?}"
            )
        });
        assert_eq!(lines.len(), 1, "one line for the one draft");

        let mut found = records(&linked.trunk);
        found.extend(records(&linked.linked));
        cleanup_linked(&linked, &found);
    }

    /// The env var that puts this test's binary in the print probe's child mode: `quiet` stops
    /// at the point before the call, `run` makes the call first.
    const PRINT_PROBE: &str = "DECCY_GATE_U7_PRINT_PROBE";

    /// The handler writes nothing itself: it returns its lines for the envelope to write once,
    /// so a handler that also prints them duplicates every line on the terminal.
    ///
    /// A test cannot read its own stdout with `std` alone, so the call is observed from a
    /// child: this same test binary, filtered to this test and run with capture off, twice —
    /// once stopping before the call and once after it — and the two streams must be identical
    /// byte for byte. Anything the handler writes to either stream is exactly the difference.
    /// The probe names no print macro, so a comment that mentions one is not evidence, and it
    /// catches output written anywhere on the call's path, not only in `src/verb_draft.rs`.
    #[test]
    fn the_handler_prints_nothing() {
        if let Some(mode) = std::env::var_os(PRINT_PROBE) {
            if mode.to_str() == Some("run") {
                let dir = PathBuf::from(
                    std::env::var_os("DECCY_GATE_U7_PRINT_REPO")
                        .expect("the probe child is given the scratch repository"),
                );
                let node = std::env::var("DECCY_GATE_U7_PRINT_NODE")
                    .expect("the probe child is given the node to name");
                let context = context::resolve(&dir).expect("the scratch repository resolves");
                draft(&context, &[node.as_str()]).expect("the named node opens in the child");
                use std::io::Write;
                std::io::stdout().flush().expect("the child's stdout flushes");
                std::io::stderr().flush().expect("the child's stderr flushes");
            }
            std::process::exit(0);
        }

        let dir = repo("prints-nothing");
        let a = node_change(&dir, "node a");
        jj(&dir, &["new", "-m", "the edit"]);

        let binary = std::env::current_exe().expect("the gate's own test binary is found");
        let probe = |mode: &str| {
            Command::new(&binary)
                .args(["--exact", "gate::the_handler_prints_nothing", "--nocapture"])
                .env(PRINT_PROBE, mode)
                .env("DECCY_GATE_U7_PRINT_REPO", &dir)
                .env("DECCY_GATE_U7_PRINT_NODE", &a)
                .output()
                .expect("the gate's own test binary is runnable")
        };
        let quiet = probe("quiet");
        let called = probe("run");

        let found = records(&dir);
        cleanup(&dir, &found);

        assert!(
            quiet.status.success(),
            "the probe's quiet pass runs: {}",
            String::from_utf8_lossy(&quiet.stderr)
        );
        assert!(
            called.status.success(),
            "the call completes in the probe child: {}",
            String::from_utf8_lossy(&called.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&called.stdout),
            String::from_utf8_lossy(&quiet.stdout),
            "the handler prints nothing to stdout: it returns its lines for the envelope to write once, so anything on this stream duplicates them"
        );
        assert_eq!(
            String::from_utf8_lossy(&called.stderr),
            String::from_utf8_lossy(&quiet.stderr),
            "the handler prints nothing to stderr either"
        );
    }
}
