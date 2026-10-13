//! Gate for M-store (the store type over one root).
//!
//! Self-contained per M-tests: pulls the module under test in with a #[path] shim — no
//! shared test-support module.

#[path = "../src/store.rs"]
mod store;

use std::path::PathBuf;
use store::Store;

#[test]
fn the_root_is_the_whole_state() {
    let root = PathBuf::from("/tmp/graph");
    let s = Store::new(root.clone());
    assert_eq!(s.root(), root.as_path());
}
