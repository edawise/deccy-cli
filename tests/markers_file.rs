//! Gate for M-markers-file (the marker file module: the file's own read and write).
//!
//! Self-contained per M-tests: pulls the modules under test in with #[path] shims and
//! carries its own fixtures — no shared test-support module. Std-only: no external crate.
//! It drives `markers_file::read` and `markers_file::write` over scratch roots and pins
//! that the read yields the file's contents as they stand, that a root with no marker file
//! yields nothing while a file that is there yields its contents even when they are empty,
//! that a write lands the given contents whole (a replacement leaving no stale tail and no
//! stray temporary), and that a failure to read or write is carried as a value with the
//! file left as it was.

#[path = "../src/store.rs"]
mod store;
#[path = "../src/markers_file.rs"]
mod markers_file;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use markers_file::{read, write, FileError, MARKERS_PATH};
use store::Store;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-markers-file-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Create the marker file's parent directory under `root`, without the file itself.
fn parent(root: &PathBuf) -> PathBuf {
    let state = root.join(".deccy");
    fs::create_dir_all(&state).unwrap();
    state
}

/// The marker file under `root`, creating its parent directory and the file.
fn seed(root: &PathBuf, contents: &str) -> PathBuf {
    parent(root);
    let file = root.join(MARKERS_PATH);
    fs::write(&file, contents).unwrap();
    file
}

#[test]
fn read_yields_the_whole_file_as_it_stands() {
    let root = scratch("read-whole");
    let contents = "{\"markers\":[],\"u\":0,\"b\":0}";
    let file = seed(&root, contents);
    let store = Store::new(root.clone());

    assert_eq!(
        read(&store).unwrap().as_deref(),
        Some(contents),
        "the read yields the file's contents as they stand"
    );
    assert!(
        file.exists(),
        "the read touches nothing: the file is still there"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn read_on_a_missing_file_yields_nothing() {
    let root = scratch("read-absent");
    parent(&root); // the directory exists; the file does not.
    let store = Store::new(root.clone());

    assert_eq!(
        read(&store).unwrap(),
        None,
        "a root with no marker file yields nothing, not a failure"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn read_on_an_empty_file_yields_empty_contents_not_absence() {
    let root = scratch("read-empty");
    seed(&root, ""); // the file is there, and its contents are empty.
    let store = Store::new(root.clone());

    assert_eq!(
        read(&store).unwrap(),
        Some(String::new()),
        "a file that is there yields its contents, empty or not: empty contents are not absence"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn read_that_cannot_complete_is_carried() {
    let root = scratch("read-fail");
    // `.deccy/markers.json` is a directory, so it cannot be read as text.
    fs::create_dir_all(root.join(MARKERS_PATH)).unwrap();
    let store = Store::new(root.clone());

    match read(&store) {
        Err(FileError::Read(_)) => {}
        other => panic!("a failed read must be carried, got {other:?}"),
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn write_lands_the_contents_whole() {
    let root = scratch("write");
    let file = seed(&root, "{\"markers\":[],\"u\":0,\"b\":0}");
    let store = Store::new(root.clone());

    write(
        &store,
        "{\"markers\":[{\"entries\":[\"aa\"],\"reverts\":[]}],\"u\":0,\"b\":1}",
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "{\"markers\":[{\"entries\":[\"aa\"],\"reverts\":[]}],\"u\":0,\"b\":1}",
        "the write lands exactly the given contents"
    );
    let mut siblings: Vec<_> = fs::read_dir(root.join(".deccy"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    siblings.sort();
    assert_eq!(
        siblings,
        vec![std::ffi::OsString::from("markers.json")],
        "a replacement leaves no stale tail and no stray temporary: {siblings:?}"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn write_on_a_missing_file_creates_it() {
    let root = scratch("write-absent");
    parent(&root); // the directory exists; the file does not.
    let store = Store::new(root.clone());

    write(&store, "{}").unwrap();

    assert_eq!(fs::read_to_string(root.join(MARKERS_PATH)).unwrap(), "{}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn write_replaces_longer_previous_contents_without_a_tail() {
    let root = scratch("write-shrink");
    let file = seed(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let store = Store::new(root.clone());

    write(&store, "bb").unwrap();

    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "bb",
        "no stale tail survives the replacement"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// Make `path` read-only, so a new entry cannot be created in it; `writable` gives it back.
fn read_only(path: &PathBuf) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o555);
    fs::set_permissions(path, permissions).unwrap();
}

/// Give the path its write permission back so the scratch tree can be torn down.
fn writable(path: &PathBuf) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn a_write_that_cannot_complete_is_carried_and_leaves_the_file() {
    let root = scratch("write-fail");
    let contents = "{\"markers\":[],\"u\":0,\"b\":0}";
    let file = seed(&root, contents);
    // The write lands by replacement (S2-markers-write, S2-atomic), so it needs a new entry
    // in the state directory to land the temporary it renames over the file: a directory
    // that cannot take one refuses the write however the temporary is named.
    read_only(&root.join(".deccy"));
    let store = Store::new(root.clone());

    match write(&store, "{}") {
        Err(FileError::Write(_)) => {}
        other => panic!("a failed write must be carried, got {other:?}"),
    }
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        contents,
        "a failed write leaves the file as it was"
    );
    writable(&root.join(".deccy"));
    fs::remove_dir_all(&root).unwrap();
}
