//! Gate for M-V-log (the log verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj repo — a real
//! workspace whose node files are named by real change ids, so the `jj log` descriptions the
//! summaries come from are present — and the rendering reaches node_load, so this is a
//! crate of its own, needing serde_json and the schema validator. The compile-time
//! `LOG_ROW` is the tie to M-route (a Verb row for the handler).
//!
//! Ordering is pinned by constructing edges against the generated change ids: a three-node
//! chain whose ids run opposite its dependencies, an evaluation whose id precedes what it
//! evaluates, an evaluation carrying both `evaluates` and `depends_on`, independent nodes (the
//! ascending tie-break), and a two-node cycle (every node listed, ascending, no hang).

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/jjread.rs"]
mod jjread;
#[path = "../../../src/jjlog.rs"]
mod jjlog;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/store_read.rs"]
mod store_read;
#[path = "../../../src/node_load.rs"]
mod node_load;
#[path = "../../../src/store_bulk.rs"]
mod store_bulk;
#[path = "../../../src/node_set.rs"]
mod node_set;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_log.rs"]
mod verb_log;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a `Verb` row for `log` (M-route, ynvwvzrm) compiles.
const LOG_ROW: Verb<Context> = Verb {
    name: "log",
    run: crate::verb_log::run,
};

#[cfg(test)]
mod gate {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::context::resolve;
    use crate::refusal::Refusal;
    use crate::verb_log::run;

    /// A scratch jj repo: a real workspace with a `nodes/` dir. The node schema is carried in
    /// the binary (`include_str!`), never read from the repo.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-log-{}-{}", std::process::id(), name));
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

