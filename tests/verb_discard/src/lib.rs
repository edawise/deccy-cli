//! Gate for M-V-discard (the `discard` verb).
//!
//! code-to-code: the dependent is the flat module `src/verb_discard.rs`, a code artifact, and
//! every prerequisite this check leans on is code or a code-shaped artifact — the registry
//! module, the workspace and abandon seams, the context with its two roots, the refusal value,
//! the route table, the record schema and the layout. The check is therefore a test: a
//! self-contained crate per M-tests, pulling the module under test and the siblings it calls
//! in through `#[path]` shims and driving `verb_discard::run` over scratch jj repositories.
//!
//! The fixed surface (M-route's handler shape):
//!
//! ```text
//! pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal>
//! ```
//!
//! `--help` answers with a line naming `discard`; whether that line comes from a constant, a
//! literal or a table is the verb's own arrangement and not this check's.
//!
//! The crate observes the decision's own observables rather than restating it: the record
//! documents under `.deccy/work/`, the names `jj workspace list` reports, the working copies'
//! change ids and commit ids, the directories under the per-user draft-workspace root, the node
//! documents, the marker file, the handler's returned lines, and the refusal's evidence. Scratch repositories are real
//! `jj git init` repositories with real records and real `jj workspace add` working copies; the
//! per-user draft root is a scratch `HOME`, so nothing here touches the round's repositories.
//!
//! What the crate does not fix is left to the gates that own it: the per-user root's literal
//! location and the namespace's spelling are not fixed by the prerequisites, so the fixtures
//! place and observe the derived directory through the same pieces the decision fixes (a scratch
//! `HOME`, the namespace the repository's canonical path gives and the handle), and the check
//! never takes the removal's subject from the record's stored path; the exact jj invocation
//! behind the forget and the abandonment is the seams'; the refusal's rendering, stream and exit
//! status, and the help text's wording, are not this check's.

#[path = "../../../src/verb_discard.rs"]
mod verb_discard;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/state_root.rs"]
mod state_root;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/jjlog.rs"]
mod jjlog;
#[path = "../../../src/jjread.rs"]
mod jjread;
#[path = "../../../src/journal.rs"]
mod journal;
#[path = "../../../src/draft_handle.rs"]
mod draft_handle;
#[path = "../../../src/draft_identity.rs"]
mod draft_identity;
#[path = "../../../src/draft_registry.rs"]
mod draft_registry;
#[path = "../../../src/jj_workspace.rs"]
mod jj_workspace;
#[path = "../../../src/jj_abandon.rs"]
mod jj_abandon;
#[path = "../../../src/node_load.rs"]
mod node_load;
#[path = "../../../src/node_own.rs"]
mod node_own;
#[path = "../../../src/node_resolve.rs"]
mod node_resolve;
#[path = "../../../src/node_set.rs"]
mod node_set;
#[path = "../../../src/store_bulk.rs"]
mod store_bulk;
#[path = "../../../src/store_read.rs"]
mod store_read;
#[path = "../../../src/store_write.rs"]
mod store_write;
#[path = "../../../src/route.rs"]
mod route;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a `Verb` row for `discard` (M-route, ynvwvzrm) compiles.
const DISCARD_ROW: Verb<Context> = Verb {
    name: "discard",
    run: crate::verb_discard::run,
};

#[cfg(test)]
mod gate {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    use serde_json::{json, Value};

    use crate::context;
    use crate::refusal::Refusal;
    use crate::verb_discard::run;

    /// Six fixed 16-character handles. A handle is any token, so the fixtures name their own.
    const A: &str = "aaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbb";
    const LANDED: &str = "dddddddddddddddd";
    const LANDED_TWO: &str = "eeeeeeeeeeeeeeee";
    const UNKNOWN: &str = "ffffffffffffffff";
    const UNKNOWN_TWO: &str = "0000000000000000";

    /// Serialize the tests: the scratch `HOME` is process-wide, so one test at a time keeps
    /// the environment and the shared root unambiguous.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The env var that puts this test's binary in the print probe's child mode: `quiet` stops
    /// before the call, `run` makes the call first.
    const PRINT_PROBE: &str = "DECCY_GATE_U2_DISCARD_PRINT_PROBE";

