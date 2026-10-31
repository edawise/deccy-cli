//! Gate for M-jj-workspace (creating, forgetting and enumerating draft jj workspaces).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the jj seam it
//! crosses with, mirroring the crate root, so `crate::jj` resolves exactly as it does in the
//! binary. The checks drive the surface over scratch jj repositories made with `jj git init` in
//! the system temp directory and over a draft-workspace root outside them; nothing here touches
//! the round's own repositories.
//!
//! The fixed surface (the module is `src/jj_workspace.rs`):
//!
//! ```text
//! pub fn path_for(root: &Path, namespace: &str, handle: &str) -> PathBuf
//! pub fn add(jj: &Jj, path: &Path, name: &str, revision: &str) -> Result<(), WorkspaceError>
//! pub fn forget(jj: &Jj, name: &str) -> Result<(), WorkspaceError>
//! pub fn list(jj: &Jj) -> Result<Vec<String>, WorkspaceError>
//!
//! #[derive(Debug)]
//! pub enum WorkspaceError {
//!     /// jj could not be spawned; the reason is carried (S3-err).
//!     Spawn(String),
//!     /// jj ran and refused; its status and both streams are carried (S3-err).
//!     Refused(Output),
//!     /// The name is not among the workspaces jj reports; the name is carried.
//!     Unknown(String),
//! }
//! ```
//!
//! `path_for` is the derivation: the draft root, then the repository's own namespace under it,
//! then the record's handle — nothing else. `add` takes the destination derived by `path_for`,
//! the jj workspace's own name, which is not the handle, and the start revision the caller
//! names; it prepares the destination's parent so that jj - which requires it - can land a
//! working copy there, and that working copy starts at the named revision rather than at
//! whatever `@` happens to hold. `forget` names a
//! workspace by that jj name and fails when jj does not know it, rather than passing on jj's
//! warning as success. `list` reports the workspace names jj reports. Every crossing goes
//! through the caller-supplied `Jj` (binary and root), and a jj refusal is returned as an error
//! carrying its evidence, never as a silent success.

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/jj_workspace.rs"]
mod jj_workspace;

#[cfg(test)]
mod gate {
    use std::collections::BTreeSet;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::jj::Jj;
    use crate::jj_workspace::{self, WorkspaceError};

    /// A fresh directory under the system temp dir, named for the test; the previous run's
    /// leftovers are removed first.
    fn scratch(kind: &str, name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-jj-workspace-{kind}-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A jj repository of its own (`jj git init` in a scratch directory).
    fn repo(name: &str) -> PathBuf {
        let dir = scratch("repo", name);
        jj_in(&dir, &["git", "init"]);
        dir
    }

    /// Run jj in `dir` as the test's own oracle, and hand back its stdout.
    fn jj_in(dir: &Path, args: &[&str]) -> String {
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
        String::from_utf8(out.stdout).unwrap()
    }

    /// The workspace names jj itself reports: the oracle `list` is compared against. The name
    /// template keeps jj's default line (which also carries a path and a change id) out of it.
    fn jj_names(dir: &Path) -> BTreeSet<String> {
        jj_in(dir, &["workspace", "list", "-T", "name ++ \"\\n\""])
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// What the module's `list` reports, as a set so the order jj happens to print in is not
    /// pinned by the gate.
    fn listed(jj: &Jj) -> BTreeSet<String> {
        jj_workspace::list(jj)
            .expect("list crosses jj and reads its names")
            .into_iter()
            .collect()
    }

    #[test]
    fn the_path_is_the_root_then_the_namespace_then_the_handle() {
        let root = Path::new("/deccy-draft-root");
        let path = jj_workspace::path_for(root, "repo-namespace", "handle01");

        assert_eq!(
            path,
            root.join("repo-namespace").join("handle01"),
            "the derivation is root/namespace/handle"
        );
        let relative: Vec<_> = path
            .strip_prefix(root)
            .expect("the path lies under the root")
            .components()
            .map(|component| component.as_os_str().to_owned())
            .collect();
        assert_eq!(
            relative,
            vec![
                OsString::from("repo-namespace"),
                OsString::from("handle01")
            ],
            "nothing but the namespace and the handle is appended"
        );

        assert_ne!(
            path,
            jj_workspace::path_for(root, "repo-namespace", "handle02"),
            "distinct handles get distinct directories"
        );
        assert_ne!(
            path,
            jj_workspace::path_for(root, "other-namespace", "handle01"),
            "distinct repositories get distinct namespaces"
        );
    }

    #[test]
    fn add_creates_the_working_copy_at_the_derived_path_under_the_draft_root() {
        let repo = repo("add");
        let draft_root = scratch("drafts", "add");
        let path = jj_workspace::path_for(&draft_root, "repo-namespace", "handle01");
        let runner = Jj::new("jj", &repo);

        jj_workspace::add(&runner, &path, "draft-one", "@")
            .expect("add crosses jj and accepts a fresh destination");

        assert!(
            path.is_dir(),
            "a working copy exists at the derived path: {}",
            path.display()
        );
        assert!(
            path.starts_with(&draft_root),
            "it lies under the draft root"
        );
        assert!(
            !path.starts_with(&repo),
            "it lies outside the repository: {}",
            path.display()
        );
        assert!(
            jj_names(&repo).contains("draft-one"),
            "jj names the new workspace: {:?}",
            jj_names(&repo)
        );
        jj_in(&path, &["status"]);
        assert_eq!(
            listed(&runner),
            jj_names(&repo),
            "the module enumerates the same workspaces jj does"
        );

        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&draft_root);
    }

    #[test]
    fn forget_drops_the_name_from_jj_and_leaves_the_others() {
        let repo = repo("forget");
        let draft_root = scratch("drafts", "forget");
        let runner = Jj::new("jj", &repo);
        let one = jj_workspace::path_for(&draft_root, "repo-namespace", "handle01");
        let two = jj_workspace::path_for(&draft_root, "repo-namespace", "handle02");
        jj_workspace::add(&runner, &one, "draft-one", "@").unwrap();
        jj_workspace::add(&runner, &two, "draft-two", "@").unwrap();

        jj_workspace::forget(&runner, "draft-one")
            .expect("forget crosses jj and accepts a name jj knows");

        assert!(
            !jj_names(&repo).contains("draft-one"),
            "jj no longer knows the forgotten name: {:?}",
            jj_names(&repo)
        );
        assert!(
            jj_names(&repo).contains("draft-two"),
            "the other workspace is untouched: {:?}",
            jj_names(&repo)
        );
        assert_eq!(listed(&runner), jj_names(&repo));

        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&draft_root);
    }

