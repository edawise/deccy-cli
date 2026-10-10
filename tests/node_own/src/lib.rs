//! Gate for M-node-own (files attributed to a change).
//!
//! A crate of its own: ownership reads the loaded document as serde_json, which
//! plain `rustc --test` cannot supply. Self-contained per M-tests — the shims pull in the
//! module under test and its siblings; no shared test-support module.

#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/node_own.rs"]
mod node_own;

#[cfg(test)]
mod gate {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use serde_json::Value;

    use crate::node_own::owned;
    use crate::store_path::Id;

    const ARRAY_REF: &str = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"rust","ref":["src/a.rs","tests/b.rs"],"rationale":"x"}}"#;
    const INLINE_REF: &str = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"x","rationale":"y"}}"#;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn doc(body: &str) -> Value {
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn an_array_ref_owns_its_node_file_and_each_ref_path() {
        let files = owned(&id("abcdefgh"), &doc(ARRAY_REF));
        assert_eq!(
            files,
            BTreeSet::from([
                PathBuf::from("nodes/abcdefgh.json"),
                PathBuf::from("src/a.rs"),
                PathBuf::from("tests/b.rs"),
            ])
        );
    }

    #[test]
    fn an_inline_ref_owns_the_node_file_alone() {
        let files = owned(&id("abcdefgh"), &doc(INLINE_REF));
        assert_eq!(files, BTreeSet::from([PathBuf::from("nodes/abcdefgh.json")]));
    }

    #[test]
    fn nothing_beyond_the_node_file_and_refs_is_owned() {
        let extra = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"rust","ref":["src/a.rs"],"rationale":"x","content":"ignored"}}"#;
        let files = owned(&id("abcdefgh"), &doc(extra));
        assert_eq!(
            files,
            BTreeSet::from([
                PathBuf::from("nodes/abcdefgh.json"),
                PathBuf::from("src/a.rs"),
            ])
        );
    }
}