    /// A fresh directory under the system temp dir; leftovers are removed first.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-discard-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the scratch directory is created");
        dir
    }

    /// One scratch `HOME` for the whole test binary, created once and shared. The per-user
    /// draft-workspace root lies under it; each repository's canonical path gives its own
    /// namespace, so tests never collide even though `HOME` is process-wide.
    fn home() -> PathBuf {
        static HOME: OnceLock<PathBuf> = OnceLock::new();
        HOME.get_or_init(|| {
            let dir = std::env::temp_dir().join(format!(
                "deccy-verb-discard-home-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("the scratch home is created");
            std::env::set_var("HOME", &dir);
            dir
        })
        .to_path_buf()
    }

    /// The per-user draft-workspace root the fixtures write under.
    fn draft_root(home: &Path) -> PathBuf {
        home.join(".local")
            .join("share")
            .join("deccy")
            .join("workspaces")
    }

    /// The repository's namespace under the per-user root, spelled the way the landed opener
    /// spells it. The check never asserts this spelling; it only places fixtures where a draft
    /// genuinely is.
    fn namespace_of(dir: &Path) -> String {
        let canonical = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        crate::draft_identity::identity(canonical.to_string_lossy().as_bytes())
            .chars()
            .take(16)
            .collect()
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

    /// A scratch jj repository with a tracked `nodes/` and an untracked `.deccy/work/`.
    fn init(dir: &Path) {
        fs::create_dir_all(dir).expect("the repository directory is created");
        jj(dir, &["git", "init"]);
        fs::write(dir.join(".git/info/exclude"), ".deccy/\n").expect("the exclude is written");
        fs::create_dir_all(dir.join("nodes")).expect("the node directory is created");
        fs::create_dir_all(dir.join(".deccy/work")).expect("the registry is created");
    }

    /// A scratch repository named for the test.
    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        init(&dir);
        dir
    }

    /// Describe the working copy as a change carrying a node document named by its own change
    /// id, and return that id.
    fn node_change(dir: &Path, message: &str) -> String {
        jj(dir, &["describe", "-m", message]);
        let id = change_id(dir, "@");
        let document = format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"{message}","rationale":"y"}}}}"#
        );
        fs::write(dir.join("nodes").join(format!("{id}.json")), document)
            .expect("the node document is written");
        id
    }

    /// Start a separate change so the node's change is not the invocation's working copy.
    fn invocation(dir: &Path) {
        jj(dir, &["new", "-m", "the invocation"]);
    }

    /// One fixture draft: its handle, the record's workspace name and path, and — when the
    /// fixture also created the working copy — the working-copy change it holds.
    struct Draft {
        name: String,
        path: PathBuf,
        change: Option<String>,
    }

    /// Write one record at `.deccy/work/<handle>.json`, and, when `create` is set, add the real
    /// `jj workspace add` working copy the record names, started at the node's change.
    fn place(
        dir: &Path,
        home: &Path,
        node: &str,
        handle: &str,
        state: &str,
        create: bool,
    ) -> Draft {
        let name = format!("ws-{handle}");
        let path = draft_root(home).join(namespace_of(dir)).join(handle);
        let mut change = None;
        if create {
            fs::create_dir_all(path.parent().expect("the namespace is a directory"))
                .expect("the namespace directory is created");
            jj(
                dir,
                &[
                    "workspace",
                    "add",
                    path.to_str().expect("the path is valid UTF-8"),
                    "--name",
                    &name,
                    "--revision",
                    node,
                ],
            );
            change = Some(change_id(&path, "@"));
        }
        let document = json!({
            "handle": handle,
            "state": state,
            "node": node,
            "edit": node,
            "workspace": { "name": name, "path": path.to_string_lossy() },
            "base": {},
        });
        crate::draft_registry::write(&dir.join(".deccy/work"), &document)
            .unwrap_or_else(|error| panic!("the fixture record {handle} is stored: {error}"));
        Draft {
            name,
            path,
            change,
        }
    }

    /// Make a draft's derived directory removal fail with not-a-directory: forget the jj
    /// workspace, clear the directory, and put a regular file where the derived directory stood.
    /// The record stays open and still names the workspace, so the draft is drifted and its
    /// directory removal is the first act that cannot complete.
    fn obstruct_derived_directory(dir: &Path, draft: &Draft) {
        jj(
            dir,
            &["workspace", "forget", &draft.name],
        );
        fs::remove_dir_all(&draft.path).expect("the fixture directory is cleared");
        fs::write(&draft.path, b"not a directory").expect("the obstruction is a regular file");
        assert!(
            draft.path.is_file(),
            "the derived path is now a regular file, so removing it as a directory fails"
        );
        assert!(
            !workspaces(dir).contains(&draft.name),
            "jj no longer holds the fixture workspace: the draft is drifted"
        );
    }

    /// Rewrite one stored record after the schema check the registry applies on write.
    fn rewrite_record(dir: &Path, handle: &str, document: &Value) {
        crate::draft_registry::write(&dir.join(".deccy/work"), document)
            .unwrap_or_else(|error| panic!("the fixture record {handle} is rewritten: {error}"));
    }

    /// `deccy discard <args>` through the real handler over the scratch repository.
    fn discard_at(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = context::resolve(dir).expect("the scratch repository resolves");
        let owned: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        run(&context, &owned)
    }

    /// Every record under `.deccy/work/`, by handle, with its exact bytes.
    fn record_files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let root = dir.join(".deccy/work");
        let mut found = BTreeMap::new();
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(_) => return found,
        };
        for entry in entries {
            let entry = entry.expect("the registry entry is readable");
            if !entry.file_type().expect("the entry type is readable").is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let handle = name.strip_suffix(".json").unwrap_or(&name).to_string();
            found.insert(handle, fs::read(entry.path()).expect("the record is readable"));
        }
        found
    }

    /// The record documents, parsed, keyed by handle.
    fn records(dir: &Path) -> BTreeMap<String, Value> {
        record_files(dir)
            .into_iter()
            .map(|(handle, bytes)| {
                let document = serde_json::from_slice(&bytes)
                    .unwrap_or_else(|error| panic!("the record {handle} is JSON: {error}"));
                (handle, document)
            })
            .collect()
    }

    /// The workspace names jj reports.
    fn workspaces(dir: &Path) -> BTreeSet<String> {
        jj(
            dir,
            &["workspace", "list", "-T", "name ++ \"\\n\""],
        )
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
    }

    /// The visible change ids, to observe an abandonment and its absence.
    fn visible_changes(dir: &Path) -> BTreeSet<String> {
        jj(
            dir,
            &[
                "log",
                "-r",
                "all()",
                "--no-graph",
                "-T",
                "change_id.shortest(8) ++ \"\\n\"",
            ],
        )
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
    }

    /// The node documents' names and bytes, to prove the call wrote no graph state.
    fn node_bytes(dir: &Path) -> BTreeMap<String, Vec<u8>> {
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

    /// Forget the fixtures' workspaces and remove their directories, then the repository.
    fn cleanup(dir: &Path, home: &Path, drafts: &[Draft]) {
        for draft in drafts {
            let _ = jj_run(dir, &["workspace", "forget", &draft.name]);
            let _ = fs::remove_dir_all(&draft.path);
        }
        let namespace = draft_root(home).join(namespace_of(dir));
        let _ = fs::remove_dir_all(&namespace);
        let _ = fs::remove_dir_all(dir);
    }

    /// A directory made unwritable for the guard's lifetime, restored on drop.
    struct ReadOnly {
        path: PathBuf,
        mode: u32,
    }

    impl ReadOnly {
        fn new(path: &Path) -> Self {
            let metadata = fs::metadata(path).expect("the directory is readable");
            let mode = metadata.permissions().mode();
            let mut permissions = metadata.permissions();
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

    /// A PATH whose first entry holds a `jj` that delegates every crossing to the real jj
    /// captured before the swap, except the crossing the shell `body` intercepts, which exits
    /// 1 with a distinctive stderr. The original PATH is restored on drop.
    ///
    /// The wrapper makes a seam observe a jj that refuses one specific crossing — a `Refused`
    /// outcome, not the `Unknown` an absent object yields — without changing the real
    /// repository or the module under test. Callers hold `serial()`, so one wrapper is active
    /// at a time.
    struct JjWrapper {
        original: std::ffi::OsString,
        dir: PathBuf,
    }

    impl JjWrapper {
        /// Install a wrapper whose shell `body` runs before the real jj is `exec`ed.
        fn new(name: &str, body: &str) -> Self {
            let original = std::env::var_os("PATH").expect("the gate runs with a PATH");
            let real = std::env::split_paths(&original)
                .map(|dir| dir.join("jj"))
                .find(|candidate| candidate.is_file())
                .expect("the real jj is on the inherited PATH");
            let dir = scratch(name);
            let wrapper = dir.join("jj");
            let script = format!(
                "#!/bin/sh\n{body}\nexec '{}' \"$@\"\n",
                real.to_string_lossy().replace('\'', "'\\''")
            );
            fs::write(&wrapper, script).expect("the failing wrapper is written");
            fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755))
                .expect("the wrapper is executable");
            let path = std::env::join_paths(
                std::iter::once(dir.clone()).chain(std::env::split_paths(&original)),
            )
            .expect("the wrapper directory joins the PATH");
            std::env::set_var("PATH", path);
            JjWrapper { original, dir }
        }

        /// A wrapper that refuses `workspace forget` with a distinctive stderr.
        fn forget_failure(name: &str) -> Self {
            Self::new(
                name,
                "case \"$1 $2\" in\n  \"workspace forget\")\n    printf '%s\\n' 'forced forget failure' >&2\n    exit 1\n    ;;\nesac",
            )
        }

        /// A wrapper that refuses `abandon` with a distinctive stderr.
        fn abandon_failure(name: &str) -> Self {
            Self::new(
                name,
                "if [ \"$1\" = \"abandon\" ]; then\n  printf '%s\\n' 'forced abandon failure' >&2\n  exit 1\nfi",
            )
        }

        /// A wrapper that refuses a `log` reading the workspace's own revision and delegates
        /// every other crossing. That read identifies the change the workspace holds; a
        /// failing read is not the workspace being absent.
        fn log_failure(name: &str, workspace: &str) -> Self {
            let revision = format!("{workspace}@");
            Self::new(
                name,
                &format!(
                    "if [ \"$1\" = \"log\" ]; then\n  for argument in \"$@\"; do\n    if [ \"$argument\" = '{}' ]; then\n      printf '%s\\n' 'forced log failure' >&2\n      exit 1\n    fi\n  done\nfi",
                    revision.replace('\'', "'\\''")
                ),
            )
        }
    }

    impl Drop for JjWrapper {
        fn drop(&mut self) {
            std::env::set_var("PATH", &self.original);
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    /// The three acts on a normal draft, and a success that renders nothing.
    #[test]
    fn one_open_draft_is_discarded_through_its_change_workspace_and_directory() {
        let _serial = serial();
        let home = home();
        let dir = repo("one");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let draft_change = draft.change.clone().expect("the fixture workspace holds a change");
        let node_commit = commit_of(&dir, &node);
        let trunk_change = change_id(&dir, "@");
        let trunk_commit = commit_of(&dir, "@");
        let before_nodes = node_bytes(&dir);
        let before_records = record_files(&dir);
        let before_changes = visible_changes(&dir);
        assert!(draft.path.is_dir(), "the fixture workspace exists");
        assert!(
            workspaces(&dir).contains(&draft.name),
            "jj lists the fixture workspace"
        );
        assert!(
            visible_changes(&dir).contains(&draft_change),
            "the draft's working-copy change is visible before the call"
        );

        let lines = discard_at(&dir, &[A]).expect("the named open draft is discarded");

        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        assert!(
            !record_files(&dir).contains_key(A),
            "the record's document leaves at its handle"
        );
        assert!(
            !draft.path.exists(),
            "the directory the record names is removed"
        );
        assert!(
            !workspaces(&dir).contains(&draft.name),
            "jj no longer holds the draft's workspace"
        );
        assert!(
            !visible_changes(&dir).contains(&draft_change),
            "the working-copy change the workspace held is abandoned"
        );
        assert_eq!(
            commit_of(&dir, &node),
            node_commit,
            "the node's owning change is untouched"
        );
        assert!(
            visible_changes(&dir).contains(&node),
            "the node's owning change is still there"
        );
        let mut expected_changes = before_changes;
        expected_changes.remove(&draft_change);
        assert_eq!(
            visible_changes(&dir),
            expected_changes,
            "the draft's own change is the only change that leaves; the call writes no other change"
        );
        assert_eq!(
            change_id(&dir, "@"),
            trunk_change,
            "the invocation's change is not rewritten"
        );
        assert_eq!(
            commit_of(&dir, "@"),
            trunk_commit,
            "the invocation's working copy is not rewritten"
        );
        assert_eq!(node_bytes(&dir), before_nodes, "the call wrote no node");
        let mut expected = before_records;
        expected.remove(A);
        assert_eq!(
            record_files(&dir),
            expected,
            "no other record changed"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "no operation marker is written, so undo and redo get no unit"
        );
        assert_eq!(crate::DISCARD_ROW.name, "discard");

        cleanup(&dir, &home, &[draft]);
    }

    /// One draft per handle, each with its own acts, and a landed record that stays.
    #[test]
    fn each_handle_discards_one_draft_and_a_landed_record_stays() {
        let _serial = serial();
        let home = home();
        let dir = repo("two");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let first = place(&dir, &home, &node, A, "open", true);
        let second = place(&dir, &home, &node, B, "open", true);
        let landed = place(&dir, &home, &node, LANDED, "landed", false);
        let landed_record = record_files(&dir)
            .remove(LANDED)
            .expect("the landed record is there");
        let first_change = first.change.clone().unwrap();
        let second_change = second.change.clone().unwrap();
        let node_commit = commit_of(&dir, &node);
        let before_changes = visible_changes(&dir);

        let lines = discard_at(&dir, &[A, B]).expect("both named open drafts are discarded");

        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        assert!(!record_files(&dir).contains_key(A));
        assert!(!record_files(&dir).contains_key(B));
        assert!(!first.path.exists(), "the first directory is removed");
        assert!(!second.path.exists(), "the second directory is removed");
        assert!(!workspaces(&dir).contains(&first.name));
        assert!(!workspaces(&dir).contains(&second.name));
        assert!(!visible_changes(&dir).contains(&first_change));
        assert!(!visible_changes(&dir).contains(&second_change));
        assert_eq!(
            record_files(&dir).remove(LANDED),
            Some(landed_record),
            "a landed record is terminal and retained byte for byte"
        );
        assert_eq!(
            commit_of(&dir, &node),
            node_commit,
            "the node's owning change is untouched"
        );
        let mut expected_changes = before_changes;
        expected_changes.remove(&first_change);
        expected_changes.remove(&second_change);
        assert_eq!(
            visible_changes(&dir),
            expected_changes,
            "the two drafts' own changes are the only changes that leave; the call writes no other change"
        );

        cleanup(&dir, &home, &[first, second, landed]);
    }

    /// A handle repeated names the same draft once.
    #[test]
    fn a_repeated_handle_discards_the_draft_once() {
        let _serial = serial();
        let home = home();
        let dir = repo("repeated");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft.change.clone().unwrap();

        let lines = discard_at(&dir, &[A, A]).expect("a repeated handle names one draft");

        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        assert!(!record_files(&dir).contains_key(A));
        assert!(!draft.path.exists());
        assert!(!workspaces(&dir).contains(&draft.name));
        assert!(!visible_changes(&dir).contains(&change));

        cleanup(&dir, &home, &[draft]);
    }

    /// A handle repeated anywhere in the call names the same draft and is discarded once: with
    /// `[A, B, A]` the call succeeds silently and A's and B's records, workspaces, directories
    /// and working-copy changes are each ended exactly once.
    ///
    /// An implementation that removes only adjacent duplicates (`Vec::dedup`) keeps `[A, B, A]`
    /// and processes A, then B, then A again: the second visit finds A's workspace gone and its
    /// directory absent, both tolerated, but its record is already removed, so the registry
    /// removal fails after A's and B's records have been removed — a partial commit presented as
    /// a refusal. This check rejects that refusal.
    ///
    /// The working-copy changes are pinned by bookmarks, as in the absent-directory check, so a
    /// forgotten workspace cannot hide an otherwise unreferenced change: only the abandonment
    /// takes them out of the visible set, and a handler that skips it fails here.
    #[test]
    fn a_non_adjacent_repeated_handle_discards_the_draft_once() {
        let _serial = serial();
        let home = home();
        let dir = repo("repeated-non-adjacent");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let first = place(&dir, &home, &node, A, "open", true);
        let second = place(&dir, &home, &node, B, "open", true);
        let first_change = first
            .change
            .clone()
            .expect("the first fixture workspace holds a change");
        let second_change = second
            .change
            .clone()
            .expect("the second fixture workspace holds a change");
        let node_commit = commit_of(&dir, &node);
        let trunk_change = change_id(&dir, "@");
        let trunk_commit = commit_of(&dir, "@");
        let before_nodes = node_bytes(&dir);
        let mut before_records = record_files(&dir);
        assert_eq!(
            before_records.len(),
            2,
            "the fixture holds exactly the two open records"
        );
        assert!(before_records.remove(A).is_some(), "the first record is there");
        assert!(before_records.remove(B).is_some(), "the second record is there");
        assert!(before_records.is_empty());
        let before_workspaces = workspaces(&dir);
        assert!(before_workspaces.contains(&first.name));
        assert!(before_workspaces.contains(&second.name));
        assert!(first.path.is_dir(), "the first derived directory exists");
        assert!(second.path.is_dir(), "the second derived directory exists");
        // Keep each working-copy change visible through a bookmark: forgetting the workspace
        // alone cannot then hide a change whose abandonment was skipped.
        jj(&dir, &["bookmark", "create", "kept-a", "-r", first_change.as_str()]);
        jj(&dir, &["bookmark", "create", "kept-b", "-r", second_change.as_str()]);
        let before_changes = visible_changes(&dir);
        assert!(
            before_changes.contains(&first_change),
            "the first draft's working-copy change is visible before the call"
        );
        assert!(
            before_changes.contains(&second_change),
            "the second draft's working-copy change is visible before the call"
        );

        let lines = discard_at(&dir, &[A, B, A]).expect(
            "a handle repeated later in the call names the same draft, so the call discards A and B once each and succeeds",
        );

        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        let remaining = record_files(&dir);
        assert!(
            !remaining.contains_key(A),
            "the repeated handle's record leaves at its handle, exactly once"
        );
        assert!(
            !remaining.contains_key(B),
            "the other named draft's record leaves too"
        );
        assert!(
            remaining.is_empty(),
            "the call removes the two named records and no others: {remaining:?}"
        );
        assert!(
            !first.path.exists(),
            "the first draft's derived directory is removed"
        );
        assert!(
            !second.path.exists(),
            "the second draft's derived directory is removed"
        );
        assert!(
            !workspaces(&dir).contains(&first.name),
            "jj no longer holds the first draft's workspace"
        );
        assert!(
            !workspaces(&dir).contains(&second.name),
            "jj no longer holds the second draft's workspace"
        );
        assert!(
            !visible_changes(&dir).contains(&first_change),
            "the first draft's working-copy change is abandoned rather than merely hidden by the forgotten workspace"
        );
        assert!(
            !visible_changes(&dir).contains(&second_change),
            "the second draft's working-copy change is abandoned rather than merely hidden by the forgotten workspace"
        );
        assert_eq!(
            commit_of(&dir, &node),
            node_commit,
            "the node's owning change is untouched by the repeated handle"
        );
        assert!(
            visible_changes(&dir).contains(&node),
            "the node's owning change is still there"
        );
        let mut expected_changes = before_changes;
        expected_changes.remove(&first_change);
        expected_changes.remove(&second_change);
        assert_eq!(
            visible_changes(&dir),
            expected_changes,
            "the two drafts' own changes are the only changes that leave; the repeated call writes no other change"
        );
        assert_eq!(
            change_id(&dir, "@"),
            trunk_change,
            "the invocation's change is not rewritten"
        );
        assert_eq!(
            commit_of(&dir, "@"),
            trunk_commit,
            "the invocation's working copy is not rewritten"
        );
        assert_eq!(
            node_bytes(&dir),
            before_nodes,
            "the call writes no node"
        );
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "a repeated handle opens no operation marker"
        );

        cleanup(&dir, &home, &[first, second]);
    }

    /// A call naming no handle, or an argument the form does not name, refuses on the surface.
    #[test]
    fn a_malformed_call_refuses_at_the_argument_surface() {
        let _serial = serial();
        let home = home();
        let rootless = scratch("rootless-args");
        let context = context::resolve(&rootless).expect("a root-less directory resolves");

        let refusal = run(&context, &[]).expect_err("a call naming no handle refuses");
        assert!(
            !refusal.cause().is_empty(),
            "the refusal carries a cause"
        );
        assert_ne!(
            refusal.cause(),
            "no workspace",
            "the argument surface answers before the store, not the absent workspace: {refusal:?}"
        );

        let refusal = run(&context, &["--frobnicate".to_string()])
            .expect_err("an unnamed argument refuses");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == "--frobnicate"),
            "the refusal names the argument at the surface, not the absent workspace: {refusal:?}"
        );

        let dir = repo("surface");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft.change.clone().unwrap();
        let before_records = record_files(&dir);
        let before_workspaces = workspaces(&dir);
        let before_nodes = node_bytes(&dir);

        for args in [Vec::<&str>::new(), vec!["--frobnicate", A]] {
            let result = discard_at(&dir, &args);
            assert!(result.is_err(), "the malformed call refuses: {args:?}");
        }

        assert_eq!(
            record_files(&dir),
            before_records,
            "a refused call wrote no record"
        );
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "a refused call forgot no workspace"
        );
        assert!(draft.path.is_dir(), "a refused call removed no directory");
        assert!(
            visible_changes(&dir).contains(&change),
            "a refused call abandoned no change"
        );
        assert_eq!(node_bytes(&dir), before_nodes, "the graph is unchanged");

        cleanup(&dir, &home, &[draft]);
        let _ = fs::remove_dir_all(&rootless);
    }

    /// A call carrying a handle that names no open record is refused whole.
    #[test]
    fn an_unknown_handle_refuses_the_whole_call() {
        let _serial = serial();
        let home = home();
        let dir = repo("unknown");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft.change.clone().unwrap();
        let before_records = record_files(&dir);
        let before_workspaces = workspaces(&dir);
        assert!(
            !records(&dir).contains_key(UNKNOWN),
            "the unknown handle names no record"
        );

        let refusal = discard_at(&dir, &[A, UNKNOWN])
            .expect_err("a handle naming no open record refuses the invocation");
        assert!(!refusal.cause().is_empty(), "the refusal carries a cause");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == UNKNOWN),
            "the refusal names the unknown handle among its evidence, so the offending handle is observable: {refusal:?}"
        );
        assert_eq!(
            record_files(&dir),
            before_records,
            "the whole call refuses and no named record leaves"
        );
        assert_eq!(workspaces(&dir), before_workspaces);
        assert!(draft.path.is_dir());
        assert!(visible_changes(&dir).contains(&change));

        let refusal = discard_at(&dir, &[UNKNOWN])
            .expect_err("an unknown handle alone refuses too");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == UNKNOWN),
            "an unknown handle alone is named among the refusal's evidence: {refusal:?}"
        );
        assert_eq!(record_files(&dir), before_records);
        assert!(draft.path.is_dir());
        assert!(visible_changes(&dir).contains(&change));

        cleanup(&dir, &home, &[draft]);
    }

    /// A landed record is not a subject, and its presence refuses the whole call.
    #[test]
    fn a_landed_record_is_never_discarded_and_refuses_the_whole_call() {
        let _serial = serial();
        let home = home();
        let dir = repo("landed");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let open = place(&dir, &home, &node, A, "open", true);
        let landed = place(&dir, &home, &node, LANDED, "landed", false);
        let open_change = open.change.clone().unwrap();
        let landed_record = record_files(&dir).remove(LANDED).unwrap();
        let before_workspaces = workspaces(&dir);

        let refusal = discard_at(&dir, &[A, LANDED])
            .expect_err("a landed handle refuses the invocation");
        assert!(!refusal.cause().is_empty());
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == LANDED),
            "the refusal names the landed handle among its evidence, so the offending handle is observable: {refusal:?}"
        );

        assert_eq!(
            record_files(&dir).remove(LANDED),
            Some(landed_record),
            "the landed record stays at its handle"
        );
        assert!(
            record_files(&dir).contains_key(A),
            "the open record named beside it stays too"
        );
        assert_eq!(workspaces(&dir), before_workspaces);
        assert!(open.path.is_dir());
        assert!(visible_changes(&dir).contains(&open_change));

        let refusal = discard_at(&dir, &[LANDED])
            .expect_err("a landed handle alone refuses");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == LANDED),
            "a landed handle alone is named among the refusal's evidence: {refusal:?}"
        );
        assert!(
            record_files(&dir).contains_key(LANDED),
            "the landed record is still there"
        );

        cleanup(&dir, &home, &[open, landed]);
    }

    /// Two handles that name no open record are both offenders: the refusal names each of them,
    /// and the open draft named beside them is discarded by nothing. A refusal that kept only
    /// one offender — a fresh refusal overwriting the previous one, or the first or last handle
    /// alone — fails one of the two membership assertions.
    #[test]
    fn two_unknown_handles_are_both_named_among_the_refusal_evidence() {
        let _serial = serial();
        let home = home();
        let dir = repo("unknown-pair");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft.change.clone().expect("the fixture workspace holds a change");
        let before_records = record_files(&dir);
        let before_workspaces = workspaces(&dir);
        assert!(
            !records(&dir).contains_key(UNKNOWN),
            "the first unknown handle names no record"
        );
        assert!(
            !records(&dir).contains_key(UNKNOWN_TWO),
            "the second unknown handle names no record"
        );

        let refusal = discard_at(&dir, &[A, UNKNOWN, UNKNOWN_TWO])
            .expect_err("two handles naming no open record refuse the invocation");
        assert!(!refusal.cause().is_empty(), "the refusal carries a cause");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == UNKNOWN),
            "the refusal names the first unknown handle among its evidence: {refusal:?}"
        );
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == UNKNOWN_TWO),
            "the refusal names the second unknown handle too, so neither offender is dropped: {refusal:?}"
        );

        assert_eq!(
            record_files(&dir),
            before_records,
            "the whole call refuses and no named record leaves"
        );
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "the whole call refuses and no workspace is forgotten"
        );
        assert!(draft.path.is_dir(), "the open draft's directory is untouched");
        assert!(
            visible_changes(&dir).contains(&change),
            "the open draft's change is untouched"
        );

        cleanup(&dir, &home, &[draft]);
    }

    /// Two landed records in one call are both offenders: the refusal names both, and the open
    /// draft named beside them keeps its record, workspace, directory and change. A refusal
    /// that kept only one offender fails one of the two membership assertions.
    #[test]
    fn two_landed_handles_are_both_named_and_the_open_draft_is_untouched() {
        let _serial = serial();
        let home = home();
        let dir = repo("landed-pair");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let open = place(&dir, &home, &node, A, "open", true);
        let landed_one = place(&dir, &home, &node, LANDED, "landed", false);
        let landed_two = place(&dir, &home, &node, LANDED_TWO, "landed", false);
        let open_change = open
            .change
            .clone()
            .expect("the fixture workspace holds a change");
        let mut before = record_files(&dir);
        let open_record = before.remove(A).expect("the open record is there");
        let landed_one_record = before
            .remove(LANDED)
            .expect("the first landed record is there");
        let landed_two_record = before
            .remove(LANDED_TWO)
            .expect("the second landed record is there");
        assert!(before.is_empty(), "the fixture holds exactly these three records");
        let before_workspaces = workspaces(&dir);
        let before_nodes = node_bytes(&dir);

        let refusal = discard_at(&dir, &[A, LANDED, LANDED_TWO])
            .expect_err("a landed handle refuses the invocation");
        assert!(!refusal.cause().is_empty(), "the refusal carries a cause");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == LANDED),
            "the refusal names the first landed handle among its evidence: {refusal:?}"
        );
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == LANDED_TWO),
            "the refusal names the second landed handle too, so neither offender is dropped: {refusal:?}"
        );

        let mut after = record_files(&dir);
        assert_eq!(
            after.remove(A),
            Some(open_record),
            "the open draft keeps its record byte for byte"
        );
        assert_eq!(
            after.remove(LANDED),
            Some(landed_one_record),
            "the first landed record stays at its handle byte for byte"
        );
        assert_eq!(
            after.remove(LANDED_TWO),
            Some(landed_two_record),
            "the second landed record stays at its handle byte for byte"
        );
        assert!(after.is_empty(), "the refused call wrote no record");
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "the refused call forgets no workspace"
        );
        assert!(
            open.path.is_dir(),
            "the open draft's directory is untouched"
        );
        assert!(
            visible_changes(&dir).contains(&open_change),
            "the open draft's change is abandoned by nothing"
        );
        assert_eq!(
            node_bytes(&dir),
            before_nodes,
            "the refused call writes no node"
        );

        cleanup(&dir, &home, &[open, landed_one, landed_two]);
    }

    /// One refusal can carry offenders of both kinds: a landed handle and an unknown handle in
    /// the same call are both named, and the open draft beside them is discarded by nothing.
    #[test]
    fn a_mixed_call_names_the_unknown_and_the_landed_handle_and_discards_nothing() {
        let _serial = serial();
        let home = home();
        let dir = repo("mixed");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let open = place(&dir, &home, &node, A, "open", true);
        let landed = place(&dir, &home, &node, LANDED, "landed", false);
        let open_change = open
            .change
            .clone()
            .expect("the fixture workspace holds a change");
        let mut before = record_files(&dir);
        let open_record = before.remove(A).expect("the open record is there");
        let landed_record = before.remove(LANDED).expect("the landed record is there");
        assert!(before.is_empty(), "the fixture holds exactly these two records");
        assert!(
            !records(&dir).contains_key(UNKNOWN),
            "the unknown handle names no record"
        );
        let before_workspaces = workspaces(&dir);
        let before_nodes = node_bytes(&dir);

        let refusal = discard_at(&dir, &[A, LANDED, UNKNOWN])
            .expect_err("an unknown handle beside a landed one refuses the invocation");
        assert!(!refusal.cause().is_empty(), "the refusal carries a cause");
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == UNKNOWN),
            "the refusal names the unknown handle among its evidence: {refusal:?}"
        );
        assert!(
            refusal.evidence().iter().any(|evidence| evidence == LANDED),
            "the refusal names the landed handle too, so neither kind of offender is dropped: {refusal:?}"
        );

        let mut after = record_files(&dir);
        assert_eq!(
            after.remove(A),
            Some(open_record),
            "the open draft keeps its record byte for byte"
        );
        assert_eq!(
            after.remove(LANDED),
            Some(landed_record),
            "the landed record stays at its handle byte for byte"
        );
        assert!(after.is_empty(), "the refused call wrote no record");
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "the refused call forgets no workspace"
        );
        assert!(open.path.is_dir(), "the open draft's directory is untouched");
        assert!(
            visible_changes(&dir).contains(&open_change),
            "the open draft's change is abandoned by nothing"
        );
        assert_eq!(
            node_bytes(&dir),
            before_nodes,
            "the refused call writes no node"
        );

        cleanup(&dir, &home, &[open, landed]);
    }

    /// A reference a record carries is not a key: its node id, its edit id and its workspace
    /// name each name something other than the draft, so a call giving one as a handle refuses
    /// whole, names the token among the refusal's evidence, and leaves the real draft's record,
    /// workspace, directory and change untouched.
    #[test]
    fn a_node_id_an_edit_id_and_a_workspace_name_are_not_handles() {
        let _serial = serial();
        let home = home();
        let dir = repo("references");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft
            .change
            .clone()
            .expect("the fixture workspace holds a change");

        // The record's edit is the change its own workspace holds: a real edit id, distinct
        // from the node id. Rewrite the fixture record so the three references genuinely differ.
        let mut document = crate::draft_registry::read(&dir.join(".deccy/work"), A)
            .expect("the fixture record is readable");
        document["edit"] = json!(change.clone());
        rewrite_record(&dir, A, &document);

        let stored = crate::draft_registry::read(&dir.join(".deccy/work"), A)
            .expect("the rewritten record is readable");
        let node_id = stored["node"]
            .as_str()
            .expect("the record names its node")
            .to_string();
        let edit_id = stored["edit"]
            .as_str()
            .expect("the record names its edit")
            .to_string();
        let workspace_name = stored["workspace"]["name"]
            .as_str()
            .expect("the record names its workspace")
            .to_string();
        assert_ne!(
            node_id, edit_id,
            "the fixture's node and edit references differ"
        );
        assert_ne!(
            workspace_name, node_id,
            "the workspace name differs from the node id"
        );
        assert_ne!(
            workspace_name, edit_id,
            "the workspace name differs from the edit id"
        );
        for token in [&node_id, &edit_id, &workspace_name] {
            assert!(
                !records(&dir).contains_key(token),
                "the reference {token} names no record"
            );
        }
        let before_records = record_files(&dir);
        let before_workspaces = workspaces(&dir);
        let before_nodes = node_bytes(&dir);

        for (kind, token) in [
            ("node id", &node_id),
            ("edit id", &edit_id),
            ("workspace name", &workspace_name),
        ] {
            let refusal = discard_at(&dir, &[A, token.as_str()])
                .expect_err("a reference used as a handle names no open record and refuses");
            assert!(!refusal.cause().is_empty(), "the refusal carries a cause");
            assert!(
                refusal.evidence().iter().any(|evidence| evidence == token),
                "the refusal names the {kind} among its evidence, so the offending token is observable: {refusal:?}"
            );
            assert_eq!(
                record_files(&dir),
                before_records,
                "a call naming the {kind} discards nothing: the open record stays byte for byte"
            );
            assert_eq!(
                workspaces(&dir),
                before_workspaces,
                "a call naming the {kind} forgets no workspace"
            );
            assert!(
                draft.path.is_dir(),
                "a call naming the {kind} removes no directory"
            );
            assert!(
                visible_changes(&dir).contains(&change),
                "a call naming the {kind} abandons no change"
            );
            assert_eq!(
                node_bytes(&dir),
                before_nodes,
                "a call naming the {kind} writes no node"
            );
        }

        cleanup(&dir, &home, &[draft]);
    }

    /// `--help` answers first, from a root-less context.
    #[test]
    fn help_answers_from_a_root_less_context() {
        let _serial = serial();
        let rootless = scratch("help");
        let context = context::resolve(&rootless).expect("a root-less directory resolves");

        let lines = run(&context, &["--help".to_string()])
            .expect("--help answers from a root-less context");

        assert!(
            !lines.is_empty(),
            "--help answers with the verb's own line: {lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.contains("discard")),
            "--help answers with a line naming the verb it describes: {lines:?}"
        );

        let _ = fs::remove_dir_all(&rootless);
    }

    /// A drifted open record is still a subject: it is discarded, not refused, and no change is
    /// substituted for the one its missing workspace no longer holds.
    #[test]
    fn a_drifted_draft_is_discarded_without_refusal_and_abandons_no_change() {
        let _serial = serial();
        let home = home();
        let dir = repo("drift");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", false);
        let node_commit = commit_of(&dir, &node);
        assert!(!draft.path.exists(), "the drifted record's workspace is gone");
        assert!(
            !workspaces(&dir).contains(&draft.name),
            "jj holds no workspace of the record's name"
        );

        let lines = discard_at(&dir, &[A]).expect("a drifted draft is discarded, not refused");

        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        assert!(!record_files(&dir).contains_key(A));
        assert_eq!(
            commit_of(&dir, &node),
            node_commit,
            "abandoning no change leaves the node's owning change exactly as it was"
        );
        assert!(
            visible_changes(&dir).contains(&node),
            "the node's owning change is untouched"
        );
        assert!(!draft.path.exists(), "the absent directory is not created");

        cleanup(&dir, &home, &[draft]);
    }

    /// The workspace's directory is not the workspace's existence. A real open draft whose
    /// derived directory has been taken away is still listed by jj, so its working-copy change
    /// is still reached and abandoned; only a workspace jj no longer lists skips the change
    /// act, and directory absence is not workspace absence.
    ///
    /// A handler that treats an absent derived directory as a gone workspace — guarding the
    /// change act with `path.exists()` — skips the abandonment, goes on to forget the workspace,
    /// tolerates the missing directory and removes the record; the call would then succeed with
    /// the working-copy change still visible, and this check fails on that change.
    ///
    /// The fixture keeps the working-copy change visible through a bookmark, so forgetting the
    /// workspace cannot hide an otherwise unreferenced change: only the change act takes it out
    /// of the visible set, and the check observes the abandonment rather than the forget.
    #[test]
    fn a_listed_workspace_with_an_absent_directory_still_has_its_change_abandoned() {
        let _serial = serial();
        let home = home();
        let dir = repo("absent-directory");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft
            .change
            .clone()
            .expect("the fixture workspace holds a change");
        let node_commit = commit_of(&dir, &node);
        // Keep the working-copy change visible through a bookmark, so abandoning it is
        // observable apart from a forgotten workspace hiding an unreferenced change.
        jj(&dir, &["bookmark", "create", "kept", "-r", change.as_str()]);
        let before_changes = visible_changes(&dir);
        assert!(
            workspaces(&dir).contains(&draft.name),
            "jj lists the fixture workspace before its directory is taken away"
        );
        assert!(
            draft.path.is_dir(),
            "the fixture's derived directory exists before it is taken away"
        );
        assert!(
            visible_changes(&dir).contains(&change),
            "the draft's working-copy change is visible before the call"
        );

        // The derived directory goes away while jj still holds the workspace.
        fs::remove_dir_all(&draft.path).expect("the fixture's derived directory is removed");
        assert!(
            !draft.path.exists(),
            "the derived directory is absent before the call"
        );
        assert!(
            workspaces(&dir).contains(&draft.name),
            "jj still lists the workspace although its directory is absent: directory absence is not workspace absence"
        );
        assert!(
            visible_changes(&dir).contains(&change),
            "the workspace's change is still in the repository before the call"
        );

        let lines = discard_at(&dir, &[A]).expect(
            "a listed workspace whose directory is absent still has its change abandoned and the discard completes",
        );

        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        assert!(
            !visible_changes(&dir).contains(&change),
            "the working-copy change the listed workspace held is abandoned even though its directory is absent"
        );
        assert!(
            !record_files(&dir).contains_key(A),
            "the record leaves at its handle"
        );
        assert!(
            !workspaces(&dir).contains(&draft.name),
            "jj no longer lists the draft's workspace"
        );
        assert!(
            !draft.path.exists(),
            "the absent directory stays absent rather than being recreated"
        );
        assert_eq!(
            commit_of(&dir, &node),
            node_commit,
            "the node's owning change is untouched"
        );
        assert!(
            visible_changes(&dir).contains(&node),
            "the node's owning change is still there"
        );
        let mut expected_changes = before_changes;
        expected_changes.remove(&change);
        assert_eq!(
            visible_changes(&dir),
            expected_changes,
            "only the draft's own change leaves; the call writes no other change"
        );

        cleanup(&dir, &home, &[draft]);
    }

    /// A discard outside the trunk refuses before any draft is discarded, naming the workspace
    /// the invocation ran in.
    #[test]
    fn a_discard_outside_the_trunk_refuses_and_discards_nothing() {
        let _serial = serial();
        let home = home();
        let base = scratch("linked");
        let dir = base.join("trunk");
        init(&dir);
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft.change.clone().unwrap();
        let linked = base.join("linked");
        jj(
            &dir,
            &[
                "workspace",
                "add",
                linked.to_str().expect("the linked path is valid UTF-8"),
                "--name",
                "linked",
            ],
        );
        let linked_context = context::resolve(&linked).expect("the linked workspace resolves");

        let owned = vec![A.to_string()];
        let refusal = run(&linked_context, &owned)
            .expect_err("a discard outside the trunk refuses before discarding any draft");

        let raw = linked.to_string_lossy().to_string();
        let canonical = fs::canonicalize(&linked)
            .expect("the linked root resolves")
            .to_string_lossy()
            .to_string();
        let named = format!("{} {}", refusal.cause(), refusal.evidence().join(" "));
        assert!(
            named.contains(&raw) || named.contains(&canonical),
            "the refusal names the workspace the invocation ran in: {refusal:?}"
        );
        assert!(
            record_files(&dir).contains_key(A),
            "nothing is discarded outside the trunk"
        );
        assert!(workspaces(&dir).contains(&draft.name));
        assert!(draft.path.is_dir());
        assert!(visible_changes(&dir).contains(&change));

        let _ = jj_run(&dir, &["workspace", "forget", "linked"]);
        cleanup(&dir, &home, &[draft]);
        let _ = fs::remove_dir_all(&linked);
        let _ = fs::remove_dir_all(&base);
    }

    /// The registry removal is the commit: a failure before it refuses the invocation and
    /// leaves the record, the completed forget stands with no rollback, and a re-run goes
    /// through the cleared workspace and reaches the commit.
    #[test]
    fn a_failure_before_the_commit_refuses_and_a_rerun_finishes() {
        let _serial = serial();
        let home = home();
        let dir = repo("retry");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let before_records = record_files(&dir);
        let namespace = draft
            .path
            .parent()
            .expect("the path carries the namespace")
            .to_path_buf();
        assert!(
            workspaces(&dir).contains(&draft.name),
            "the fixture workspace is there before the attempt"
        );

        let outcome = {
            let _guard = ReadOnly::new(&namespace);
            discard_at(&dir, &[A])
        };
        let refusal = outcome.expect_err(
            "a directory removal that fails before the commit refuses the invocation",
        );
        assert!(!refusal.cause().is_empty(), "the refusal carries a cause");
        assert_eq!(
            record_files(&dir),
            before_records,
            "the commit is the registry removal, so a failure before it leaves the record at its handle"
        );
        assert!(
            !workspaces(&dir).contains(&draft.name),
            "the completed forget stands: a failure before the commit rolls nothing back"
        );
        assert!(
            draft.path.exists(),
            "the directory removal that failed did not complete"
        );

        let lines = discard_at(&dir, &[A])
            .expect("a re-run goes through the already-cleared workspace to the commit");
        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        assert!(!record_files(&dir).contains_key(A));
        assert!(!draft.path.exists());

        cleanup(&dir, &home, &[draft]);
    }

    /// A failure at the workspace crossing that is not the seam's `Unknown` is a failure before
    /// the registry-removal commit: the invocation refuses, the record stays at its handle, and
    /// the workspace and the derived directory the failed forget would have reached stay
    /// standing. Only an absent workspace is tolerated; a jj that refuses the forget is not an
    /// absence.
    ///
    /// The change act precedes the forget, so the draft's change may already be abandoned when
    /// the forget fails; this check asserts the no-record-removal boundary and does not require
    /// the change to survive.
    #[test]
    fn a_non_unknown_forget_failure_refuses_before_the_commit() {
        let _serial = serial();
        let home = home();
        let dir = repo("forget-failure");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let before_records = record_files(&dir);
        let before_workspaces = workspaces(&dir);
        assert!(
            workspaces(&dir).contains(&draft.name),
            "the fixture workspace is there before the attempt"
        );
        assert!(
            draft.path.is_dir(),
            "the fixture directory is there before the attempt"
        );

        let outcome = {
            let _path = JjWrapper::forget_failure("forget-failure-bin");
            discard_at(&dir, &[A])
        };
        let refusal = outcome.expect_err(
            "a forget jj refuses — not an absent workspace — refuses the invocation",
        );
        let signal = format!("{} {}", refusal.cause(), refusal.evidence().join(" "));
        assert!(
            signal.contains("forced forget failure"),
            "the refusal carries the failed forget's stderr, so a failure that is not an absence is observable: {refusal:?}"
        );

        assert_eq!(
            record_files(&dir),
            before_records,
            "a failure before the commit removes no record: the named record stays at its handle"
        );
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "the forget that failed left the workspace standing"
        );
        assert!(
            draft.path.is_dir(),
            "the failed forget did not reach the directory act, so the derived directory stands"
        );

        cleanup(&dir, &home, &[draft]);
    }

    /// A failure of the abandon crossing is a failure before the commit: the invocation
    /// refuses, carries the crossing's evidence, and leaves the record, the workspace, the
    /// derived directory and the change standing. The change may still exist because the
    /// abandon itself failed; the check does not require it to leave.
    ///
    /// A handler that dropped the abandon status check would go on to forget the workspace,
    /// remove the directory and remove the record, and this check would see a success.
    #[test]
    fn a_jj_abandon_refusal_refuses_before_the_commit() {
        let _serial = serial();
        let home = home();
        let dir = repo("abandon-failure");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft
            .change
            .clone()
            .expect("the fixture workspace holds a change");
        let before_records = record_files(&dir);
        let before_workspaces = workspaces(&dir);
        assert!(
            workspaces(&dir).contains(&draft.name),
            "the fixture workspace is there before the attempt"
        );
        assert!(
            draft.path.is_dir(),
            "the fixture directory is there before the attempt"
        );
        assert!(
            visible_changes(&dir).contains(&change),
            "the draft's working-copy change is visible before the attempt"
        );

        let outcome = {
            let _path = JjWrapper::abandon_failure("abandon-failure-bin");
            discard_at(&dir, &[A])
        };
        let refusal = outcome
            .expect_err("an abandon jj refuses — not an absent change — refuses the invocation");
        let signal = format!("{} {}", refusal.cause(), refusal.evidence().join(" "));
        assert!(
            signal.contains("forced abandon failure"),
            "the refusal carries the failed abandon's stderr, so a failure that is not an absence is observable: {refusal:?}"
        );

        assert_eq!(
            record_files(&dir),
            before_records,
            "a failure before the commit removes no record: the named record stays at its handle"
        );
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "the failed abandon was reached before the forget, so the workspace stands"
        );
        assert!(
            draft.path.is_dir(),
            "the failed abandon was reached before the directory removal, so the derived directory stands"
        );
        assert!(
            visible_changes(&dir).contains(&change),
            "the failed abandon abandoned nothing: the workspace's change still exists"
        );

        cleanup(&dir, &home, &[draft]);
    }

    /// A failing read of the change an existing workspace holds is a failure before the
    /// commit, not the workspace being gone: the invocation refuses and nothing is removed.
    ///
    /// jj still lists the workspace, so its absence — the only case the decision carries
    /// through without an abandonment — is not what happened. A handler that maps any
    /// non-zero read of the workspace's own revision to "gone" skips the change act and goes
    /// on to forget the workspace, remove the directory and remove the record; this check
    /// would see that success and fail.
    #[test]
    fn a_log_failure_for_an_existing_workspace_refuses_instead_of_skipping() {
        let _serial = serial();
        let home = home();
        let dir = repo("log-failure");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft
            .change
            .clone()
            .expect("the fixture workspace holds a change");
        let before_records = record_files(&dir);
        let before_workspaces = workspaces(&dir);
        let before_nodes = node_bytes(&dir);
        assert!(
            workspaces(&dir).contains(&draft.name),
            "jj lists the fixture workspace before the attempt, so the workspace is not gone"
        );
        assert!(
            draft.path.is_dir(),
            "the fixture directory is there before the attempt"
        );
        assert!(
            visible_changes(&dir).contains(&change),
            "the draft's working-copy change is visible before the attempt"
        );

        let outcome = {
            let _path = JjWrapper::log_failure("log-failure-bin", &draft.name);
            discard_at(&dir, &[A])
        };
        let refusal = outcome.expect_err(
            "a failed read of an existing workspace's change is not an absent workspace; the invocation refuses",
        );
        assert!(!refusal.cause().is_empty(), "the refusal carries a cause");

        assert_eq!(
            record_files(&dir),
            before_records,
            "a failure before the commit removes no record: the named record stays at its handle"
        );
        assert_eq!(
            workspaces(&dir),
            before_workspaces,
            "the call refuses before the forget: jj still lists the workspace"
        );
        assert!(
            draft.path.is_dir(),
            "the call refuses before the directory removal: the derived directory stands"
        );
        assert!(
            visible_changes(&dir).contains(&change),
            "no change is abandoned and no change is substituted, so the workspace's change stands"
        );
        assert_eq!(
            node_bytes(&dir),
            before_nodes,
            "the refused call writes no node"
        );

        cleanup(&dir, &home, &[draft]);
    }

    /// The registry removal is the commit for the whole call, not for one draft: every named
    /// draft's change, workspace and directory acts complete before any record leaves the
    /// store. The first draft's acts are allowed to complete, then the second draft's directory
    /// removal is forced to fail with not-a-directory. A per-draft commit would already have
    /// removed the first record before that failure.
    ///
    /// The handles are assumed to be processed in argument order, which the fixtures choose so
    /// that the draft recorded first is also the draft whose acts complete first.
    #[test]
    fn every_named_drafts_acts_complete_before_any_record_is_removed() {
        let _serial = serial();
        let home = home();
        let dir = repo("acts-before-commit");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let first = place(&dir, &home, &node, A, "open", true);
        let second = place(&dir, &home, &node, B, "open", true);
        let first_change = first.change.clone().expect("the first fixture workspace holds a change");
        let node_commit = commit_of(&dir, &node);

        obstruct_derived_directory(&dir, &second);

        let refusal = discard_at(&dir, &[A, B])
            .expect_err("a directory removal that fails refuses the whole invocation");
        assert!(!refusal.cause().is_empty(), "the refusal carries a cause");

        let remaining = record_files(&dir);
        assert!(
            remaining.contains_key(A),
            "the first draft's record stays until every act of every named draft completed; a per-draft commit would have removed it already"
        );
        assert!(
            remaining.contains_key(B),
            "the second draft's record stays too"
        );

        // The first draft's acts completed and stand, with no rollback.
        assert!(
            !visible_changes(&dir).contains(&first_change),
            "the first draft's change was abandoned before the failure"
        );
        assert!(
            !workspaces(&dir).contains(&first.name),
            "the first draft's workspace was forgotten before the failure"
        );
        assert!(
            !first.path.exists(),
            "the first draft's derived directory was removed before the failure"
        );

        // The second draft's obstruction is untouched by the failed removal, and the node's
        // owning change is not a subject of any act.
        assert!(
            second.path.is_file(),
            "the second draft's derived path is still the obstructing file"
        );
        assert_eq!(
            commit_of(&dir, &node),
            node_commit,
            "the node's owning change is untouched"
        );

        cleanup(&dir, &home, &[first, second]);
    }

    /// The removal's subject is the directory the handle derives, not the path the record
    /// stores. The record's `workspace.path` is rewritten to a decoy directory carrying a file;
    /// the derived directory must leave, the decoy and its file must survive, and the record
    /// must leave at its handle.
    #[test]
    fn the_removed_directory_is_the_handle_derivation_not_the_records_stored_path() {
        let _serial = serial();
        let home = home();
        let dir = repo("stored-path");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);
        let change = draft.change.clone().expect("the fixture workspace holds a change");

        let decoy = scratch("stored-path-decoy");
        fs::write(decoy.join("keep.txt"), b"keep").expect("the decoy file is written");
        let mut document = crate::draft_registry::read(&dir.join(".deccy/work"), A)
            .expect("the fixture record is readable");
        document["workspace"]["path"] = json!(decoy.to_string_lossy().into_owned());
        rewrite_record(&dir, A, &document);

        let lines = discard_at(&dir, &[A]).expect("the open draft is discarded");
        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );

        assert!(
            !draft.path.exists(),
            "the directory the handle derives is removed"
        );
        assert!(
            decoy.is_dir(),
            "the decoy directory the record stored is not the removal's subject"
        );
        assert!(
            decoy.join("keep.txt").is_file(),
            "the decoy directory's file survives"
        );
        assert_eq!(
            fs::read(decoy.join("keep.txt")).expect("the decoy file is readable"),
            b"keep",
            "the decoy directory's file is untouched"
        );
        assert!(
            !record_files(&dir).contains_key(A),
            "the record leaves at its handle"
        );
        assert!(
            !workspaces(&dir).contains(&draft.name),
            "the draft's workspace is forgotten"
        );
        assert!(
            !visible_changes(&dir).contains(&change),
            "the draft's change is abandoned"
        );

        let _ = fs::remove_dir_all(&decoy);
        cleanup(&dir, &home, &[draft]);
    }

    /// Only the discarded draft's derived directory is removed: not the namespace directory
    /// above it, not the per-user root, and not a sibling draft's directory or record.
    #[test]
    fn discarding_one_draft_leaves_its_sibling_and_the_namespace() {
        let _serial = serial();
        let home = home();
        let dir = repo("namespace");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let first = place(&dir, &home, &node, A, "open", true);
        let second = place(&dir, &home, &node, B, "open", true);
        let first_change = first.change.clone().expect("the first fixture workspace holds a change");
        let second_change = second.change.clone().expect("the second fixture workspace holds a change");
        let namespace = first
            .path
            .parent()
            .expect("the derived path carries the namespace")
            .to_path_buf();
        let root = draft_root(&home);
        assert_eq!(
            second.path.parent(),
            Some(namespace.as_path()),
            "both drafts share the repository's namespace"
        );

        let lines = discard_at(&dir, &[A]).expect("the named draft is discarded");
        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );

        assert!(
            !record_files(&dir).contains_key(A),
            "the discarded record leaves"
        );
        assert!(
            record_files(&dir).contains_key(B),
            "the sibling's record survives"
        );
        assert!(
            !first.path.exists(),
            "the discarded draft's derived directory is removed"
        );
        assert!(
            second.path.is_dir(),
            "the sibling's derived directory survives"
        );
        assert!(
            workspaces(&dir).contains(&second.name),
            "the sibling's workspace survives"
        );
        assert!(
            visible_changes(&dir).contains(&second_change),
            "the sibling's change survives"
        );
        assert!(
            !visible_changes(&dir).contains(&first_change),
            "the discarded draft's change is abandoned"
        );
        assert!(
            namespace.is_dir(),
            "the namespace directory above the handle survives"
        );
        assert!(root.is_dir(), "the per-user draft-workspace root survives");

        // The last draft leaving the namespace removes only its own directory too: the
        // namespace and the root survive even when no record names a directory in them.
        let lines = discard_at(&dir, &[B]).expect("the sibling is itself a discardable draft");
        assert!(
            lines.is_empty(),
            "a successful discard writes nothing to stdout: {lines:?}"
        );
        assert!(
            !record_files(&dir).contains_key(B),
            "the last record leaves at its handle"
        );
        assert!(
            !second.path.exists(),
            "the last draft's derived directory is removed"
        );
        assert!(
            namespace.is_dir(),
            "the namespace directory survives after the last record leaves"
        );
        assert!(
            root.is_dir(),
            "the per-user draft-workspace root survives after the last record leaves"
        );

        cleanup(&dir, &home, &[first, second]);
    }

    /// The handler writes nothing itself: it returns its lines for the envelope to write once,
    /// so a handler that also prints them duplicates every line on the terminal.
    ///
    /// A test cannot read its own stdout with `std` alone, so the call is observed from a
    /// child: this same test binary, filtered to this test and run with capture off, twice —
    /// once stopping before the call and once after it — and the two streams must be identical
    /// byte for byte. Anything the handler writes to either stream is exactly the difference.
    /// The probe names no print macro, so a comment that mentions one is not evidence, and it
    /// catches output written anywhere on the call's path, not only in `src/verb_discard.rs`.
    #[test]
    fn the_handler_prints_nothing() {
        if let Some(mode) = std::env::var_os(PRINT_PROBE) {
            if mode.to_str() == Some("run") {
                let dir = PathBuf::from(
                    std::env::var_os("DECCY_GATE_U2_DISCARD_PRINT_REPO")
                        .expect("the probe child is given the scratch repository"),
                );
                let handle = std::env::var("DECCY_GATE_U2_DISCARD_PRINT_HANDLE")
                    .expect("the probe child is given the handle to discard");
                let context = context::resolve(&dir).expect("the scratch repository resolves");
                let owned = vec![handle];
                run(&context, &owned).expect("the discard completes in the child");
                use std::io::Write;
                std::io::stdout().flush().expect("the child's stdout flushes");
                std::io::stderr().flush().expect("the child's stderr flushes");
            }
            std::process::exit(0);
        }

        let _serial = serial();
        let home = home();
        let dir = repo("prints-nothing");
        let node = node_change(&dir, "node a");
        invocation(&dir);
        let draft = place(&dir, &home, &node, A, "open", true);

        let binary = std::env::current_exe().expect("the gate's own test binary is found");
        let probe = |mode: &str| {
            Command::new(&binary)
                .args(["--exact", "gate::the_handler_prints_nothing", "--nocapture"])
                .env(PRINT_PROBE, mode)
                .env("DECCY_GATE_U2_DISCARD_PRINT_REPO", &dir)
                .env("DECCY_GATE_U2_DISCARD_PRINT_HANDLE", A)
                .output()
                .expect("the gate's own test binary is runnable")
        };
        let quiet = probe("quiet");
        let called = probe("run");
        cleanup(&dir, &home, &[draft]);

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
