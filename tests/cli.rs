//! Gate for M-main (the crate root: the module map, the verb table, the assembled entry).
//!
//! Self-contained per M-tests: this file carries its own helpers and fixtures. Black-box per
//! I1-test: assert_cmd drives the built binary, and each case gets a tempfile-scoped scratch
//! directory holding a real jj workspace, so the assembled entry resolves the substrate the
//! same way production does. Resolution is total (S3-root-optional): a directory with no
//! workspace still resolves, so the globals answer and a workspace-needing verb refuses on
//! the root-less context. The map check reads `src/lib.rs` back and compares it with the
//! modules under `src/`. The lockfile check reads the committed `Cargo.lock`'s
//! `[[package]]` entry for the crate back against the version token the version
//! decision (nlomlqum) records and the manifest's `[package]` version; the suite
//! runs under `--locked`, so a committed lockfile that no longer matches the
//! manifest is refused before the assertions. The two draft rows are observed end-to-end, not by shape: a
//! successful `draft` leaves the registry holding exactly one `open` record naming the node
//! the call named, and `drafts` names that same handle, so a row pointing at another handler
//! writes nothing (or lists nothing). The `discard` row is observed the same way: the bare
//! list names it in table order and `discard --help` is answered by the discard handler,
//! whose line names the verb.

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::Command as DeccyCommand;
use tempfile::TempDir;

/// The bare list, in table order, with `init` first.
const VERB_LIST: &str =
    "init\nshow\nlog\nvalidate\nnew\nrecord\nremove\nundo\nredo\ndraft\ndrafts\ndiscard\n";

/// A scratch jj workspace: the assembled entry resolves jj on PATH and the root at cwd.
fn workspace() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new("jj")
        .args(["git", "init"])
        .current_dir(dir.path())
        .output()
        .expect("jj is on PATH for the gate");
    assert!(
        out.status.success(),
        "jj git init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

/// The built binary, run in `dir` with `args`.
fn deccy(dir: &Path, args: &[&str]) -> DeccyCommand {
    let mut cmd = DeccyCommand::cargo_bin("deccy").unwrap();
    cmd.current_dir(dir).args(args);
    cmd
}

/// The built binary, run in `dir` with `args` and its `HOME` pinned to `home`: the draft
/// workspaces a successful `draft` opens land under the scratch home, not the caller's.
fn deccy_in_home(dir: &Path, home: &Path, args: &[&str]) -> DeccyCommand {
    let mut cmd = deccy(dir, args);
    cmd.env("HOME", home);
    cmd
}

/// A scratch initialised repository holding one node, and that node's id. `init` lays the
/// state directory down and the work registry is modelled where it lives (M-draft-store,
/// twpvqstu), as the draft verb's own gate models it; `new` creates the node in a change, so
/// the id names both the change and `nodes/<id>.json`.
fn repo_with_a_node(home: &Path) -> (TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    deccy_in_home(dir.path(), home, &["init"])
        .assert()
        .success()
        .stderr("");
    fs::create_dir_all(dir.path().join(".deccy").join("work"))
        .expect("the work registry is created where it lives");
    let out = deccy_in_home(dir.path(), home, &["new", "-m", "gate node", "--on", "zzzzzzzz"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "new: {out:?}");
    assert!(out.stderr.is_empty(), "new: {out:?}");
    let id = String::from_utf8(out.stdout.clone())
        .unwrap()
        .trim()
        .to_string();
    assert!(!id.is_empty(), "new reports the created id: {out:?}");
    assert!(
        dir.path().join("nodes").join(format!("{id}.json")).is_file(),
        "the node document is at the change its id names: {id}"
    );
    (dir, id)
}

/// The `.deccy/work/` record files, sorted by name; none when the registry is not there.
fn records(dir: &Path) -> Vec<std::path::PathBuf> {
    let work = dir.join(".deccy").join("work");
    let mut paths: Vec<std::path::PathBuf> = match fs::read_dir(&work) {
        Ok(entries) => entries
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .collect(),
        Err(_) => Vec::new(),
    };
    paths.sort();
    paths
}

#[test]
fn version_prints_the_build_version() {
    let dir = workspace();
    deccy(dir.path(), &["--version"])
        .assert()
        .success()
        .stdout(format!("{}\n", env!("CARGO_PKG_VERSION")))
        .stderr("");
}

#[test]
fn bare_help_lists_the_verbs() {
    let dir = workspace();
    deccy(dir.path(), &["--help"])
        .assert()
        .success()
        .stdout(VERB_LIST)
        .stderr("");
}

#[test]
fn draft_without_a_node_refuses_and_opens_nothing() {
    // M-verb-draft (xulmunov): a call naming no node is refused at the argument surface,
    // before any workspace or record exists (xzrkrmmr). The refusal's stream and status
    // are the envelope's (S4); its wording belongs to no decision, so the check pins the
    // shape and the absence of a `.deccy/work/` entry, not how the line reads.
    let dir = workspace();
    let mut command = deccy(dir.path(), &["draft"]);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stderr).unwrap().lines().count(),
        1,
        "one stderr line"
    );
    let work = dir.path().join(".deccy").join("work");
    assert!(
        fs::read_dir(&work).map_or(true, |mut entries| entries.next().is_none()),
        "the refusal opens no draft record"
    );
}

