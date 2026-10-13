//! Gate for M-node-set (the node set, its documents, its edges, and the order it lists).
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
#[path = "../../../src/store_bulk.rs"]
mod store_bulk;
#[path = "../../../src/node_set.rs"]
mod node_set;

#[cfg(test)]
mod gate {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::Value;

    use crate::jj::Jj;
    use crate::node_set::{list, list_ordered, list_with_drops, Edges, NodeRead};

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
        // Snapshot the document into its own change so the change's commit id is stable
        // before the read maps it.
        jj(dir, &["status"]);
        id
    }

    /// Open a fresh change on top of the working copy and return its id.
    fn next(dir: &Path) -> String {
        jj(dir, &["new"]);
        head(dir)
    }

    fn listed(dir: &Path) -> BTreeMap<String, NodeRead> {
        list(&Jj::new("jj", dir), None).expect("the listing succeeds")
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

    /// The order jj itself lists the changes touching `nodes/`, read straight from jj so the
    /// expectation never comes from the module under test.
    fn listing_order(dir: &Path) -> Vec<String> {
        jj(
            dir,
            &[
                "log",
                "--no-graph",
                "-r",
                "files(\"nodes/\")",
                "-T",
                "change_id.shortest(8) ++ \"\\n\"",
            ],
        )
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
    }

    /// The ids an ordered listing reports, in the order it reports them.
    fn order_of(ordered: &[(String, NodeRead)]) -> Vec<String> {
        ordered.iter().map(|(id, _)| id.clone()).collect()
    }

    /// The candidates jj lists for the fixture minus the ids the fixture made into nodes:
    /// the candidates that own no `nodes/<their own id>.json` and so must be reported as
    /// dropped. Read from jj's own listing, never from the module under test.
    fn dropped_in(dir: &Path, nodes: &[String]) -> Vec<String> {
        listing_order(dir)
            .into_iter()
            .filter(|id| !nodes.contains(id))
            .collect()
    }

    /// A scratch jj repo whose objects git can reach, for the batch path.
    fn colocated(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-list-git-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let status = Command::new("jj")
            .args(["git", "init", "--colocate"])
            .arg(&dir)
            .status()
            .expect("jj is on PATH for the gate");
        assert!(status.success(), "jj git init --colocate must succeed");
        fs::create_dir_all(dir.join("nodes")).unwrap();
        dir
    }

    /// The real binary behind a shim, found before any shim is created.
    fn real(name: &str) -> PathBuf {
        let path = std::env::var_os("PATH").expect("PATH is set");
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
        panic!("{name} must be on PATH");
    }

    fn shim(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// A scratch directory for shims, outside every fixture so it never enters a working copy.
    fn shim_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-list-shims-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A jj shim that logs every call and then runs the real jj.
    fn node_shim(work: &Path, name: &str) -> (PathBuf, PathBuf) {
        let bin = work.join(format!("{name}-jj"));
        let log = work.join(format!("{name}-jj.log"));
        shim(
            &bin,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\nexec \"{}\" \"$@\"\n",
                log.display(),
                real("jj").display()
            ),
        );
        (bin, log)
    }

    /// A git shim that logs its calls and stdin and then runs the real git.
    fn git_shim(work: &Path, name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let bin = work.join(format!("{name}-git"));
        let calls = work.join(format!("{name}-git.log"));
        let requests = work.join(format!("{name}-git.stdin"));
        shim(
            &bin,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\ntee -a \"{}\" | \"{}\" \"$@\"\n",
                calls.display(),
                requests.display(),
                real("git").display()
            ),
        );
        (bin, calls, requests)
    }

    fn file_shows(log: &Path) -> usize {
        fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with("file show"))
            .count()
    }

    fn batches(log: &Path) -> usize {
        fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .filter(|line| *line == "cat-file --batch")
            .count()
    }

    /// `count` real node changes, each owning its own document.
    fn seed(dir: &Path, count: usize) {
        for _ in 0..count {
            next(dir);
            node(dir, decision());
        }
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
    fn the_listing_reports_the_ids_in_the_order_jj_listed_them() {
        let r = repo("order");
        // Seed nodes until jj's listing order differs from the ids' sort order, so a listing
        // that sorts its ids — what the set's `BTreeMap` yields — cannot pass by coincidence.
        let mut ids = vec![node(&r, decision())];
        loop {
            let order = listing_order(&r);
            assert_eq!(
                sorted(order.clone()),
                sorted(ids.clone()),
                "every node is listed once"
            );
            if order != sorted(order.clone()) {
                break;
            }
            assert!(
                ids.len() < 12,
                "the fixture never produced a listing order other than the ids' order"
            );
            next(&r);
            ids.push(node(&r, decision()));
        }

        let expected = listing_order(&r);
        assert_eq!(
            sorted(expected.clone()),
            sorted(ids.clone()),
            "every node is listed once"
        );
        assert!(
            expected != sorted(expected.clone()),
            "the fixture lists the ids in an order other than their own"
        );
        let ordered = list_ordered(&Jj::new("jj", &r), None).expect("the listing succeeds");
        assert_eq!(
            order_of(&ordered),
            expected,
            "the ids are reported in the order jj listed them, not in the ids' own order"
        );
    }

    #[test]
    fn the_ordered_listing_keeps_jjs_order_across_a_candidate_that_is_not_a_node() {
        let r = repo("order-skips");
        let a = node(&r, decision());
        let b = next(&r);
        // A buffer that touches `nodes/` by editing a sibling's document and owns none of its
        // own is a candidate, not a node (S2-set).
        fs::write(
            r.join("nodes").join(format!("{a}.json")),
            r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"edited by the buffer","rationale":"y"}}"#,
        )
        .unwrap();
        next(&r);
        let c = node(&r, decision());

        let expected: Vec<String> = listing_order(&r)
            .into_iter()
            .filter(|id| id != &b)
            .collect();
        let ordered = list_ordered(&Jj::new("jj", &r), None).expect("the listing succeeds");
        assert_eq!(
            order_of(&ordered),
            expected,
            "the buffer is not a node, and the rest keep the order jj listed them"
        );
        assert!(!order_of(&ordered).contains(&b), "the buffer is not a node");

        let by_id = listed(&r);
        assert_eq!(order_of(&ordered).len(), by_id.len(), "the ordered listing is the set");
        for (id, read) in &ordered {
            let same = by_id
                .get(id)
                .unwrap_or_else(|| panic!("{id} is in the set"));
            assert_eq!(
                format!("{read:?}"),
                format!("{same:?}"),
                "{id} is read the same way in both listings"
            );
        }
        assert!(order_of(&ordered).contains(&c));
        assert!(order_of(&ordered).contains(&a));
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

        // The ordered listing surfaces the same outcomes, keeping jj's order.
        let ordered = list_ordered(&Jj::new("jj", &r), None).expect("the listing succeeds");
        assert_eq!(
            order_of(&ordered),
            listing_order(&r),
            "a failure keeps the order jj listed"
        );
        let failed: Vec<&String> = ordered
            .iter()
            .filter(|(_, read)| matches!(read, NodeRead::Failed(_)))
            .map(|(id, _)| id)
            .collect();
        assert_eq!(failed, vec![&b], "the broken document fails, it does not abort");
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
    fn a_buffer_that_edits_another_nodes_document_is_not_a_node() {
        let r = repo("buffer");
        let a = node(&r, decision());
        let b = next(&r);
        // The buffer's only change is to a sibling's document: it touches `nodes/` but owns
        // no `nodes/<its own id>.json`, so it is not a node (S2-set).
        fs::write(
            r.join("nodes").join(format!("{a}.json")),
            r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"edited by the buffer","rationale":"y"}}"#,
        )
        .unwrap();

        let nodes = listed(&r);
        assert_eq!(
            nodes.keys().cloned().collect::<Vec<_>>(),
            [a.clone()],
            "the buffer is not a node"
        );
        assert!(!nodes.contains_key(&b), "the buffer's id names no document");
        let (document, _) = loaded(&nodes, &a);
        assert_eq!(
            document.pointer("/content/content").and_then(Value::as_str),
            Some("x"),
            "the node is read at its own change, not through the buffer's edit"
        );
    }

    #[test]
    fn a_change_with_a_stray_file_under_nodes_is_not_a_node() {
        let r = repo("stray-under-nodes");
        let a = node(&r, decision());
        let b = next(&r);
        // A change that adds a stray file under `nodes/` touches the path without carrying
        // its own document, so it is not a node (S2-set).
        fs::write(r.join("nodes").join("not-a-node.json"), "{}\n").unwrap();

        let nodes = listed(&r);
        assert_eq!(
            nodes.keys().cloned().collect::<Vec<_>>(),
            [a],
            "the stray change is not a node"
        );
        assert!(!nodes.contains_key(&b), "the stray change's id names no document");
    }

    #[test]
    fn an_empty_node_sets_nothing() {
        let r = repo("empty");
        assert!(listed(&r).is_empty());
        assert!(
            list_ordered(&Jj::new("jj", &r), None)
                .expect("the listing succeeds")
                .is_empty(),
            "an empty set lists nothing, in order"
        );
    }

    #[test]
    fn the_crossing_count_does_not_grow_with_the_node_count() {
        let small = colocated("count-small");
        let large = colocated("count-large");
        seed(&small, 2);
        seed(&large, 5);

        let work = shim_dir("count");
        let (jj_small, log_small) = node_shim(&work, "small");
        let (jj_large, log_large) = node_shim(&work, "large");
        let (git_small, calls_small, _) = git_shim(&work, "small");
        let (git_large, calls_large, _) = git_shim(&work, "large");

        let small_nodes = list(
            &Jj::new(jj_small.as_path(), small.as_path()),
            Some(git_small.as_path()),
        )
        .expect("the small set lists");
        let large_nodes = list(
            &Jj::new(jj_large.as_path(), large.as_path()),
            Some(git_large.as_path()),
        )
        .expect("the large set lists");
        assert_eq!(small_nodes.len(), 2);
        assert_eq!(large_nodes.len(), 5);

        assert_eq!(
            file_shows(&log_small),
            file_shows(&log_large),
            "the crossing count must not grow with the node count: small={:?} large={:?}",
            fs::read_to_string(&log_small),
            fs::read_to_string(&log_large)
        );
        assert_eq!(batches(&calls_small), 1, "one batch for the small set");
        assert_eq!(batches(&calls_large), 1, "one batch for the large set");
    }

    #[test]
    fn the_set_read_reports_the_candidates_it_drops_for_want_of_a_document() {
        let r = colocated("drops");
        let a = node(&r, decision());
        // A buffer that touches `nodes/` by editing a sibling's document owns none of its own,
        // so the set read drops it (S2-set).
        let buffer = next(&r);
        fs::write(
            r.join("nodes").join(format!("{a}.json")),
            r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"edited by the buffer","rationale":"y"}}"#,
        )
        .unwrap();
        next(&r);
        let c = node(&r, decision());
        // A change carrying a stray file under `nodes/` touches the path without owning a
        // document, so the set read drops it too.
        let stray = next(&r);
        fs::write(r.join("nodes").join("not-a-node.json"), "{}\n").unwrap();
        next(&r);
        let e = node(&r, decision());

        let nodes = vec![a.clone(), c.clone(), e.clone()];
        let expected_dropped = dropped_in(&r, &nodes);
        assert_eq!(
            sorted(expected_dropped.clone()),
            sorted(vec![buffer.clone(), stray.clone()]),
            "the fixture drops a buffer and a stray change, and nothing else"
        );

        // Both read paths — the batched one through git and the single-document fallback —
        // report the same drops.
        let git = real("git");
        let (batched, batched_dropped) =
            list_with_drops(&Jj::new("jj", &r), Some(git.as_path())).expect("the set read reports");
        let (fallback, fallback_dropped) =
            list_with_drops(&Jj::new("jj", &r), None).expect("the set read reports");
        assert_eq!(
            sorted(batched_dropped.clone()),
            sorted(expected_dropped.clone()),
            "the candidates dropped for want of their own document are reported by id"
        );
        assert_eq!(
            sorted(fallback_dropped.clone()),
            sorted(batched_dropped.clone()),
            "the report does not depend on which read path served the documents"
        );
        assert_eq!(
            order_of(&fallback),
            order_of(&batched),
            "both read paths report the same set, in the same order"
        );

        // Reporting a drop does not add it: the drops and the nodes are disjoint, and together
        // they are exactly jj's candidate listing.
        let ordered = order_of(&batched);
        for id in &batched_dropped {
            assert!(
                !ordered.contains(id),
                "{id} is dropped for want of its own document, so it is not in the set"
            );
        }
        assert_eq!(
            sorted(ordered.clone()),
            sorted(nodes.clone()),
            "the node set is exactly the candidates that own their own document"
        );
        let mut reported = ordered.clone();
        reported.extend(batched_dropped.clone());
        assert_eq!(
            sorted(reported),
            sorted(listing_order(&r)),
            "the set and the drops account for every candidate jj listed, once each"
        );
        assert_eq!(
            ordered,
            listing_order(&r)
                .into_iter()
                .filter(|id| !batched_dropped.contains(id))
                .collect::<Vec<_>>(),
            "the set keeps jj's order, with the dropped candidates' places left out"
        );

        // The unchanged part: `list` keeps its keys, and the ordered set is what
        // `list_ordered` reports, entry for entry.
        let by_id = listed(&r);
        assert_eq!(
            sorted(by_id.keys().cloned().collect()),
            sorted(nodes.clone()),
            "`list` is keyed by exactly the node ids"
        );
        assert!(
            !by_id.contains_key(&buffer),
            "the buffer is still not a node"
        );
        assert!(
            !by_id.contains_key(&stray),
            "the stray change is still not a node"
        );
        let plain = list_ordered(&Jj::new("jj", &r), None).expect("the listing succeeds");
        assert_eq!(
            order_of(&plain),
            order_of(&batched),
            "`list_ordered` reports the same set in the same order"
        );
        for (id, read) in &batched {
            let same = plain
                .iter()
                .find(|(other, _)| other == id)
                .map(|(_, read)| read)
                .unwrap_or_else(|| panic!("{id} is in the ordered set"));
            assert_eq!(
                format!("{read:?}"),
                format!("{same:?}"),
                "{id} is read the same way in both listings"
            );
        }
    }

    #[test]
    fn a_candidate_whose_document_fails_to_load_is_not_a_dropped_candidate() {
        let r = repo("drops-broken");
        let a = node(&r, decision());
        next(&r);
        let broken = head(&r);
        // The change owns its document; the document simply does not load, which is a node
        // with a failure, not a change dropped for want of a document (S2-set, S2-invalid).
        fs::write(
            r.join("nodes").join(format!("{broken}.json")),
            "{ this is not json",
        )
        .unwrap();

        let (ordered, dropped) =
            list_with_drops(&Jj::new("jj", &r), None).expect("the set read reports");
        assert!(
            dropped.is_empty(),
            "only a candidate with no document of its own is dropped, got {dropped:?}"
        );
        assert_eq!(
            sorted(order_of(&ordered)),
            sorted(vec![a, broken.clone()]),
            "the failing document stays in the set"
        );
        assert_eq!(
            order_of(&ordered),
            listing_order(&r),
            "a failure keeps the order jj listed"
        );
        let failure = ordered
            .iter()
            .find(|(id, _)| id == &broken)
            .unwrap_or_else(|| panic!("{broken} is in the set"));
        assert!(
            matches!(failure.1, NodeRead::Failed(_)),
            "a broken document is surfaced as a failure, not dropped"
        );
    }

    #[test]
    fn a_set_that_drops_nothing_reports_no_drops() {
        let r = repo("drops-none");
        let (empty, dropped) =
            list_with_drops(&Jj::new("jj", &r), None).expect("the set read reports");
        assert!(empty.is_empty(), "an empty fixture has no nodes");
        assert!(
            dropped.is_empty(),
            "an empty fixture drops nothing, got {dropped:?}"
        );

        let a = node(&r, decision());
        next(&r);
        let b = node(&r, decision());
        let (ordered, dropped) =
            list_with_drops(&Jj::new("jj", &r), None).expect("the set read reports");
        assert!(
            dropped.is_empty(),
            "every candidate owns its document, so none is dropped, got {dropped:?}"
        );
        assert_eq!(sorted(order_of(&ordered)), sorted(vec![a, b]));
        assert_eq!(order_of(&ordered), listing_order(&r));
    }
}
