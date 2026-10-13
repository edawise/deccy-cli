//! Gate for M-exclude (the exclude module: the file's own read and append).
//!
//! Self-contained per M-tests: pulls the modules under test in with #[path] shims and
//! carries its own fixtures — no shared test-support module. Std-only: no external crate.
//! It drives `exclude::read` and `exclude::append` over scratch roots and pins that the
//! read yields the file's contents as they stand and that a root with no exclude file
//! reads as empty, that an append makes the entry a line (an existing template keeping
//! its lines, a missing final newline added), and that a failure to read or write is
//! carried as a value with the file left as it was.

#[path = "../src/store.rs"]
mod store;
#[path = "../src/exclude.rs"]
mod exclude;

use std::fs;
use std::path::PathBuf;

use exclude::{append, read, ExcludeError};
use store::Store;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-exclude-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Create the exclude file's parent directory under `root`, without the file itself.
fn parent(root: &PathBuf) -> PathBuf {
    let info = root.join(".git/info");
    fs::create_dir_all(&info).unwrap();
    info
}

/// The exclude file under `root`, creating its parent directory and the file.
fn seed(root: &PathBuf, contents: &str) -> PathBuf {
    let info = parent(root);
    let exclude = info.join("exclude");
    fs::write(&exclude, contents).unwrap();
    exclude
}

#[test]
fn read_yields_the_whole_file_as_it_stands() {
    let root = scratch("read-whole");
    let template = "# git ls-files --others --exclude-from=.git/info/exclude\n*.log\n.deccy/\n";
    let file = seed(&root, template);
    let store = Store::new(root.clone());

    assert_eq!(
        read(&store).unwrap(),
        template,
        "the read yields every line already recorded"
    );
    assert!(
        file.exists(),
        "the read touches nothing: the file is still there"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn read_on_a_missing_file_is_empty() {
    let root = scratch("read-absent");
    parent(&root); // the directory exists; the file does not.
    let store = Store::new(root.clone());

    assert_eq!(
        read(&store).unwrap(),
        "",
        "a root with no exclude file reads as empty, not as a failure"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn read_that_cannot_complete_is_carried() {
    let root = scratch("read-fail");
    // `.git/info/exclude` is a directory, so it cannot be read as text.
    fs::create_dir_all(root.join(".git/info/exclude")).unwrap();
    let store = Store::new(root.clone());

    match read(&store) {
        Err(ExcludeError::Read(_)) => {}
        other => panic!("a failed read must be carried, got {other:?}"),
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_fresh_file_gains_the_entry() {
    let root = scratch("fresh");
    let file = seed(&root, "");
    let store = Store::new(root.clone());

    append(&store, ".deccy/").unwrap();

    assert_eq!(fs::read_to_string(&file).unwrap(), ".deccy/\n");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn append_on_a_missing_file_creates_it_with_the_entry() {
    let root = scratch("append-absent");
    parent(&root); // the directory exists; the file does not.
    let store = Store::new(root.clone());

    append(&store, ".deccy/").unwrap();

    assert_eq!(
        fs::read_to_string(root.join(".git/info/exclude")).unwrap(),
        ".deccy/\n",
        "a missing file is empty for the append too, so the entry alone lands"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn an_existing_template_keeps_its_lines() {
    let root = scratch("template");
    let template = "# git ls-files --others --exclude-from=.git/info/exclude\n*.log\n";
    let file = seed(&root, template);
    let store = Store::new(root.clone());

    append(&store, ".deccy/").unwrap();

    let after = fs::read_to_string(&file).unwrap();
    assert_eq!(after, format!("{template}.deccy/\n"));
    assert!(
        after.contains("# git ls-files --others"),
        "git's own template is preserved: {after:?}"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_missing_final_newline_is_added() {
    let root = scratch("no-newline");
    let file = seed(&root, "*.log");
    let store = Store::new(root.clone());

    append(&store, ".deccy/").unwrap();

    assert_eq!(fs::read_to_string(&file).unwrap(), "*.log\n.deccy/\n");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_write_that_cannot_complete_is_carried_and_leaves_the_file() {
    let root = scratch("write-fail");
    let file = seed(&root, "# git's own template\n");
    // Occupy the temporary's name with a directory, so the replace cannot create it.
    fs::create_dir(root.join(format!(".git/info/exclude.tmp-{}", std::process::id()))).unwrap();
    let store = Store::new(root.clone());

    match append(&store, ".deccy/") {
        Err(ExcludeError::Write(_)) => {}
        other => panic!("a failed write must be carried, got {other:?}"),
    }
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "# git's own template\n",
        "a failed write leaves the file as it was"
    );
    fs::remove_dir_all(&root).unwrap();
}