#[test]
fn drafts_lists_no_records_on_a_fresh_workspace() {
    // M-verb-drafts (kpkplmqy): a bare `drafts` lists the registry's records, found by
    // scanning `.deccy/work/` (qspsyqlp, rqtowtqn); a workspace with no records lists
    // nothing. The line form is left open, so the check pins that no record line appears.
    let dir = workspace();
    deccy(dir.path(), &["drafts"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn draft_dispatch_opens_one_open_record_naming_the_node() {
    // M-verb-draft (xulmunov) is reached through its own row: the call opens a draft whose
    // record is written `open` at `.deccy/work/<handle>.json` naming the node the call
    // named (kwqnrzoz, vkkwusty). A row pointing at another handler writes nothing.
    let home = tempfile::tempdir().unwrap();
    let (dir, node) = repo_with_a_node(home.path());
    deccy_in_home(dir.path(), home.path(), &["draft", &node])
        .assert()
        .success()
        .stderr("");

    let found = records(dir.path());
    assert_eq!(found.len(), 1, "one record per opening, got {found:?}");
    let path = &found[0];
    let handle = path.file_stem().unwrap().to_string_lossy().into_owned();
    let text = fs::read_to_string(path).unwrap();
    let record: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        record["handle"].as_str(),
        Some(handle.as_str()),
        "the record's handle is the name its file gives: {text}"
    );
    assert_eq!(
        record["state"].as_str(),
        Some("open"),
        "an opened draft has landed nothing: {text}"
    );
    assert_eq!(
        record["node"].as_str(),
        Some(node.as_str()),
        "the record names the node the call named: {text}"
    );
}

#[test]
fn drafts_dispatch_names_the_open_record_it_lists() {
    // M-verb-drafts (kpkplmqy) is reached through its own row: a bare `drafts` lists the
    // record the successful `draft` wrote, by its handle (qspsyqlp, nzrnrlky). A row
    // pointing at another handler lists nothing that names it.
    let home = tempfile::tempdir().unwrap();
    let (dir, node) = repo_with_a_node(home.path());
    deccy_in_home(dir.path(), home.path(), &["draft", &node])
        .assert()
        .success()
        .stderr("");

    let found = records(dir.path());
    assert_eq!(found.len(), 1, "one record per opening, got {found:?}");
    let handle = found[0]
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let out = deccy_in_home(dir.path(), home.path(), &["drafts"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "drafts: {out:?}");
    assert!(out.stderr.is_empty(), "drafts: {out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.lines().any(|line| line.contains(&handle)),
        "the listing names the open record's handle {handle:?}, got {stdout:?}"
    );
}

#[test]
fn discard_row_dispatches_and_the_bare_list_names_it() {
    // M-V-discard (trwtmzyq) reaches the entry through the crate root: `discard` is a row in
    // the table pointing at its own handler, so the bare list names it in table order and
    // `discard --help` is answered by that handler. The help line's wording is the verb's own
    // (xlvoxmyl); the check requires only that it names the verb, so a missing row (the
    // unknown-verb refusal) or a row pointing at another handler does not answer it.
    let dir = workspace();
    let out = deccy(dir.path(), &["discard", "--help"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "discard --help: {out:?}");
    assert!(out.stderr.is_empty(), "discard --help: {out:?}");
    let help = String::from_utf8(out.stdout).unwrap();
    assert!(
        help.lines().any(|line| line.contains("discard")),
        "the discard handler answers its own help, got {help:?}"
    );

    let out = deccy(dir.path(), &["--help"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "--help: {out:?}");
    assert!(out.stderr.is_empty(), "--help: {out:?}");
    let listed = String::from_utf8(out.stdout).unwrap();
    assert!(
        listed.lines().any(|line| line == "discard"),
        "the bare list names the discard row, got {listed:?}"
    );
}

#[test]
fn pre_existing_verbs_still_answer() {
    // The table was extended, not rewritten: verbs that already had a row still dispatch
    // to their own handlers and answer, rather than taking the unknown-verb refusal.
    let dir = workspace();
    deccy(dir.path(), &["log"]).assert().success().stderr("");
    deccy(dir.path(), &["validate"])
        .assert()
        .success()
        .stderr("");
}

#[test]
fn unknown_verb_refuses_named() {
    // D20 (mvkuovxs) fixes that the refusal names the unknown word; O1–O4 and the S4
    // clauses fix the stream and the status, not the wording.
    let dir = workspace();
    let mut command = deccy(dir.path(), &["bogus"]);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "one stderr line: {stderr:?}");
    assert!(
        stderr.contains("bogus"),
        "the refusal names the unknown word, got {stderr:?}"
    );
}

#[test]
fn no_verb_refuses() {
    // S1-shape (knmzsloo): only a single lowercase word dispatches, so a first word that is
    // nothing at all takes the refusal path; the shape is the envelope's.
    let dir = workspace();
    let mut command = deccy(dir.path(), &[]);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stderr).unwrap().lines().count(),
        1,
        "one stderr line"
    );
}

#[test]
fn bare_help_works_without_a_workspace() {
    // Resolution is total: a directory with no workspace resolves root-less, so the globals
    // still answer (S3-root-optional).
    let dir = tempfile::tempdir().unwrap();
    deccy(dir.path(), &["--help"])
        .assert()
        .success()
        .stdout(VERB_LIST)
        .stderr("");
}

#[test]
fn a_verb_that_needs_the_workspace_refuses_without_one() {
    // The root-less context is a state, not a resolution failure: a verb that reaches for the
    // store refuses, naming the absence (S3-root-required).
    let dir = tempfile::tempdir().unwrap();
    deccy(dir.path(), &["log"])
        .assert()
        .code(2)
        .stdout("");
}

#[test]
fn init_prepares_a_repository() {
    // IN-dirs (zynmumoz) and IN-mechanism (lmomkluy) fix the bootstrap's parts; the line the
    // invocation answers with is M-output-return's, so the gate pins that it answers, not
    // what it says.
    let dir = tempfile::tempdir().unwrap();
    let mut command = deccy(dir.path(), &["init"]);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(
        !out.stdout.is_empty(),
        "the invocation answers with its line, got {out:?}"
    );
    assert!(dir.path().join(".git").is_dir(), "a git repository is created");
    assert!(dir.path().join(".jj").is_dir(), "a jj workspace is created");
    assert!(dir.path().join("nodes").is_dir(), "the node directory is created");
    assert!(dir.path().join(".deccy").is_dir(), "the state directory is created");
}

#[test]
fn init_refuses_an_already_initialised_target() {
    // IN-guard (vsyytyrt): the invocation refuses and changes nothing. The refusal's shape
    // is O1–O4's and the S4 clauses'; its wording belongs to no decision.
    let dir = tempfile::tempdir().unwrap();
    deccy(dir.path(), &["init"]).assert().success();
    let before = listing(dir.path());
    let mut command = deccy(dir.path(), &["init"]);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stderr).unwrap().lines().count(),
        1,
        "one stderr line"
    );
    assert_eq!(listing(dir.path()), before, "the refusal changes nothing");
}

