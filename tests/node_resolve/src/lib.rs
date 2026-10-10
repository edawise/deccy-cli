//! Gate for M-node-resolve (edge-target resolution, root exception included).
//!
//! A crate of its own: resolution pulls in node_load, which needs serde_json and a schema
//! validator. Self-contained per M-tests — the shims pull in the module under test and the
//! siblings it calls; no shared test-support module. The gate is driven over a scratch jj
//! repo whose node files are named by the change ids that own them, because the target is
//! read at the change its id names.

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
#[path = "../../../src/node_resolve.rs"]
mod node_resolve;

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::jj::Jj;
    use crate::node_load::LoadError;
    use crate::store_path::Id;
    use crate::node_resolve::{resolve, Resolved, ROOT_ID};

    const CONFORMING: &str = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"x","rationale":"y"}}"#;

    /// A scratch jj repo with a `nodes/` dir. The node schema is carried in the binary
    /// (`include_str!`), never read from the repo.
    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-resolve-{}-{}", std::process::id(), name));
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

    fn runner(dir: &Path) -> Jj {
        Jj::new("jj", dir)
    }

    fn id(raw: &str) -> Id {
        Id::parse(raw).expect("a change id parses")
    }

    #[test]
    fn a_sibling_target_resolves_to_its_document() {
        let r = repo("sibling");
        let sibling = head(&r);
        fs::write(r.join("nodes").join(format!("{sibling}.json")), CONFORMING).unwrap();
        match resolve(&runner(&r), &id(&sibling)) {
            Ok(Resolved::Node(doc)) => assert_eq!(doc["kind"], "decision"),
            other => panic!("expected a node, got {other:?}"),
        }
    }

    #[test]
    fn the_root_resolves_without_a_file() {
        let r = repo("root");
        // nodes/zzzzzzzz.json deliberately does not exist.
        assert!(matches!(
            resolve(&runner(&r), &id(ROOT_ID)),
            Ok(Resolved::Root)
        ));
    }

    #[test]
    fn an_absent_target_surfaces_as_missing() {
        let r = repo("absent");
        let absent = head(&r);
        // The id names a change; no `nodes/<id>.json` is at it.
        assert!(matches!(
            resolve(&runner(&r), &id(&absent)),
            Err(LoadError::Missing)
        ));
    }
}
