//! Gate for M-node-load (read, parse, and schema-check a node document).
//!
//! A crate of its own: load needs serde_json and a schema validator, which plain
//! `rustc --test` cannot supply. Self-contained per M-tests — the shims pull in the module
//! under test and the siblings it calls; no shared test-support module. The gate is driven
//! over a scratch jj repo — a real workspace whose node files are named by the change ids
//! that own them — because the read crosses jj and a document is read at the change its id
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

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::jj::Jj;
    use crate::node_load::{load, LoadError};
    use crate::store_path::Id;

    const CONFORMING: &str = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"x","rationale":"y"}}"#;

    /// A scratch jj repo with a `nodes/` dir. The node schema is carried in the binary
    /// (`include_str!`), never read from the repo.
    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-load-{}-{}", std::process::id(), name));
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

    /// The change id jj reports for the working copy.
    fn head(dir: &Path) -> String {
        let out = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"])
            .current_dir(dir)
            .output()
            .expect("jj is on PATH for the gate");
        assert!(out.status.success(), "jj log must succeed");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// Write `body` as the node document the current change owns, and hand back the runner
    /// and the owning change id — the id that names the document.
    fn own(dir: &Path, body: &str) -> (Jj, Id) {
        let id = head(dir);
        fs::write(dir.join("nodes").join(format!("{id}.json")), body).unwrap();
        (
            Jj::new("jj", dir),
            Id::parse(&id).expect("a change id parses"),
        )
    }

    #[test]
    fn a_conforming_document_loads() {
        let r = repo("valid");
        let (jj, id) = own(&r, CONFORMING);
        let doc = load(&jj, &id).expect("a conforming node loads");
        assert_eq!(doc["kind"], "decision");
    }

    #[test]
    fn a_missing_document_reads_apart_from_a_broken_one() {
        let r = repo("missing");
        let empty = head(&r);
        // The id names a change; no `nodes/<id>.json` is at it.
        assert!(matches!(
            load(&Jj::new("jj", r.as_path()), &Id::parse(&empty).unwrap()),
            Err(LoadError::Missing)
        ));
        let (jj, id) = own(&r, "not json");
        assert!(matches!(load(&jj, &id), Err(LoadError::NotJson(_))));
    }

    #[test]
    fn an_unchecked_document_never_surfaces() {
        let r = repo("invalid");
        let (jj, id) = own(&r, r#"{"kind":"decision"}"#);
        assert!(matches!(load(&jj, &id), Err(LoadError::Invalid(_))));
    }
}
