//! Gate for M-store-bulk (the set read: map change ids, batch the owning commits, fall back).
//!
//! A crate of its own: the read pulls in the runner, the store read, and the checking point,
//! which need serde_json and the schema validator. Self-contained per M-tests — the shims pull
//! in the module under test and the siblings it calls; no shared test-support module. The
//! healthy path is driven over a real colocated jj/git repository; the fallbacks are driven
//! through counting shims so neither the crossing count nor the request bytes are guessed.

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/jjread.rs"]
mod jjread;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/store_read.rs"]
mod store_read;
#[path = "../../../src/node_load.rs"]
mod node_load;
#[path = "../../../src/store_bulk.rs"]
mod store_bulk;

#[cfg(test)]
mod gate {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::Value;

    use crate::jj::Jj;
    use crate::node_load::{load, LoadError};
    use crate::store_bulk::read;
    use crate::store_path::Id;

    /// A scratch jj repo with a `nodes/` dir; colocated when git must reach the objects.
    fn scratch(name: &str, colocate: bool) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-bulk-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut command = Command::new("jj");
        command.args(["git", "init"]);
        if colocate {
            command.arg("--colocate");
        }
        let status = command.arg(&dir).status().expect("jj is on PATH for the gate");
        assert!(status.success(), "jj git init must succeed");
        fs::create_dir_all(dir.join("nodes")).unwrap();
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

