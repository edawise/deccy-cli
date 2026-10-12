//! Gate for M-jj-init (the ordered bootstrap).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parents
//! (jj, store, store_path, store_write, exclude); no shared test-support module. Plain
//! `rustc --test`, std only —
//! it drives `jj_init::bootstrap` over a scratch directory with real jj and git processes and
//! pins the ordered bootstrap: a git repository is created, `.deccy/` is appended to
//! `.git/info/exclude` with git's own template preserved, then a jj workspace is created
//! colocated with it. It also pins the record step's idempotence — a re-run adds nothing —
//! and that an unreadable exclude file is a carried failure, never a panic.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/store.rs"]
mod store;
#[path = "../src/store_path.rs"]
mod store_path;
#[path = "../src/store_write.rs"]
mod store_write;
#[path = "../src/exclude.rs"]
mod exclude;
#[path = "../src/jj_init.rs"]
mod jj_init;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use jj_init::{bootstrap, BootstrapError};
use store::Store;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-jjinit-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The jj workspace root reported in `dir`, or empty when there is none.
fn jj_root(dir: &Path) -> String {
    let out = Command::new("jj")
        .args(["workspace", "root"])
        .current_dir(dir)
        .output()
        .expect("jj is on PATH for the gate");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Write an executable stand-in binary at `path` with the given script body.
fn stand_in(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_ordered_bootstrap_creates_a_git_repository_appends_the_state_directory_then_colocates() {
    let dir = scratch("ordered");
    let git = Jj::new("git", &dir);
    let jj = Jj::new("jj", &dir);
    let store = Store::new(&dir);

    bootstrap(&git, &jj, &store).expect("the bootstrap runs");

    // 1. A git repository is created.
    assert!(dir.join(".git").is_dir(), "a git repository is created");
    // 2. `.deccy/` is appended to `.git/info/exclude`; git's own template is preserved.
    let exclude = fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
    assert!(
        exclude.lines().any(|line| line == ".deccy/"),
        "the state directory is appended: {exclude:?}"
    );
    assert!(
        exclude.contains("# git ls-files --others --exclude-from=.git/info/exclude"),
        "git's own template is preserved: {exclude:?}"
    );
    // 3. A jj workspace is created colocated with it.
    assert!(dir.join(".jj").is_dir(), "a jj workspace is created");
    assert_eq!(
        fs::canonicalize(jj_root(&dir)).unwrap(),
        fs::canonicalize(&dir).unwrap(),
        "the workspace is colocated with the git repository"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_state_directory_is_recorded_before_the_workspace_is_created() {
    let dir = scratch("order");
    // A jj binary that refuses to run unless `.deccy/` is already recorded in the exclude
    // file: if the workspace were created before the state directory was recorded, the
    // crossing fails and the bootstrap returns a failure value.
    let fake = dir.join("fake-jj.sh");
    stand_in(
        &fake,
        "#!/bin/sh\n\
         grep -qx '.deccy/' .git/info/exclude || { echo 'state dir not recorded yet' >&2; exit 3; }\n\
         exec jj \"$@\"\n",
    );
    let git = Jj::new("git", &dir);
    let jj = Jj::new(&fake, &dir);
    let store = Store::new(&dir);

    bootstrap(&git, &jj, &store).expect("the state directory is recorded before the workspace");

    assert!(dir.join(".jj").is_dir(), "the workspace crossing ran");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_crossing_that_cannot_spawn_is_a_failure_value_not_a_panic() {
    let dir = scratch("spawn");
    let git = Jj::new(dir.join("no-such-git"), &dir);
    let jj = Jj::new("jj", &dir);
    let store = Store::new(&dir);

    match bootstrap(&git, &jj, &store) {
        Err(BootstrapError::Spawn(reason)) => assert!(!reason.is_empty()),
        Err(other) => panic!("a missing git binary must be a spawn failure, got {other:?}"),
        Ok(()) => panic!("a missing git binary must not bootstrap"),
    }
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_crossing_that_exits_non_zero_is_a_failure_value() {
    let dir = scratch("failed");
    let fake = dir.join("failing-git.sh");
    stand_in(&fake, "#!/bin/sh\necho 'git says no' >&2\nexit 2\n");
    let git = Jj::new(&fake, &dir);
    let jj = Jj::new("jj", &dir);
    let store = Store::new(&dir);

    match bootstrap(&git, &jj, &store) {
        Err(BootstrapError::Failed(output)) => assert_eq!(output.status, 2),
        Err(other) => panic!("a non-zero crossing must be a failure value, got {other:?}"),
        Ok(()) => panic!("a failing crossing must not bootstrap"),
    }
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn recording_the_state_directory_is_idempotent() {
    let dir = scratch("idempotent");
    let git = Jj::new("git", &dir);
    let jj = Jj::new("jj", &dir);
    let store = Store::new(&dir);

    bootstrap(&git, &jj, &store).expect("the first bootstrap runs");
    // A second bootstrap re-runs the record step over a file that already holds the entry.
    // Its workspace crossing refuses a repository that already exists, so only the exclude
    // file's state is pinned here.
    let _ = bootstrap(&git, &jj, &store);

    let exclude = fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
    assert_eq!(
        exclude.lines().filter(|line| *line == ".deccy/").count(),
        1,
        "the state directory is recorded once, so a re-run adds nothing: {exclude:?}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_unreadable_exclude_file_is_a_carried_failure_not_a_panic() {
    let dir = scratch("unreadable");
    // A git that initializes the repository and then leaves `.git/info/exclude` a directory:
    // the exclude read fails, and the failure is carried as a value, never rendered.
    let fake = dir.join("git-unreadable-exclude.sh");
    stand_in(
        &fake,
        "#!/bin/sh\n\
         git \"$@\" || exit $?\n\
         rm -f .git/info/exclude\n\
         mkdir .git/info/exclude\n",
    );
    let git = Jj::new(&fake, &dir);
    let jj = Jj::new("jj", &dir);
    let store = Store::new(&dir);

    match bootstrap(&git, &jj, &store) {
        Err(BootstrapError::Exclude(_)) => {}
        Err(other) => panic!("an unreadable exclude file must be an exclude failure, got {other:?}"),
        Ok(()) => panic!("an unreadable exclude file must not bootstrap"),
    }
    fs::remove_dir_all(&dir).unwrap();
}