    /// The change id jj reports for @, shortest(8).
    fn head_id(dir: &Path) -> String {
        let out = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"])
            .current_dir(dir)
            .output()
            .expect("jj log runs");
        assert!(out.status.success(), "jj log must succeed");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// Open a fresh described change in `dir` and return its shortest(8) change id.
    fn described(dir: &Path, summary: &str) -> String {
        let status = Command::new("jj")
            .args(["new"])
            .current_dir(dir)
            .status()
            .expect("jj new runs");
        assert!(status.success(), "jj new must succeed");
        let status = Command::new("jj")
            .args(["describe", "-m", summary])
            .current_dir(dir)
            .status()
            .expect("jj describe runs");
        assert!(status.success(), "jj describe must succeed");
        head_id(dir)
    }

    fn decision(marker: &str) -> String {
        format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"{marker}","rationale":"y"}}}}"#
        )
    }

    fn evaluation(evaluates: &str, marker: &str) -> String {
        format!(
            r#"{{"kind":"evaluation","evaluates":["{evaluates}"],"content":{{"type":"text","ref":"inline","content":"{marker}","rationale":"y"}}}}"#
        )
    }

    /// A JSON array of the given change ids.
    fn quoted(ids: &[&str]) -> String {
        ids.iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// A decision with an explicit dependency list — the fixture's edges, not the root alone.
    fn decision_on(marker: &str, deps: &[&str]) -> String {
        format!(
            r#"{{"kind":"decision","depends_on":[{}],"content":{{"type":"text","ref":"inline","content":"{marker}","rationale":"y"}}}}"#,
            quoted(deps)
        )
    }

    /// An evaluation that evaluates one node and may also depend on others.
    fn evaluation_on(evaluates: &str, deps: &[&str], marker: &str) -> String {
        let depends_on = if deps.is_empty() {
            String::new()
        } else {
            format!(r#""depends_on":[{}],"#, quoted(deps))
        };
        format!(
            r#"{{"kind":"evaluation","evaluates":["{evaluates}"],{depends_on}"content":{{"type":"text","ref":"inline","content":"{marker}","rationale":"y"}}}}"#
        )
    }

    /// Put `body` in `nodes/<id>.json` at the change that owns it. The read takes the document
    /// from the owning change, not the tip, so each document must be written inside its change.
    fn place(dir: &Path, id: &str, summary: &str, body: &str) {
        let status = Command::new("jj")
            .args(["edit", id])
            .current_dir(dir)
            .status()
            .expect("jj edit runs");
        assert!(status.success(), "jj edit must succeed");
        let status = Command::new("jj")
            .args(["describe", "-m", summary])
            .current_dir(dir)
            .status()
            .expect("jj describe runs");
        assert!(status.success(), "jj describe must succeed");
        write_node(dir, id, body);
    }

    /// The ids the listing put on each line, in order.
    fn line_ids(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .map(|line| line.split(' ').next().unwrap_or("").to_string())
            .collect()
    }

    /// Write the node for `id`, named by that change id — the summary comes from jj's description.
    fn write_node(dir: &Path, id: &str, body: &str) {
        // `jj edit` checks out the owning change and can drop the empty, untracked `nodes/` dir.
        fs::create_dir_all(dir.join("nodes")).unwrap();
        fs::write(dir.join("nodes").join(format!("{id}.json")), body).unwrap();
    }

    /// `deccy log <args>` through the real handler over the scratch repo.
    fn log(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
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
    fn each_node_renders_one_line_with_its_kind_and_summary() {
        let dir = scratch("lines");
        let decision_id = described(&dir, "a decision node");
        write_node(&dir, &decision_id, &decision("d"));
        let evaluation_id = described(&dir, "an evaluation node");
        write_node(&dir, &evaluation_id, &evaluation(&decision_id, "e"));

        let lines = log(&dir, &[]).expect("log lists");
        let got: BTreeSet<String> = lines.iter().cloned().collect();
        assert_eq!(lines.len(), 2, "one line per node: {lines:?}");
        assert!(
            got.contains(&format!("{decision_id} decision a decision node")),
            "the decision line is id + kind + summary: {got:?}"
        );
        assert!(
            got.contains(&format!("{evaluation_id} evaluation an evaluation node")),
            "the evaluation line is id + kind + summary: {got:?}"
        );
        assert_eq!(crate::LOG_ROW.name, "log");
    }

    #[test]
    fn log_changes_nothing() {
        let dir = scratch("readonly");
        let id = described(&dir, "one");
        write_node(&dir, &id, &decision("x"));
        let before = listing(&dir);
        let _ = log(&dir, &[]).unwrap();
        assert_eq!(listing(&dir), before);
    }

    #[test]
    fn help_is_the_verb_s_own_and_plain() {
        let dir = scratch("help");
        let lines = log(&dir, &["--help"]).expect("--help shows");
        assert!(!lines.is_empty(), "the verb carries its own help");
        assert!(
            lines.iter().all(|line| !line.contains('\u{1b}')),
            "help is plain UTF-8 with no styling escapes"
        );
    }

    #[test]
    fn an_argument_refuses_naming_it() {
        let dir = scratch("argument");
        match log(&dir, &["extra"]) {
            Err(refusal) => assert_eq!(refusal.evidence(), ["extra"]),
            Ok(lines) => panic!("an argument must refuse, got {lines:?}"),
        }
    }

    #[test]
    fn a_node_that_fails_to_load_refuses() {
        let dir = scratch("broken");
        let good = described(&dir, "a good node");
        write_node(&dir, &good, &decision("d"));
        let bad = described(&dir, "a broken node");
        write_node(&dir, &bad, "{ not json");
        match log(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().iter().any(|e| e == &bad),
                "the refusal names the node that did not load: {refusal:?}"
            ),
            Ok(lines) => panic!("a node that did not load must refuse, got {lines:?}"),
        }
    }

    #[test]
    fn a_schema_invalid_node_also_refuses_the_whole_set() {
        let dir = scratch("invalid");
        let good = described(&dir, "a good node");
        write_node(&dir, &good, &decision("d"));
        let bad = described(&dir, "an invalid node");
        write_node(&dir, &bad, r#"{"kind":"decision"}"#);
        match log(&dir, &[]) {
            Err(refusal) => assert!(
                refusal.evidence().iter().any(|e| e == &bad),
                "the refusal names the node that did not load: {refusal:?}"
            ),
            Ok(lines) => panic!("an invalid node must refuse the set, got {lines:?}"),
        }
    }

    #[test]
    fn an_empty_node_dir_yields_zero_lines() {
        let dir = scratch("empty");
        let lines = log(&dir, &[]).expect("an empty set lists");
        assert!(lines.is_empty(), "no nodes, no lines: {lines:?}");
    }

    #[test]
    fn a_chain_lists_prerequisites_before_dependents() {
        let dir = scratch("chain");
        let mut ids = vec![
            described(&dir, "change one"),
            described(&dir, "change two"),
            described(&dir, "change three"),
        ];
        ids.sort();
        let (small, mid, large) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
        place(&dir, &small, "small", &decision_on("s", &[&mid]));
        place(&dir, &mid, "mid", &decision_on("m", &[&large]));
        place(&dir, &large, "large", &decision_on("l", &["zzzzzzzz"]));

        let lines = log(&dir, &[]).expect("log lists");
        assert_eq!(lines.len(), 3, "one line per node: {lines:?}");
        assert_eq!(
            line_ids(&lines),
            vec![large, mid, small],
            "every node follows its prerequisite, against change-id order: {lines:?}"
        );
    }

    #[test]
    fn an_evaluation_lists_after_what_it_evaluates() {
        let dir = scratch("evaluates");
        let mut ids = vec![
            described(&dir, "evaluation node"),
            described(&dir, "decision node"),
        ];
        ids.sort();
        let (evaluation_id, decision_id) = (ids[0].clone(), ids[1].clone());
        place(
            &dir,
            &evaluation_id,
            "an evaluation",
            &evaluation_on(&decision_id, &[], "e"),
        );
        place(
            &dir,
            &decision_id,
            "a decision",
            &decision_on("d", &["zzzzzzzz"]),
        );

        let lines = log(&dir, &[]).expect("log lists");
        assert_eq!(lines.len(), 2, "one line per node: {lines:?}");
        assert_eq!(
            line_ids(&lines),
            vec![decision_id, evaluation_id],
            "an evaluation follows what it evaluates, against change-id order: {lines:?}"
        );
    }

    #[test]
    fn an_evaluation_lists_after_both_what_it_evaluates_and_what_it_depends_on() {
        let dir = scratch("both");
        let mut ids = vec![
            described(&dir, "evaluation node"),
            described(&dir, "evaluated node"),
            described(&dir, "dependency node"),
        ];
        ids.sort();
        let (evaluation_id, evaluated_id, dependency_id) =
            (ids[0].clone(), ids[1].clone(), ids[2].clone());
        place(
            &dir,
            &evaluation_id,
            "an evaluation",
            &evaluation_on(&evaluated_id, &[&dependency_id], "e"),
        );
        place(
            &dir,
            &evaluated_id,
            "the evaluated decision",
            &decision_on("d", &["zzzzzzzz"]),
        );
        place(
            &dir,
            &dependency_id,
            "the depended-on decision",
            &decision_on("p", &["zzzzzzzz"]),
        );

        let lines = log(&dir, &[]).expect("log lists");
        assert_eq!(lines.len(), 3, "one line per node: {lines:?}");
        assert_eq!(
            line_ids(&lines),
            vec![evaluated_id, dependency_id, evaluation_id],
            "an evaluation follows both edges, or ties ascending: {lines:?}"
        );
    }

    #[test]
    fn independent_nodes_list_in_ascending_change_id_order() {
        let dir = scratch("independent");
        let ids = vec![
            described(&dir, "one"),
            described(&dir, "two"),
            described(&dir, "three"),
        ];
        let mut expected = ids.clone();
        expected.sort();
        for (index, id) in ids.iter().enumerate() {
            place(
                &dir,
                id,
                &format!("independent {index}"),
                &decision_on("i", &["zzzzzzzz"]),
            );
        }

        let lines = log(&dir, &[]).expect("log lists");
        assert_eq!(lines.len(), 3, "one line per node: {lines:?}");
        assert_eq!(
            line_ids(&lines),
            expected,
            "the tie-break is ascending change id: {lines:?}"
        );
    }

    #[test]
    fn a_cycle_lists_every_node_and_terminates() {
        let dir = scratch("cycle");
        let mut ids = vec![
            described(&dir, "cycle one"),
            described(&dir, "cycle two"),
        ];
        ids.sort();
        let (a, b) = (ids[0].clone(), ids[1].clone());
        place(&dir, &a, "cycle a", &decision_on("a", &[&b]));
        place(&dir, &b, "cycle b", &decision_on("b", &[&a]));

        let lines = log(&dir, &[]).expect("log lists");
        assert_eq!(lines.len(), 2, "both cycle nodes are listed: {lines:?}");
        assert_eq!(
            line_ids(&lines),
            vec![a, b],
            "cycle leftovers follow in ascending change id order: {lines:?}"
        );
    }
}
