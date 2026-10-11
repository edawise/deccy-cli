//! Gate for M-V-validate (the validate verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj repo — a real
//! workspace holding real node files — and the checks reach node_set, node_resolve, and
//! node_load, so this is a crate of its own, needing serde_json and the schema
//! validator. The compile-time `VALIDATE_ROW` is the tie to M-route (a Verb row for the
//! handler).

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/jjread.rs"]
mod jjread;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/store_read.rs"]
mod store_read;
#[path = "../../../src/node_load.rs"]
mod node_load;
#[path = "../../../src/node_set.rs"]
mod node_set;
#[path = "../../../src/node_resolve.rs"]
mod node_resolve;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_validate.rs"]
mod verb_validate;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a `Verb` row for `validate` (M-route, ynvwvzrm)
/// compiles.
const VALIDATE_ROW: Verb<Context> = Verb {
    name: "validate",
    run: crate::verb_validate::run,
};

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::context::resolve;
    use crate::refusal::Refusal;
    use crate::verb_validate::run;

    /// A scratch jj repo: a real workspace with a `nodes/` dir. The node schema is carried in
    /// the binary (`include_str!`), never read from the repo.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-validate-{}-{}", std::process::id(), name));
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

    fn ids(ids: &[&str]) -> String {
        ids.iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",")
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

    /// Open a fresh described change and write `body` as the node document it owns; return its
    /// change id, which names the document the read reaches.
    fn node(dir: &Path, summary: &str, body: &str) -> String {
        jj(dir, &["new"]);
        jj(dir, &["describe", "-m", summary]);
        let id = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        write_node(dir, &id, body);
        id
    }

    fn decision(id: &str, depends_on: &[&str]) -> String {
        format!(
            r#"{{"kind":"decision","depends_on":[{}],"content":{{"type":"text","ref":"inline","content":"{id}","rationale":"y"}}}}"#,
            ids(depends_on)
        )
    }

    fn evaluation(id: &str, evaluates: &[&str], depends_on: &[&str]) -> String {
        format!(
            r#"{{"kind":"evaluation","evaluates":[{}],"depends_on":[{}],"content":{{"type":"test","ref":"inline","content":"{id}","rationale":"y"}}}}"#,
            ids(evaluates),
            ids(depends_on)
        )
    }

    fn write_node(dir: &Path, id: &str, body: &str) {
        fs::write(dir.join("nodes").join(format!("{id}.json")), body).unwrap();
    }

    /// `deccy validate <args>` through the real handler over the scratch repo.
    fn validate(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch repo resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        run(&context, &owned)
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir.join("nodes"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_clean_graph_reports_nothing() {
        let dir = scratch("clean");
        let a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        let b = node(&dir, "b", &decision("b", &[a.as_str()]));
        let _c = node(
            &dir,
            "c",
            &evaluation("c", &[b.as_str()], &[a.as_str()]),
        );
        let lines = validate(&dir, &[]).expect("a clean graph validates");
        assert!(lines.is_empty(), "no failures, no lines: {lines:?}");
        assert_eq!(crate::VALIDATE_ROW.name, "validate");
    }

    #[test]
    fn a_schema_invalid_node_is_one_validity_line_naming_it() {
        let dir = scratch("invalid");
        let id = node(&dir, "bad", r#"{"kind":"decision"}"#);
        let lines = validate(&dir, &[]).expect("validate always runs");
        assert_eq!(lines.len(), 1, "one failure: {lines:?}");
        assert!(
            lines[0].starts_with(&format!("validity {id}")),
            "the line names the check and the node: {lines:?}"
        );
    }

    #[test]
    fn an_unreadable_node_is_a_validity_line_not_a_silent_skip() {
        let dir = scratch("unreadable");
        // The read reaches a document, but it is not JSON: the invocation reports it, never skips it.
        let id = node(&dir, "broken", "{ not json");
        let lines = validate(&dir, &[]).expect("validate always runs");
        assert_eq!(lines.len(), 1, "the unreadable node is reported: {lines:?}");
        assert!(
            lines[0].starts_with(&format!("validity {id}")),
            "the line names the check and the node: {lines:?}"
        );
    }

    #[test]
    fn an_edge_target_with_no_file_is_one_resolution_line() {
        let dir = scratch("resolution");
        let _a = node(&dir, "a", &decision("a", &["deadbeef"]));
        let lines = validate(&dir, &[]).expect("validate always runs");
        let resolution: Vec<&String> = lines
            .iter()
            .filter(|line| line.starts_with("resolution "))
            .collect();
        assert_eq!(resolution.len(), 1, "one resolution failure: {lines:?}");
        assert!(
            resolution[0].contains("deadbeef"),
            "the line names the unresolved target: {lines:?}"
        );
    }

    #[test]
    fn an_uncovered_decision_edge_is_one_coverage_line() {
        let dir = scratch("coverage");
        let a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        let b = node(&dir, "b", &decision("b", &[a.as_str()]));
        let lines = validate(&dir, &[]).expect("validate always runs");
        let coverage: Vec<&String> = lines
            .iter()
            .filter(|line| line.starts_with("coverage "))
            .collect();
        assert_eq!(coverage.len(), 1, "one coverage failure: {lines:?}");
        assert!(
            coverage[0].contains(&b) && coverage[0].contains(&a),
            "the line names the dependent and the prerequisite: {lines:?}"
        );
    }

    #[test]
    fn an_edge_to_the_root_is_not_a_coverage_failure() {
        let dir = scratch("root-edge");
        let _a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        let lines = validate(&dir, &[]).expect("validate always runs");
        assert!(
            lines.iter().all(|line| !line.starts_with("coverage ")),
            "the root is not a decision, so its edge is out of scope: {lines:?}"
        );
    }

    #[test]
    fn an_unexpected_argument_refuses_naming_it() {
        let dir = scratch("argument");
        match validate(&dir, &["extra"]) {
            Err(refusal) => assert_eq!(refusal.evidence(), ["extra"]),
            Ok(lines) => panic!("an argument must refuse, got {lines:?}"),
        }
    }

    #[test]
    fn help_is_the_verb_s_own_and_plain() {
        let dir = scratch("help");
        let lines = validate(&dir, &["--help"]).expect("--help shows");
        assert!(!lines.is_empty(), "the verb carries its own help");
        assert!(
            lines.iter().all(|line| !line.contains('\u{1b}')),
            "help is plain UTF-8 with no styling escapes"
        );
    }

    #[test]
    fn validate_changes_nothing() {
        let dir = scratch("readonly");
        let _a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        let before = listing(&dir);
        let _ = validate(&dir, &[]).unwrap();
        assert_eq!(listing(&dir), before);
    }
}
