//! Gate for E-M-store-write: the write module answers to the interface, the rule, the root and
//! the layout.
//!
//! Self-contained per M-tests: the `#[path]` shims pull in the module under test and the two
//! things it reaches beyond the filesystem — the root holder and the path derivation — and the
//! fixtures are node documents under a scratch root addressed by id. Std-only: no external
//! crate.

#[path = "../src/store.rs"]
mod store;
#[path = "../src/store_path.rs"]
mod store_path;
#[path = "../src/store_write.rs"]
mod store_write;

use std::fs;
use std::path::{Path, PathBuf};

use store::Store;
use store_path::Id;

/// A scratch root under the temp dir, removed by the caller.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "deccy-store-write-{}-{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Create the scratch root's `nodes/` directory and return it.
fn nodes_dir(root: &Path) -> PathBuf {
    let nodes = root.join("nodes");
    fs::create_dir_all(&nodes).unwrap();
    nodes
}

/// The names in `nodes/`, sorted — the document and anything else that landed there.
fn listing(nodes: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(nodes)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_write_places_the_ids_document_at_nodes_id_json_and_nothing_else() {
    let root = scratch("lands");
    let nodes = nodes_dir(&root);
    let id = "abcdefgh";
    let document = r#"{"kind":"decision","depends_on":["zzzzzzzz"]}"#;
    store_write::write(&Store::new(root.clone()), &Id::parse(id).unwrap(), document).unwrap();
    assert_eq!(
        fs::read_to_string(nodes.join(format!("{id}.json"))).unwrap(),
        document,
        "the bytes given are the document's complete contents"
    );
    assert_eq!(
        listing(&nodes),
        vec![format!("{id}.json")],
        "nothing else lands in the directory"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_second_shorter_write_replaces_the_document_whole() {
    let root = scratch("replaces");
    let nodes = nodes_dir(&root);
    let id = "abcdefgh";
    let store = Store::new(root.clone());
    store_write::write(&store, &Id::parse(id).unwrap(), "a first, much longer draft of the document").unwrap();
    store_write::write(&store, &Id::parse(id).unwrap(), "short").unwrap();
    assert_eq!(
        fs::read_to_string(nodes.join(format!("{id}.json"))).unwrap(),
        "short",
        "no stale tail from the longer draft"
    );
    assert_eq!(
        listing(&nodes),
        vec![format!("{id}.json")],
        "no stray temporary"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_write_without_the_documents_directory_fails_and_creates_nothing() {
    let root = scratch("no-nodes-dir");
    assert!(!root.join("nodes").exists());
    match store_write::write(&Store::new(root.clone()), &Id::parse("abcdefgh").unwrap(), "x") {
        Err(error) => assert!(
            !error.to_string().is_empty(),
            "the cause is carried as a value: {error:?}"
        ),
        Ok(()) => panic!("a write with no nodes/ directory cannot complete"),
    }
    assert!(
        !root.join("nodes").exists(),
        "the write never creates the document's directory"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_write_that_cannot_complete_leaves_the_document_and_no_temporary() {
    use std::os::unix::fs::PermissionsExt;

    let root = scratch("cannot-complete");
    let nodes = nodes_dir(&root);
    let id = "abcdefgh";
    let store = Store::new(root.clone());
    let before = "the document as it was";
    store_write::write(&store, &Id::parse(id).unwrap(), before).unwrap();
    // The document's own directory refuses the sibling temporary, so the write cannot land.
    fs::set_permissions(&nodes, fs::Permissions::from_mode(0o555)).unwrap();
    let outcome = store_write::write(&store, &Id::parse(id).unwrap(), "a replacement that cannot land");
    let names = listing(&nodes);
    fs::set_permissions(&nodes, fs::Permissions::from_mode(0o755)).unwrap();
    match outcome {
        Err(error) => assert!(
            !error.to_string().is_empty(),
            "the cause is carried as a value: {error:?}"
        ),
        Ok(()) => panic!("a write whose temporary cannot be created must fail"),
    }
    assert_eq!(
        fs::read_to_string(nodes.join(format!("{id}.json"))).unwrap(),
        before,
        "the document is exactly as it was"
    );
    assert_eq!(
        names,
        vec![format!("{id}.json")],
        "no temporary left behind"
    );
    fs::remove_dir_all(&root).unwrap();
}