/// The target's own entries, sorted, for the changed-nothing assertions.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_map_names_exactly_the_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let declared = declared_modules(&fs::read_to_string(root.join("src/lib.rs")).unwrap());
    let present = modules_on_disk(&root.join("src"));
    assert_eq!(
        declared, present,
        "src/lib.rs must declare exactly the modules under src/"
    );
}

/// The `pub mod <name>;` declarations in the crate root, sorted.
fn declared_modules(lib: &str) -> Vec<String> {
    let mut names: Vec<String> = lib
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub mod ")?
                .strip_suffix(';')
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// The module stems of the `.rs` files directly under `src/`, sorted — the crate root and
/// the binary are not modules.
fn modules_on_disk(src: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(src)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if path.extension().map(|ext| ext == "rs").unwrap_or(false) {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                if stem == "lib" || stem == "main" {
                    None
                } else {
                    Some(stem)
                }
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

#[test]
fn lockfile_carries_the_recorded_version_token() {
    // M-main commits `Cargo.lock`, so the crate's own `[[package]]` entry pins the
    // identity the manifest declares; the version in that entry, in the manifest's
    // `[package]`, and in the version decision (nlomlqum) is one token, taken from the
    // decision rather than chosen at each surface. The check reads all three and
    // requires them to agree. The suite runs under `cargo test --locked`, so a
    // committed lockfile that does not match the manifest is refused before this body.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let token = recorded_version_token(&root.join("nodes/nlomlqum.json"));
    let manifest = manifest_version(&fs::read_to_string(root.join("Cargo.toml")).unwrap())
        .expect("Cargo.toml declares a [package] version");
    let locked = locked_version(
        &fs::read_to_string(root.join("Cargo.lock")).unwrap(),
        "deccy-cli",
    )
    .expect("Cargo.lock carries a [[package]] entry for deccy-cli");
    assert_eq!(
        manifest, token,
        "the manifest's [package] version takes the token the version decision records"
    );
    assert_eq!(
        locked, token,
        "Cargo.lock pins deccy-cli at {locked:?} but the version decision records {token:?}"
    );
}

/// The version token in a version decision's `content.content`.
fn recorded_version_token(node: &Path) -> String {
    let text = fs::read_to_string(node).unwrap();
    let document: serde_json::Value = serde_json::from_str(&text).unwrap();
    document["content"]["content"]
        .as_str()
        .map(|token| token.to_string())
        .unwrap_or_else(|| panic!("{} records no version token", node.display()))
}

/// The `version` value declared inside `[package]` of a TOML manifest.
fn manifest_version(manifest: &str) -> Option<String> {
    let mut inside = false;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            inside = line == "[package]";
            continue;
        }
        if inside {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "version" {
                    return Some(value.trim().trim_matches('"').trim_matches('\'').to_string());
                }
            }
        }
    }
    None
}

/// The `version` of the package named `name` in a TOML lockfile.
fn locked_version(lock: &str, name: &str) -> Option<String> {
    for chunk in lock.split("[[package]]").skip(1) {
        let mut package = None;
        let mut version = None;
        for raw in chunk.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
                match key.trim() {
                    "name" => package = Some(value),
                    "version" => version = Some(value),
                    _ => {}
                }
            }
        }
        if package.as_deref() == Some(name) {
            return version;
        }
    }
    None
}