    #[test]
    fn list_returns_exactly_the_names_jj_reports() {
        let repo = repo("list");
        let draft_root = scratch("drafts", "list");
        let runner = Jj::new("jj", &repo);

        assert_eq!(
            listed(&runner),
            jj_names(&repo),
            "a fresh repository is enumerated exactly as jj enumerates it"
        );
        assert_eq!(
            listed(&runner).len(),
            jj_names(&repo).len(),
            "no duplicate and no invented name"
        );

        let one = jj_workspace::path_for(&draft_root, "repo-namespace", "handle01");
        let two = jj_workspace::path_for(&draft_root, "repo-namespace", "handle02");
        jj_workspace::add(&runner, &one, "draft-one", "@").unwrap();
        jj_workspace::add(&runner, &two, "draft-two", "@").unwrap();

        let reported = jj_names(&repo);
        assert!(
            reported.contains("draft-one") && reported.contains("draft-two"),
            "jj reports both added workspaces: {reported:?}"
        );
        assert_eq!(
            listed(&runner),
            reported,
            "the enumeration is exactly jj's names, not jj's default line"
        );

        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&draft_root);
    }

    #[test]
    fn forgetting_a_name_jj_does_not_know_is_an_error_and_removes_nothing() {
        let repo = repo("forget-absent");
        let runner = Jj::new("jj", &repo);
        let before = jj_names(&repo);

        match jj_workspace::forget(&runner, "no-such-draft") {
            Err(WorkspaceError::Unknown(name)) => {
                assert_eq!(name, "no-such-draft", "the unknown name is carried")
            }
            Err(other) => panic!(
                "the name is not among jj's workspaces; the name is carried, got {other:?}"
            ),
            Ok(()) => panic!("forgetting an unknown name must not pass silently"),
        }
        assert_eq!(jj_names(&repo), before, "nothing was removed");
        assert_eq!(listed(&runner), before);

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_crossing_that_cannot_spawn_jj_is_an_error_and_creates_nothing() {
        let repo = repo("seam");
        let draft_root = scratch("drafts", "seam");
        let missing = Path::new("/nonexistent/deccy-jj-workspace/jj");
        let runner = Jj::new(missing, &repo);
        let path = jj_workspace::path_for(&draft_root, "repo-namespace", "handle01");

        match jj_workspace::add(&runner, &path, "draft-one", "@") {
            Err(WorkspaceError::Spawn(_)) => {}
            Err(other) => panic!("the spawn failure is carried as evidence, got {other:?}"),
            Ok(()) => panic!("a crossing that cannot spawn jj must not report success"),
        }
        assert!(
            !path.join(".jj").exists(),
            "no working copy exists at the derived path"
        );
        match jj_workspace::forget(&runner, "no-such-draft") {
            Err(WorkspaceError::Spawn(_)) => {}
            other => panic!("forget crosses through the given runner, got {other:?}"),
        }
        match jj_workspace::list(&runner) {
            Err(WorkspaceError::Spawn(_)) => {}
            other => panic!("list crosses through the given runner, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&draft_root);
    }

    /// The workspace starts at the revision `add` is handed: its working copy holds that
    /// revision's tree and its parent is that change, not the change `@` happens to name.
    #[test]
    fn add_lands_the_named_revision() {
        let repo = repo("start-revision");
        let draft_root = scratch("drafts", "start-revision");
        let runner = Jj::new("jj", &repo);

        fs::write(repo.join("marker.txt"), "base").unwrap();
        jj_in(&repo, &["commit", "-m", "base"]);
        let base_change = jj_in(&repo, &["log", "-r", "@-", "--no-graph", "-T", "change_id"])
            .trim()
            .to_string();
        // A later change now holds different content; the workspace must not start from it.
        fs::write(repo.join("marker.txt"), "later").unwrap();
        jj_in(&repo, &["commit", "-m", "later"]);

        let path = jj_workspace::path_for(&draft_root, "repo-namespace", "handle01");
        jj_workspace::add(&runner, &path, "draft-one", &base_change)
            .expect("add crosses jj with the revision it is handed");

        assert_eq!(
            fs::read_to_string(path.join("marker.txt")).unwrap().trim(),
            "base",
            "the new workspace's working copy starts at the named revision"
        );
        let parent = jj_in(&path, &["log", "-r", "@-", "--no-graph", "-T", "change_id"])
            .trim()
            .to_string();
        assert_eq!(
            parent, base_change,
            "the workspace's parent is the named change, not the later change"
        );

        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&draft_root);
    }
}
