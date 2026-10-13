//! Gate for M-node-set (the node set, its documents, and its edges).
//!
//! A crate of its own: listing pulls in node_load, which needs serde_json and a schema
//! validator. Self-contained per M-tests — the shims pull in the module under test and the
//! siblings it calls; no shared test-support module. The gate is driven over a scratch jj
//! repo — a real workspace whose node files are named by the change ids that own them —
//! because the set is the ids jj reports and each document is read at the change its id
//! names.

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
#[path = "../../../src/node_set.rs"]
mod node_set;

#[cfg(test)]
mod gate {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::Value;

    use crate::jj::Jj;
    use crate::node_set::{list, Edges, NodeRead};

    fn decision() -> &'static str {
        r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"x","rationale":"y"}}"#
    }

    fn child(parent: &str) -> String {
        format!(
            r#"{{"kind":"decision","depends_on":["{parent}"],"content":{{"type":"text","ref":"inline","content":"x","rationale":"y"}}}}"#
        )
    }

    fn evaluation(evaluates: &str, depends_on: &str) -> String {
        format!(
            r#"{{"kind":"evaluation","evaluates":["{evaluates}"],"depends_on":["{depends_on}"],"content":{{"type":"test","ref":"inline","content":"x","rationale":"y"}}}}"#
        )
    }

    /// A scratch jj repo with a `nodes/` dir. The node schema is carried in the binary
    /// (`include_str!`), never read from the repo.
    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-list-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let status = Command::new("jj")
            .args(["git", "init"])
            .arg(&dir)
            .status()
            .expect("jj is on PATH for the gate");
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

    /// The change id jj reports for the working copy.
    fn head(dir: &Path) -> String {
        jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        )
    }

    /// Write `body` as the node document the current change owns, and return its id.
    fn node(dir: &Path, body: &str) -> String {
        let id = head(dir);
        fs::write(dir.join("nodes").join(format!("{id}.json")), body).unwrap();
        id
    }

    /// Open a fresh change on top of the working copy and return its id.
    fn next(dir: &Path) -> String {
        jj(dir, &["new"]);
        head(dir)
    }

    fn listed(dir: &Path) -> BTreeMap<String, NodeRead> {
        list(&Jj::new("jj", dir)).expect("the listing succeeds")
    }

    /// The document and edges of a node that loaded.
    fn loaded<'a>(nodes: &'a BTreeMap<String, NodeRead>, id: &str) -> (&'a Value, &'a Edges) {
        match &nodes[id] {
            NodeRead::Loaded { document, edges } => (document, edges),
            NodeRead::Failed(error) => panic!("{id} should load, got {error:?}"),
        }
    }

    fn sorted(mut ids: Vec<String>) -> Vec<String> {
        ids.sort();
        ids
    }

    #[test]
    fn every_node_the_listing_reports_has_its_document_and_edges() {
        let r = repo("corpus");
        let a = node(&r, decision());
        let b = next(&r);
        fs::write(r.join("nodes").join(format!("{b}.json")), child(&a)).unwrap();
        let c = next(&r);
        fs::write(
            r.join("nodes").join(format!("{c}.json")),
            evaluation(&b, &a),
        )
        .unwrap();

        let nodes = listed(&r);
        assert_eq!(
            nodes.keys().cloned().collect::<Vec<_>>(),
            sorted(vec![a.clone(), b.clone(), c.clone()])
        );

        let (document, edges) = loaded(&nodes, &a);
        assert_eq!(
            document.get("kind").and_then(Value::as_str),
            Some("decision")
        );
        assert_eq!(edges.depends_on, vec!["zzzzzzzz".to_string()]);
        assert!(edges.evaluates.is_empty());

        let (document, edges) = loaded(&nodes, &b);
        assert_eq!(
            document.get("kind").and_then(Value::as_str),
            Some("decision")
        );
        assert_eq!(edges.depends_on, vec![a.clone()]);
        assert!(edges.evaluates.is_empty());

        let (document, edges) = loaded(&nodes, &c);
        assert_eq!(
            document.get("kind").and_then(Value::as_str),
            Some("evaluation")
        );
        assert_eq!(edges.depends_on, vec![a.clone()]);
        assert_eq!(edges.evaluates, vec![b.clone()]);
    }

    #[test]
    fn a_document_that_fails_to_load_is_an_outcome_not_an_abort() {
        let r = repo("broken");
        let a = node(&r, decision());
        let b = next(&r);
        fs::write(r.join("nodes").join(format!("{b}.json")), "{ this is not json").unwrap();
        let nodes = listed(&r);
        assert!(matches!(nodes[&a], NodeRead::Loaded { .. }));
        assert!(matches!(nodes[&b], NodeRead::Failed(_)));
    }

    #[test]
    fn a_change_that_owns_no_node_document_is_not_a_node() {
        let r = repo("stray");
        let a = node(&r, decision());
        next(&r);
        fs::create_dir_all(r.join("src")).unwrap();
        fs::write(r.join("src/lib.rs"), "// not a node\n").unwrap();
        let nodes = listed(&r);
        assert_eq!(nodes.keys().cloned().collect::<Vec<_>>(), [a]);
    }

    #[test]
    fn an_empty_node_sets_nothing() {
        let r = repo("empty");
        assert!(listed(&r).is_empty());
    }
}