    fn head(dir: &Path) -> String {
        jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        )
    }

    fn next(dir: &Path) -> String {
        jj(dir, &["new"]);
        head(dir)
    }

    fn write_node(dir: &Path, id: &str, body: &str) {
        fs::write(dir.join("nodes").join(format!("{id}.json")), body).unwrap();
    }

    /// Open a fresh change and write `body` as the node document it owns; return its id.
    fn node(dir: &Path, body: &str) -> String {
        let id = next(dir);
        write_node(dir, &id, body);
        // Snapshot the document into its own change so the change's commit id is stable
        // before the mapping reads it.
        jj(dir, &["status"]);
        id
    }

    fn decision(text: &str) -> String {
        format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"{text}","rationale":"y"}}}}"#
        )
    }

    fn commit(dir: &Path, id: &str) -> String {
        jj(dir, &["log", "-r", id, "--no-graph", "-T", "commit_id"])
    }

    fn parsed(ids: &[String]) -> Vec<Id> {
        ids.iter()
            .map(|id| Id::parse(id).expect("a change id parses"))
            .collect()
    }

    /// The real binary behind a shim, found before any shim is put on PATH.
    fn real(name: &str) -> PathBuf {
        let path = std::env::var_os("PATH").expect("PATH is set");
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
        panic!("{name} must be on PATH");
    }

    /// A scratch directory for shims, outside every fixture so it never enters a working copy.
    fn shim_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-bulk-shims-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn shim(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// A jj shim that logs every call and then runs the real jj.
    fn jj_shim(work: &Path, name: &str) -> (PathBuf, PathBuf) {
        fs::create_dir_all(work).unwrap();
        let bin = work.join(format!("{name}-jj"));
        let log = work.join(format!("{name}-jj.log"));
        shim(
            &bin,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\nexec \"{}\" \"$@\"\n",
                log.display(),
                real("jj").display()
            ),
        );
        (bin, log)
    }

    /// A git shim that logs its calls and its stdin and then runs the real git.
    fn git_shim(work: &Path, name: &str) -> (PathBuf, PathBuf, PathBuf) {
        fs::create_dir_all(work).unwrap();
        let bin = work.join(format!("{name}-git"));
        let calls = work.join(format!("{name}-git.log"));
        let requests = work.join(format!("{name}-git.stdin"));
        shim(
            &bin,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\ntee -a \"{}\" | \"{}\" \"$@\"\n",
                calls.display(),
                requests.display(),
                real("git").display()
            ),
        );
        (bin, calls, requests)
    }

    /// Whether two read outcomes are the same: the same document, or the same failure shape
    /// with the same carried reason.
    fn same(got: &Result<Value, LoadError>, want: &Result<Value, LoadError>) -> bool {
        match (got, want) {
            (Ok(got), Ok(want)) => got == want,
            (Err(LoadError::Missing), Err(LoadError::Missing)) => true,
            (Err(LoadError::Unreadable(got)), Err(LoadError::Unreadable(want))) => got == want,
            (Err(LoadError::NotJson(got)), Err(LoadError::NotJson(want))) => got == want,
            (Err(LoadError::Invalid(got)), Err(LoadError::Invalid(want))) => got == want,
            (Err(LoadError::Schema(got)), Err(LoadError::Schema(want))) => got == want,
            _ => false,
        }
    }

    fn matches_the_single_document_read(dir: &Path, results: &BTreeMap<String, Result<Value, LoadError>>, ids: &[String]) {
        let reference = Jj::new("jj", dir);
        for id in ids {
            let expected = load(&reference, &Id::parse(id).unwrap());
            assert!(
                same(&results[id], &expected),
                "{id}: batch result {:?} does not match the single-document read {:?}",
                results[id],
                expected
            );
        }
    }

    #[test]
    fn the_healthy_set_crosses_once_and_matches_the_single_document_read() {
        let r = scratch("healthy", true);
        let a = node(&r, &decision("a"));
        let b = node(&r, &decision("b"));
        let c = node(&r, &decision("c"));
        let ids = vec![a.clone(), b.clone(), c.clone()];
        let work = shim_dir("healthy");
        let (jj_bin, jj_log) = jj_shim(&work, "healthy");
        let (git_bin, git_calls, git_requests) = git_shim(&work, "healthy");

        let results = read(&Jj::new(jj_bin.as_path(), r.as_path()), Some(&Jj::new(git_bin.as_path(), r.as_path())), &parsed(&ids));
        matches_the_single_document_read(&r, &results, &ids);

        let calls = fs::read_to_string(&jj_log).unwrap_or_default();
        let mappings = calls.lines().filter(|line| line.starts_with("log ")).count();
        assert_eq!(mappings, 1, "one mapping crossing: {calls:?}");
        assert!(
            calls.lines().all(|line| !line.starts_with("file show")),
            "the healthy set makes no per-node read: {calls:?}"
        );
        let git = fs::read_to_string(&git_calls).unwrap_or_default();
        assert_eq!(
            git.lines().filter(|line| *line == "cat-file --batch").count(),
            1,
            "one batch crossing: {git:?}"
        );

        let requests = fs::read_to_string(&git_requests).unwrap_or_default();
        for id in &ids {
            let mapped = commit(&r, id);
            assert!(
                requests
                    .lines()
                    .any(|line| line == format!("{mapped}:nodes/{id}.json")),
                "the batch requests {id} at its mapped commit: {requests:?}"
            );
        }
    }

    #[test]
    fn the_batch_reads_the_owning_commit_not_the_working_copy() {
        let r = scratch("owning-commit", true);
        let a = node(&r, &decision("original"));
        // A later change edits the same document in the working copy: it owns no node.
        next(&r);
        write_node(&r, &a, &decision("edited"));

        let results = read(
            &Jj::new("jj", r.as_path()),
            Some(&Jj::new("git", r.as_path())),
            &parsed(&[a.clone()]),
        );
        match &results[&a] {
            Ok(document) => assert_eq!(
                document
                    .pointer("/content/content")
                    .and_then(Value::as_str),
                Some("original"),
                "the batch reads the owning commit, not the working copy"
            ),
            other => panic!("the owning document must be served, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_object_falls_back_for_that_id_and_preserves_absence() {
        let r = scratch("missing-object", true);
        let a = node(&r, &decision("a"));
        let b = next(&r);
        let ids = vec![a.clone(), b.clone()];
        let work = shim_dir("missing");
        let (jj_bin, jj_log) = jj_shim(&work, "missing");

        let results = read(
            &Jj::new(jj_bin.as_path(), r.as_path()),
            Some(&Jj::new("git", r.as_path())),
            &parsed(&ids),
        );
        assert!(results[&a].is_ok(), "the served id still loads");
        assert!(
            matches!(results[&b], Err(LoadError::Missing)),
            "absence is preserved: {:?}",
            results[&b]
        );

        let calls = fs::read_to_string(&jj_log).unwrap_or_default();
        let shows: Vec<&str> = calls
            .lines()
            .filter(|line| line.starts_with("file show"))
            .collect();
        assert_eq!(shows.len(), 1, "one per-object fallback: {calls:?}");
        assert!(
            shows[0].contains(&b),
            "the fallback is for the id the batch could not serve: {calls:?}"
        );
    }

    #[test]
    fn a_non_blob_object_falls_back_for_that_id() {
        let r = scratch("non-blob", true);
        let a = node(&r, &decision("a"));
        let b = next(&r);
        // `nodes/<b>.json` is a directory, so the batch answers with a tree, not a blob.
        fs::create_dir_all(r.join("nodes").join(format!("{b}.json"))).unwrap();
        fs::write(
            r.join("nodes").join(format!("{b}.json")).join("child"),
            "x",
        )
        .unwrap();
        jj(&r, &["status"]);
        let ids = vec![a.clone(), b.clone()];

        let results = read(
            &Jj::new("jj", r.as_path()),
            Some(&Jj::new("git", r.as_path())),
            &parsed(&ids),
        );
        assert!(results[&a].is_ok(), "the blob is served");
        matches_the_single_document_read(&r, &results, &ids);
    }

    #[test]
    fn a_mapping_failure_falls_back_for_the_whole_set() {
        let r = scratch("mapping-failure", true);
        let a = node(&r, &decision("a"));
        let b = node(&r, &decision("b"));
        let ids = vec![a.clone(), b.clone()];
        let work = shim_dir("mapping");
        let jj_bin = work.join("mapping-jj");
        shim(
            &jj_bin,
            &format!(
                "#!/bin/sh\nif [ \"$1\" = \"log\" ]; then exit 1; fi\nexec \"{}\" \"$@\"\n",
                real("jj").display()
            ),
        );

        let results = read(
            &Jj::new(jj_bin.as_path(), r.as_path()),
            Some(&Jj::new("git", r.as_path())),
            &parsed(&ids),
        );
        matches_the_single_document_read(&r, &results, &ids);
    }

    #[test]
    fn a_git_failure_falls_back_for_the_whole_set() {
        let r = scratch("git-failure", true);
        let a = node(&r, &decision("a"));
        let b = node(&r, &decision("b"));
        let ids = vec![a.clone(), b.clone()];
        let missing = r.join("no-such-git");

        let results = read(
            &Jj::new("jj", r.as_path()),
            Some(&Jj::new(missing.as_path(), r.as_path())),
            &parsed(&ids),
        );
        matches_the_single_document_read(&r, &results, &ids);
    }

    #[test]
    fn an_absent_or_non_colocated_git_runner_falls_back() {
        let r = scratch("no-git", false);
        let a = node(&r, &decision("a"));
        let b = node(&r, &decision("b"));
        let ids = vec![a.clone(), b.clone()];

        let without = read(&Jj::new("jj", r.as_path()), None, &parsed(&ids));
        matches_the_single_document_read(&r, &without, &ids);

        let non_colocated = read(
            &Jj::new("jj", r.as_path()),
            Some(&Jj::new("git", r.as_path())),
            &parsed(&ids),
        );
        matches_the_single_document_read(&r, &non_colocated, &ids);
    }

    #[test]
    fn a_divergent_change_falls_back_per_id() {
        let r = scratch("divergent", true);
        let a = node(&r, &decision("a"));
        let b = node(&r, &decision("b"));
        let ids = vec![a.clone(), b.clone()];
        let (commit_a, commit_b) = (commit(&r, &a), commit(&r, &b));
        let work = shim_dir("divergent");
        let jj_bin = work.join("divergent-jj");
        let jj_log = work.join("divergent-jj.log");
        shim(
            &jj_bin,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\nif [ \"$1\" = \"log\" ]; then\n  printf '%s %s\\n%s %s\\n%s %s\\n' \"{a}\" \"{commit_a}\" \"{a}\" \"{commit_b}\" \"{b}\" \"{commit_b}\"\n  exit 0\nfi\nexec \"{}\" \"$@\"\n",
                jj_log.display(),
                real("jj").display(),
                a = a.as_str(),
                commit_a = commit_a.as_str(),
                commit_b = commit_b.as_str(),
                b = b.as_str()
            ),
        );
        let (git_bin, _git_calls, git_requests) = git_shim(&work, "divergent");

        let results = read(
            &Jj::new(jj_bin.as_path(), r.as_path()),
            Some(&Jj::new(git_bin.as_path(), r.as_path())),
            &parsed(&ids),
        );
        matches_the_single_document_read(&r, &results, &ids);

        let calls = fs::read_to_string(&jj_log).unwrap_or_default();
        assert!(
            calls
                .lines()
                .any(|line| line.starts_with("file show") && line.contains(&a)),
            "the divergent id falls back: {calls:?}"
        );
        let requests = fs::read_to_string(&git_requests).unwrap_or_default();
        assert!(
            !requests.contains(&format!(":nodes/{a}.json")),
            "the divergent id is never requested from the batch: {requests:?}"
        );
        assert!(
            requests
                .lines()
                .any(|line| line == format!("{commit_b}:nodes/{b}.json")),
            "the non-divergent id is served from the batch: {requests:?}"
        );
    }
}
