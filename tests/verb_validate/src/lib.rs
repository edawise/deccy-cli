//! Gate for M-V-validate (the validate verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj repo — a real
//! workspace holding real node files — and the checks reach node_set, node_resolve, and
//! node_load, so this is a crate of its own, needing serde_json and the schema
//! validator. The compile-time `VALIDATE_ROW` is the tie to M-route (a Verb row for the
//! handler). The fourth check is driven the same way: the fixtures read jj's own order, then
//! assert the line an edge running against history produces and the silence of an edge that
//! runs with it. The fifth check is driven the same way again: the fixtures write changes whose
//! recorded diffs stray outside their own documents (xotxspwp), read jj's own diff summaries so
//! the expectation never comes from the handler, and assert the line per disagreeing path, the
//! silence of a graph whose changes stay inside their owning set, and that the run leaves the
//! tree it reported on exactly as it found it. A check that could not itself run is driven the
//! same way: the fixture refuses the custody read's `diff.files()` crossing (and nothing the
//! node set reads) and asserts one check-level line under O10 — the check's name, the stand-in,
//! the evidence — with the other four checks' findings beside it and the tree left as found.
//!
//! The accounting the pass owes the node store is driven the same way, with fixtures whose
//! changes touch `nodes/` and whose own documents are absent from it: the candidates jj lists
//! and the documents the fixture carries are read from the fixture, never from the handler, so
//! the claims about which change carries no document of its own and which the set holds are the
//! fixture's. One run then names every candidate exactly once — a change carrying no document of
//! its own by one `validity <id> <reason>` line, the nodes by custody or, when their diff stays
//! inside the set they own, by nothing at all — and a candidate so named is not thereby a node:
//! it is not resolved, not placed in history and given no owning set. The rest of the run is
//! asserted unchanged beside it: a document that is present but does not load stays validity's
//! node rather than becoming a drop as well, and nothing is repaired on the tree.

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
#[path = "../../../src/store_bulk.rs"]
mod store_bulk;
#[path = "../../../src/node_set.rs"]
mod node_set;
#[path = "../../../src/node_resolve.rs"]
mod node_resolve;
#[path = "../../../src/node_own.rs"]
mod node_own;
#[path = "../../../src/jj_custody.rs"]
mod jj_custody;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/state_root.rs"]
mod state_root;
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

    /// The order jj itself lists the changes touching `nodes/`, read straight from jj so an
    /// expectation about history never comes from the handler under test.
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

    /// Where history puts `id`: its index in jj's own listing, smallest first.
    fn position(dir: &Path, id: &str) -> usize {
        listing_order(dir)
            .iter()
            .position(|listed| listed == id)
            .unwrap_or_else(|| panic!("{id} is not in the listing"))
    }

    /// Make `id` the working-copy change, so a document written next belongs to that change —
    /// the way a fixture writes an edge onto an older change and history has moved past it.
    fn edit_at(dir: &Path, id: &str) {
        jj(dir, &["edit", id]);
    }

    /// Write `body` onto the change `id` already owns and snapshot it into that change.
    fn rewrite_older(dir: &Path, id: &str, body: &str) {
        edit_at(dir, id);
        write_node(dir, id, body);
        jj(dir, &["status"]);
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

    /// The real jj behind a counting shim, found before PATH is changed.
    fn real_jj() -> PathBuf {
        let path = std::env::var_os("PATH").expect("PATH is set");
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("jj");
            if candidate.is_file() {
                return candidate;
            }
        }
        panic!("jj must be on PATH");
    }

    /// Run `body` with a counting `jj` first on PATH: every invocation whose physical cwd is
    /// `root` is logged, then the real jj runs. Returns the body's value and the logged calls.
    fn counting<T>(root: &Path, name: &str, body: impl FnOnce() -> T) -> (T, Vec<String>) {
        use std::os::unix::fs::PermissionsExt;

        let real = real_jj();
        let shim_dir = std::env::temp_dir().join(format!(
            "deccy-validate-shim-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&shim_dir);
        fs::create_dir_all(&shim_dir).unwrap();
        let log = shim_dir.join("calls.log");
        let script = shim_dir.join("jj");
        let root = root.canonicalize().unwrap();
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nif [ \"$(pwd -P)\" = \"{}\" ]; then printf '%s\\n' \"$*\" >> \"{}\"; fi\nexec \"{}\" \"$@\"\n",
                root.display(),
                log.display(),
                real.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();

        let original = std::env::var_os("PATH");
        let mut entries = vec![shim_dir.clone()];
        entries.extend(std::env::split_paths(&original.clone().unwrap()));
        std::env::set_var("PATH", std::env::join_paths(entries).unwrap());
        let value = body();
        match original {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
        let calls = fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        (value, calls)
    }

    fn file_shows(calls: &[String]) -> usize {
        calls
            .iter()
            .filter(|call| call.starts_with("file show"))
            .count()
    }

    /// Every `jj log` crossing: the readings of order the invocation makes.
    fn logs(calls: &[String]) -> usize {
        calls.iter().filter(|call| call.starts_with("log ")).count()
    }

    /// PATH is process-global, so the counting bodies are serialized: one count at a time.
    static COUNTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn counting_now() -> std::sync::MutexGuard<'static, ()> {
        COUNTING.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A five-node graph: the same node count either way, but the edge-heavy one carries six
    /// decision edges and an evaluation that covers them.
    fn counting_graph(dir: &Path, edges: bool) {
        let a = node(dir, "a", &decision("a", &["zzzzzzzz"]));
        if !edges {
            for index in 0..4 {
                let id = format!("d{index}");
                node(dir, &id, &decision(&id, &["zzzzzzzz"]));
            }
            return;
        }
        let b = node(dir, "b", &decision("b", &[a.as_str()]));
        let c = node(dir, "c", &decision("c", &[a.as_str(), b.as_str()]));
        let d = node(dir, "d", &decision("d", &[a.as_str(), b.as_str(), c.as_str()]));
        node(
            dir,
            "e",
            &evaluation(
                "e",
                &[b.as_str(), c.as_str(), d.as_str()],
                &[a.as_str(), b.as_str(), c.as_str()],
            ),
        );
    }

    #[test]
    fn the_crossing_count_does_not_grow_with_the_edge_count() {
        let _counting = counting_now();
        let thin = scratch("crossings-thin");
        counting_graph(&thin, false);
        let thick = scratch("crossings-thick");
        counting_graph(&thick, true);

        let (thin_lines, thin_calls) =
            counting(&thin, "thin", || validate(&thin, &[]).expect("validate runs"));
        let (thick_lines, thick_calls) =
            counting(&thick, "thick", || validate(&thick, &[]).expect("validate runs"));

        assert_eq!(
            file_shows(&thin_calls),
            file_shows(&thick_calls),
            "the same node count must make the same number of file-show crossings: thin={thin_calls:?} thick={thick_calls:?}"
        );
        assert_eq!(thin_lines, thick_lines, "the check lines are unchanged");
    }

    /// `count` nodes, each depending on nothing but the root: every edge is out of the topology
    /// check's scope except the root exemption, so the graph is clean at any size.
    fn root_children(dir: &Path, count: usize) {
        for index in 0..count {
            let id = format!("n{index}");
            node(dir, &id, &decision(&id, &["zzzzzzzz"]));
        }
    }

    /// The topology lines of one run, the only lines an inverted edge may add (S-topology).
    fn topology(lines: &[String]) -> Vec<&String> {
        lines
            .iter()
            .filter(|line| line.starts_with("topology "))
            .collect()
    }

    #[test]
    fn a_history_ordered_graph_reports_no_topology_line() {
        let dir = scratch("topology-clean");
        let a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        let b = node(&dir, "b", &decision("b", &[a.as_str()]));
        let _ab = node(&dir, "ab", &evaluation("ab", &[b.as_str()], &[a.as_str()]));
        let c = node(&dir, "c", &decision("c", &[b.as_str()]));
        let _bc = node(&dir, "bc", &evaluation("bc", &[c.as_str()], &[b.as_str()]));
        // The fixture is what the check assumes: jj lists each dependent before its prerequisite.
        assert!(
            position(&dir, &c) < position(&dir, &b) && position(&dir, &b) < position(&dir, &a),
            "history runs newest to oldest: {:?}",
            listing_order(&dir)
        );

        let lines = validate(&dir, &[]).expect("a clean graph validates");
        assert!(
            topology(&lines).is_empty(),
            "every edge runs newest to oldest, so there is nothing to report: {lines:?}"
        );
        assert!(lines.is_empty(), "and no other check fails either: {lines:?}");
    }

    #[test]
    fn an_inverted_depends_on_edge_is_one_topology_line() {
        let dir = scratch("topology-inverted-depends");
        let dependent = node(&dir, "dependent", &decision("dependent", &["zzzzzzzz"]));
        let prerequisite = node(
            &dir,
            "prerequisite",
            &decision("prerequisite", &["zzzzzzzz"]),
        );
        let _cover = node(
            &dir,
            "cover",
            &evaluation("cover", &[dependent.as_str()], &[prerequisite.as_str()]),
        );
        // The dependent is the older change and names the newer one: the coverage check still
        // passes, so history is the only thing the edge disagrees with.
        rewrite_older(&dir, &dependent, &decision("dependent", &[prerequisite.as_str()]));
        assert!(
            position(&dir, &prerequisite) < position(&dir, &dependent),
            "the fixture is inverted: history lists the target before the dependent: {:?}",
            listing_order(&dir)
        );

        let lines = validate(&dir, &[]).expect("validate always runs");
        assert_eq!(
            lines,
            vec![format!("topology {dependent} {prerequisite}")],
            "one line naming the dependent and the newer target, and nothing else: {lines:?}"
        );
    }

    #[test]
    fn an_inverted_evaluates_edge_is_one_topology_line() {
        let dir = scratch("topology-inverted-evaluates");
        let older = node(&dir, "older", &decision("older", &["zzzzzzzz"]));
        let checker = node(
            &dir,
            "checker",
            &evaluation("checker", &[older.as_str()], &[older.as_str()]),
        );
        let evaluated = node(&dir, "evaluated", &decision("evaluated", &["zzzzzzzz"]));
        // The evaluation is the older change and evaluates a node history puts after it; an
        // `evaluates` edge carries no coverage duty, so the inversion is its only failure.
        rewrite_older(
            &dir,
            &checker,
            &evaluation("checker", &[evaluated.as_str()], &[older.as_str()]),
        );
        assert!(
            position(&dir, &evaluated) < position(&dir, &checker),
            "the fixture is inverted: history lists the evaluated node first: {:?}",
            listing_order(&dir)
        );

        let lines = validate(&dir, &[]).expect("validate always runs");
        assert_eq!(
            lines,
            vec![format!("topology {checker} {evaluated}")],
            "every edge is checked, `evaluates` included: {lines:?}"
        );
    }

    #[test]
    fn a_target_the_set_cannot_place_is_never_a_topology_finding() {
        let dir = scratch("topology-unplaceable");
        let _a = node(&dir, "a", &decision("a", &["deadbeef"]));
        let lines = validate(&dir, &[]).expect("validate always runs");
        assert!(
            topology(&lines).is_empty(),
            "a target with no change has no position to compare: {lines:?}"
        );
        assert_eq!(lines.len(), 1, "resolution is the only line: {lines:?}");
        assert!(
            lines[0].starts_with("resolution "),
            "and it is resolution's: {lines:?}"
        );
    }

    #[test]
    fn the_topology_check_adds_no_crossing_as_the_graph_grows() {
        let _counting = counting_now();
        let small = scratch("topology-crossings-small");
        root_children(&small, 3);
        let large = scratch("topology-crossings-large");
        root_children(&large, 9);

        let (small_lines, small_calls) = counting(&small, "topology-small", || {
            validate(&small, &[]).expect("validate runs")
        });
        let (large_lines, large_calls) = counting(&large, "topology-large", || {
            validate(&large, &[]).expect("validate runs")
        });

        assert!(
            small_lines.is_empty() && large_lines.is_empty(),
            "both graphs are history-ordered: small={small_lines:?} large={large_lines:?}"
        );
        assert_eq!(
            logs(&small_calls),
            logs(&large_calls),
            "the graph's own listing is the only order read, whatever its size: small={small_calls:?} large={large_calls:?}"
        );
        assert_eq!(
            logs(&large_calls), 3,
            "one listing for the set, one mapping for the set read, and one custody crossing; no check adds a fourth: {large_calls:?}"
        );
        assert_eq!(
            small_calls.len(),
            large_calls.len(),
            "no crossing is made per node or per edge: small={small_calls:?} large={large_calls:?}"
        );
    }

    #[test]
    fn help_names_the_fourth_check() {
        let dir = scratch("topology-help");
        let lines = validate(&dir, &["--help"]).expect("--help shows");
        let help = lines.join("\n").to_lowercase();
        assert!(
            help.contains("topology"),
            "the help names the check it reports: {help:?}"
        );
    }

    /// The paths jj says `id`'s recorded diff touched, read straight from jj so a fixture's
    /// expectation about history never comes from the handler under test. Both sides of an entry
    /// count, as they do for the check's own read (S-read-custody).
    fn touched(dir: &Path, id: &str) -> Vec<String> {
        let listing = jj(
            dir,
            &[
                "log",
                "--no-graph",
                "-r",
                id,
                "-T",
                "diff.files().map(|f| f.source().path() ++ \"\\n\" ++ f.target().path() ++ \"\\n\").join(\"\")",
            ],
        );
        let mut paths: Vec<String> = Vec::new();
        for path in listing.lines().map(str::trim).filter(|path| !path.is_empty()) {
            if !paths.iter().any(|seen| seen == path) {
                paths.push(path.to_string());
            }
        }
        paths.sort();
        paths
    }

    /// Every change jj reports, with the paths its recorded diff touches: the repo's history as
    /// jj itself reports it, so a claim that nothing was repaired is checked against jj.
    fn history(dir: &Path) -> String {
        jj(
            dir,
            &[
                "log",
                "--no-graph",
                "-r",
                "::",
                "-T",
                "change_id.shortest(8) ++ \" \" ++ diff.files().map(|f| f.source().path() ++ \"->\" ++ f.target().path()).join(\",\") ++ \"\\n\"",
            ],
        )
    }

    /// Write `body` at the repo-relative `path`, creating its directory.
    fn write_file(dir: &Path, path: &str, body: &str) {
        let full = dir.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, body).unwrap();
    }

    /// A change whose node document owns nothing but itself — inline ref (xzxtxymq) — and whose
    /// recorded diff also touches `extra`: every one of those paths is a path the node does not
    /// own, so the change's diff has strayed outside its owning set (xotxspwp).
    fn straying(dir: &Path, name: &str, extra: &[&str]) -> String {
        jj(dir, &["new"]);
        jj(dir, &["describe", "-m", name]);
        let id = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        write_node(dir, &id, &decision(&id, &["zzzzzzzz"]));
        for path in extra {
            write_file(dir, path, name);
        }
        jj(dir, &["status"]);
        id
    }

    /// A change whose node document lists `refs` and whose recorded diff touches its own document
    /// and exactly those paths: the owning set is `content.ref`'s paths (xzxtxymq), so touching
    /// them is not straying.
    fn owning(dir: &Path, name: &str, refs: &[&str]) -> String {
        jj(dir, &["new"]);
        jj(dir, &["describe", "-m", name]);
        let id = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        write_node(
            dir,
            &id,
            &format!(
                r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"rust","ref":[{}],"rationale":"y"}}}}"#,
                ids(refs)
            ),
        );
        for path in refs {
            write_file(dir, path, name);
        }
        jj(dir, &["status"]);
        id
    }

    fn sorted(mut values: Vec<String>) -> Vec<String> {
        values.sort();
        values
    }

    /// A change that touches the node store without owning a document of its own: it writes
    /// `files` into its working copy and carries no `nodes/<its own id>.json` — a buffer editing
    /// a sibling's document, a change carrying stray files there (S2-set).
    fn carrier(dir: &Path, name: &str, files: &[(&str, &str)]) -> String {
        jj(dir, &["new"]);
        jj(dir, &["describe", "-m", name]);
        let id = jj(
            dir,
            &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
        );
        for (path, body) in files {
            write_file(dir, path, body);
        }
        jj(dir, &["status"]);
        id
    }

    /// The document a buffer writes over a sibling's: valid, and not the bytes the sibling's own
    /// change recorded, so the buffer's diff really touches the sibling's document.
    fn edited(id: &str) -> String {
        format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"{id}","rationale":"edited by the buffer"}}}}"#
        )
    }

    /// The candidates touching `nodes/` that carry no document of their own, from the fixture
    /// alone: jj's own candidate listing minus the changes owning `nodes/<their id>.json`.
    fn without_a_document(dir: &Path) -> Vec<String> {
        sorted(
            listing_order(dir)
                .into_iter()
                .filter(|id| !dir.join("nodes").join(format!("{id}.json")).is_file())
                .collect(),
        )
    }

    /// The ids a run named in the id position of the lines a check leads with (O10's second
    /// position), in the order the run reported them.
    fn named(lines: &[String], check: &str) -> Vec<String> {
        lines
            .iter()
            .filter(|line| line.split_whitespace().next() == Some(check))
            .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
            .collect()
    }

    /// The custody lines: the fifth check's findings, recognised by the check name the line leads
    /// with under O10's `<check> <id> <reason>` shape. The name's wording belongs to the verb, so
    /// the test asks only that the check it leads with is custody's.
    fn custody(lines: &[String]) -> Vec<&String> {
        lines
            .iter()
            .filter(|line| {
                line.split_whitespace()
                    .next()
                    .map_or(false, |check| check.to_lowercase().contains("custody"))
            })
            .collect()
    }

    #[test]
    fn a_stray_path_is_one_custody_line_naming_the_change_and_the_path() {
        let dir = scratch("custody-stray");
        let clean = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        let stray = straying(&dir, "b", &["docs/b.md"]);
        // The fixture is what the check assumes: `b`'s recorded diff touches its own document and
        // `docs/b.md`, which its node — inline ref, so the document alone — does not own.
        assert_eq!(touched(&dir, &clean), vec![format!("nodes/{clean}.json")]);
        assert_eq!(
            touched(&dir, &stray),
            vec!["docs/b.md".to_string(), format!("nodes/{stray}.json")],
            "the fixture strays: {:?}",
            history(&dir)
        );

        let lines = validate(&dir, &[]).expect("validate always runs");
        let found = custody(&lines);
        assert_eq!(
            found.len(),
            1,
            "one line for the one disagreeing path: {lines:?}"
        );
        let fields: Vec<&str> = found[0].split_whitespace().collect();
        assert_eq!(
            fields.get(1).copied(),
            Some(stray.as_str()),
            "O10's shape puts the change the line is about second: {lines:?}"
        );
        assert!(
            found[0].contains("docs/b.md"),
            "and the line's reason names the path: {lines:?}"
        );
        assert_eq!(lines.len(), 1, "and nothing else disagrees: {lines:?}");
    }

    #[test]
    fn every_path_a_change_does_not_own_gets_its_own_custody_line() {
        let dir = scratch("custody-paths");
        let stray = straying(&dir, "b", &["docs/one.md", "src/two.rs"]);
        assert_eq!(
            touched(&dir, &stray),
            vec![
                "docs/one.md".to_string(),
                format!("nodes/{stray}.json"),
                "src/two.rs".to_string(),
            ],
            "the fixture strays onto two paths: {:?}",
            history(&dir)
        );

        let lines = validate(&dir, &[]).expect("validate always runs");
        let found = custody(&lines);
        assert_eq!(
            found.len(),
            2,
            "one line per disagreeing path: {lines:?}"
        );
        assert!(
            found.iter().any(|line| line.contains("docs/one.md")),
            "the first path is named: {lines:?}"
        );
        assert!(
            found.iter().any(|line| line.contains("src/two.rs")),
            "the second path is named: {lines:?}"
        );
        assert!(
            found
                .iter()
                .all(|line| line.split_whitespace().nth(1) == Some(stray.as_str())),
            "every line names the change that does not own the path: {lines:?}"
        );
        assert_eq!(lines.len(), 2, "and nothing else disagrees: {lines:?}");
    }

    #[test]
    fn a_graph_whose_changes_stay_inside_their_own_paths_reports_no_custody_line() {
        let dir = scratch("custody-clean");
        let _a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        let owning = owning(&dir, "b", &["src/owned.rs", "tests/owned.rs"]);
        // The fixture draws the owning set from the node document's own ref list (xzxtxymq): the
        // change touches those paths and its document, and nothing else.
        assert_eq!(
            touched(&dir, &owning),
            vec![
                format!("nodes/{owning}.json"),
                "src/owned.rs".to_string(),
                "tests/owned.rs".to_string(),
            ],
            "the fixture stays inside its owning set: {:?}",
            history(&dir)
        );

        let lines = validate(&dir, &[]).expect("validate always runs");
        assert!(
            custody(&lines).is_empty(),
            "a path its own document lists is owned, not strayed onto: {lines:?}"
        );
        assert!(lines.is_empty(), "and no other check fails: {lines:?}");
    }

    #[test]
    fn the_fifth_check_reports_and_does_not_repair() {
        let dir = scratch("custody-no-repair");
        let stray = straying(&dir, "b", &["docs/b.md"]);
        let document = dir.join("nodes").join(format!("{stray}.json"));
        let document_before = fs::read_to_string(&document).unwrap();
        let file_before = fs::read_to_string(dir.join("docs/b.md")).unwrap();
        let history_before = history(&dir);
        let touched_before = touched(&dir, &stray);

        let lines = validate(&dir, &[]).expect("validate always runs");
        assert_eq!(
            custody(&lines).len(),
            1,
            "the disagreement is reported: {lines:?}"
        );

        assert_eq!(
            history(&dir),
            history_before,
            "no change is rewritten, squashed or added so the report would stop disagreeing"
        );
        assert_eq!(
            touched(&dir, &stray),
            touched_before,
            "the offending diff still touches the path its node does not own"
        );
        assert_eq!(
            fs::read_to_string(&document).unwrap(),
            document_before,
            "no node document is amended so its diff would fit"
        );
        assert_eq!(
            fs::read_to_string(dir.join("docs/b.md")).unwrap(),
            file_before,
            "no edit is moved off the path it strayed onto"
        );
    }

    /// The evidence a jj that cannot produce the custody listing reports. O10's third position
    /// carries what the check's crossing produced, so the fixture fixes what jj says and the
    /// run has to carry it through.
    const CUSTODY_EVIDENCE: &str = "error: the diff summary could not be read";

    /// Run `body` with a `jj` first on PATH that fails every crossing whose template carries the
    /// custody listing's `diff.files()` summary, and runs the real jj for every other crossing.
    ///
    /// The two reads are distinct invocations: the set's candidate listing is
    /// `log -r files("nodes/") -T change_id…`, the change-to-commit mapping is `log -r <ids> -T
    /// change_id… commit_id`, and only S-read-custody's crossing carries `diff.files()` (`-r ::`,
    /// and the one-at-a-time fallback's per-change listing). So the shim fails the custody read —
    /// the listing and its fallback — and nothing the node set reads, which is what makes the run
    /// reach the fifth check at all. Returns the body's value and the crossings seen, each with
    /// whether it was refused.
    fn custody_read_fails<T>(
        root: &Path,
        name: &str,
        body: impl FnOnce() -> T,
    ) -> (T, Vec<(bool, String)>) {
        use std::os::unix::fs::PermissionsExt;

        let real = real_jj();
        let shim_dir = std::env::temp_dir().join(format!(
            "deccy-validate-refuse-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&shim_dir);
        fs::create_dir_all(&shim_dir).unwrap();
        let log = shim_dir.join("calls.log");
        let script = shim_dir.join("jj");
        let root = root.canonicalize().unwrap();
        fs::write(
            &script,
            format!(
                r#"#!/bin/sh
here=0
if [ "$(pwd -P)" = "{root}" ]; then here=1; fi
if [ "$here" = 1 ]; then
  case "$*" in
    *diff.files*) printf 'FAIL %s\n' "$*" >> "{log}"; printf '%s\n' "{evidence}" >&2; exit 1 ;;
    *) printf 'OK %s\n' "$*" >> "{log}" ;;
  esac
fi
exec "{real}" "$@"
"#,
                root = root.display(),
                log = log.display(),
                evidence = CUSTODY_EVIDENCE,
                real = real.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();

        let original = std::env::var_os("PATH");
        let mut entries = vec![shim_dir.clone()];
        entries.extend(std::env::split_paths(&original.clone().unwrap()));
        std::env::set_var("PATH", std::env::join_paths(entries).unwrap());
        let value = body();
        match original {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
        let crossings = fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(|line| match line.strip_prefix("FAIL ") {
                Some(call) => (true, call.to_string()),
                None => (false, line.trim_start_matches("OK ").to_string()),
            })
            .collect();
        (value, crossings)
    }

    #[test]
    fn a_custody_read_that_cannot_run_is_one_check_line_and_not_a_refusal() {
        let _counting = counting_now();
        let dir = scratch("custody-unreadable");
        // Two of the other four checks fail on this graph, so the run has findings to report
        // beside the line for the check that never ran.
        let invalid = node(&dir, "invalid", r#"{"kind":"decision"}"#);
        let unresolved = node(&dir, "unresolved", &decision("unresolved", &["deadbeef"]));

        let (outcome, crossings) =
            custody_read_fails(&dir, "custody-unreadable", || validate(&dir, &[]));

        // The fixture is what the addition assumes: the check could not run and the set could.
        assert!(
            crossings
                .iter()
                .any(|(failed, call)| !*failed && call.contains("files(\"nodes/\")")),
            "the set's candidate listing ran, so the run reached the checks: {crossings:?}"
        );
        assert!(
            crossings
                .iter()
                .any(|(failed, call)| *failed && call.contains("diff.files")),
            "the custody listing is the crossing that failed: {crossings:?}"
        );
        assert!(
            crossings
                .iter()
                .all(|(failed, call)| !*failed || call.contains("diff.files")),
            "no crossing the node set makes was refused: {crossings:?}"
        );

        let lines = outcome.expect(
            "a check that could not itself run does not refuse the invocation: the other four \
             checks' findings are reported beside its line",
        );

        // O10's line for a check that could not run: the check's name, the stand-in, the
        // evidence — one line for the check, not one per change it failed to reach.
        let found = custody(&lines);
        assert_eq!(
            found.len(),
            1,
            "one line for the check that could not run: {lines:?}"
        );
        let fields: Vec<&str> = found[0].split_whitespace().collect();
        assert!(
            fields.len() >= 3,
            "the line holds the same three positions as a failure line: {lines:?}"
        );
        assert_eq!(
            fields[1], "-",
            "the id position carries O10's stand-in: `custody - <jj evidence>`: {lines:?}"
        );
        assert!(
            fields[1] != invalid && fields[1] != unresolved,
            "no single node or change is implicated, so the id is not one of them: {lines:?}"
        );
        assert!(
            fields[2..].join(" ").contains(CUSTODY_EVIDENCE),
            "the third position carries the evidence the check produced: {lines:?}"
        );

        // The other four checks still report what they found in the same run.
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with(&format!("validity {invalid}"))),
            "the invalid node is still a validity line: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("resolution ") && line.contains("deadbeef")),
            "the unresolved target is still a resolution line: {lines:?}"
        );
        assert_eq!(
            lines.len(),
            3,
            "the two failures and the one could-not-run line, and nothing else: {lines:?}"
        );
    }

    #[test]
    fn a_custody_read_that_cannot_run_leaves_the_tree_as_it_found_it() {
        let _counting = counting_now();
        let dir = scratch("custody-unreadable-no-change");
        let _a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        // A change whose diff strays outside its own node: with a read, the fifth check would
        // report it. The run must not reach for the tree to make the report cleaner either way.
        let stray = straying(&dir, "b", &["docs/b.md"]);
        let document = dir.join("nodes").join(format!("{stray}.json"));
        let document_before = fs::read_to_string(&document).unwrap();
        let file_before = fs::read_to_string(dir.join("docs/b.md")).unwrap();
        let history_before = history(&dir);
        let listing_before = listing(&dir);

        let (outcome, _) = custody_read_fails(&dir, "custody-unreadable-no-change", || {
            validate(&dir, &[])
        });
        let lines = outcome.expect("a check that could not itself run does not refuse the run");
        assert_eq!(
            custody(&lines).len(),
            1,
            "the check that could not run is reported: {lines:?}"
        );
        assert_eq!(
            lines.len(),
            1,
            "and it reports no finding it never read: {lines:?}"
        );

        assert_eq!(
            listing(&dir),
            listing_before,
            "no document is added or removed by the run"
        );
        assert_eq!(
            history(&dir),
            history_before,
            "no change is rewritten, squashed or added"
        );
        assert_eq!(
            fs::read_to_string(&document).unwrap(),
            document_before,
            "no node document is amended"
        );
        assert_eq!(
            fs::read_to_string(dir.join("docs/b.md")).unwrap(),
            file_before,
            "no edit is moved off the path it strayed onto"
        );
    }

    #[test]
    fn help_names_the_five_checks() {
        let dir = scratch("custody-help");
        let lines = validate(&dir, &["--help"]).expect("--help shows");
        let help = lines.join("\n").to_lowercase();
        for check in ["validity", "resolution", "coverage", "topology"] {
            assert!(
                help.contains(check),
                "the help still names the {check} check: {help:?}"
            );
        }
        assert!(
            help.contains("custody"),
            "and names the fifth check, content custody, with the other four: {help:?}"
        );
    }

    #[test]
    fn a_change_carrying_no_document_of_its_own_is_one_validity_line_naming_it() {
        let dir = scratch("accounting-drops");
        let a = node(&dir, "a", &decision("a", &["zzzzzzzz"]));
        // A buffer that touches `nodes/` only by editing a sibling's document, and a change that
        // carries stray files there: neither owns a `nodes/<its own id>.json` (S2-set).
        let sibling = format!("nodes/{a}.json");
        let edited_a = edited("a");
        let buffer = carrier(&dir, "buffer", &[(sibling.as_str(), edited_a.as_str())]);
        let stray = carrier(
            &dir,
            "stray",
            &[
                ("nodes/not-a-node.json", "{}\n"),
                ("nodes/also-not.json", "{}\n"),
            ],
        );
        // The fixture is what the addition assumes: both are candidates for `nodes/`, and
        // neither carries a document of its own.
        let drops = without_a_document(&dir);
        assert_eq!(
            drops,
            sorted(vec![buffer.clone(), stray.clone()]),
            "two candidates carry no document of their own: listing {:?}, documents {:?}",
            listing_order(&dir),
            listing(&dir)
        );
        assert_eq!(
            touched(&dir, &buffer),
            vec![format!("nodes/{a}.json")],
            "the buffer's diff touches a path its own change does not own: {:?}",
            history(&dir)
        );

        let lines = validate(&dir, &[]).expect("validate always runs");

        // Every candidate is accounted for, and the ones carrying no document of their own are
        // validity's: named by it, spelled the way the set spells the ids it reports, once each
        // however many paths the change touched.
        let validity = named(&lines, "validity");
        assert_eq!(
            sorted(validity.clone()),
            drops,
            "one validity line per candidate carrying no document, each named once: {lines:?}"
        );
        assert_eq!(
            lines.len(),
            validity.len(),
            "the two drops are the whole report; no candidate is accounted for twice: {lines:?}"
        );
        for line in &lines {
            assert!(
                line.split_whitespace().count() >= 3,
                "the drop's line keeps O10's `<check> <id> <reason>` shape: {line:?}"
            );
        }
        // A dropped candidate has no owning set, so custody attributes nothing for it: the
        // buffer's diff touches a sibling's document, and a set invented for the buffer would
        // report that path as strayed onto.
        assert!(
            custody(&lines).is_empty(),
            "a change with no document of its own is validity's, not given a set by custody: {lines:?}"
        );
    }

    #[test]
    fn every_candidate_touching_the_node_store_is_accounted_for_once_in_one_run() {
        let dir = scratch("accounting-all");
        // One change of each kind the node store can meet: a node that stays inside its own set,
        // a buffer editing a sibling's document, a change carrying stray files under `nodes/`,
        // a node whose diff strays outside its set, and a node whose document is there but does
        // not load.
        let clean = node(&dir, "clean", &decision("clean", &["zzzzzzzz"]));
        let sibling = format!("nodes/{clean}.json");
        let edited_clean = edited("clean");
        let buffer = carrier(&dir, "buffer", &[(sibling.as_str(), edited_clean.as_str())]);
        let stray = carrier(
            &dir,
            "stray",
            &[
                ("nodes/not-a-node.json", "{}\n"),
                ("nodes/also-not.json", "{}\n"),
            ],
        );
        let aboard = straying(&dir, "aboard", &["docs/aboard.md"]);
        let broken = node(&dir, "broken", "{ not json");

        let drops = without_a_document(&dir);
        assert_eq!(
            drops,
            sorted(vec![buffer.clone(), stray.clone()]),
            "the fixture drops exactly the buffer and the stray change: listing {:?}, \
             documents {:?}",
            listing_order(&dir),
            listing(&dir)
        );
        assert_eq!(
            touched(&dir, &aboard),
            vec!["docs/aboard.md".to_string(), format!("nodes/{aboard}.json")],
            "and one node's diff strays outside the set its own document gives it: {:?}",
            history(&dir)
        );
        let listing_before = listing(&dir);
        let history_before = history(&dir);

        let lines = validate(&dir, &[]).expect("validate always runs");

        // Validity names the candidates carrying no document of their own, and keeps the node
        // whose document is present but did not load: a drop, and a failure, are different
        // things and each change is named once.
        let mut expected = drops.clone();
        expected.push(broken.clone());
        assert_eq!(
            sorted(named(&lines, "validity")),
            sorted(expected),
            "the drops and the failing document are validity's, each once: {lines:?}"
        );
        let found = custody(&lines);
        assert_eq!(
            found.len(),
            1,
            "the one node whose diff strays is one custody line: {lines:?}"
        );
        assert_eq!(
            found[0].split_whitespace().nth(1),
            Some(aboard.as_str()),
            "naming the change that strayed: {lines:?}"
        );
        assert!(
            found[0].contains("docs/aboard.md"),
            "and the path it strayed onto: {lines:?}"
        );
        assert!(
            named(&lines, "resolution").is_empty(),
            "nothing the graph records fails to resolve: {lines:?}"
        );
        assert!(
            named(&lines, "coverage").is_empty(),
            "every decision edge here is the root's, which is out of scope: {lines:?}"
        );
        assert!(
            named(&lines, "topology").is_empty(),
            "history runs newest to oldest throughout: {lines:?}"
        );
        assert_eq!(
            lines.len(),
            4,
            "three validity lines and one custody line, and no candidate reported twice: {lines:?}"
        );

        // The accounting, in one run: the changes the run named and the one it was entitled to
        // leave silent are exactly the candidates jj lists, each appearing once. A node whose
        // diff stays inside the set its document gives it is the silent one.
        let mut accounted = named(&lines, "validity");
        accounted.extend(named(&lines, "custody"));
        let mut once = accounted.clone();
        once.sort();
        once.dedup();
        assert_eq!(
            once.len(),
            accounted.len(),
            "no candidate is accounted for by two lines: {lines:?}"
        );
        let mut candidates: Vec<String> = listing_order(&dir)
            .into_iter()
            .filter(|id| id != &clean)
            .collect();
        candidates.sort();
        assert_eq!(
            once, candidates,
            "every candidate but the one that stayed inside its own set is named: {lines:?}"
        );
        assert!(
            !accounted.contains(&clean),
            "a node whose diff touches only paths it owns is attributed and silent: {lines:?}"
        );

        // Nothing is repaired: no document is created for a change that carried none, no diff is
        // moved and no change is rewritten (xotxspwp's refusal of repair).
        assert_eq!(
            listing(&dir),
            listing_before,
            "no document is created for a change that carries none, and none is removed"
        );
        assert_eq!(
            history(&dir),
            history_before,
            "no change is rewritten, squashed or added so a report would stop disagreeing"
        );
    }

    #[test]
    fn a_candidate_carrying_no_document_of_its_own_is_not_a_node_the_checks_place() {
        let dir = scratch("accounting-not-nodes");
        let dropped = carrier(&dir, "dropped", &[("nodes/not-a-node.json", "{}\n")]);
        // A decision naming the drop as its prerequisite: were the drop put back in the set to be
        // named, the target would resolve and history would be the only thing left to report.
        let dependent = node(&dir, "dependent", &decision("dependent", &[dropped.as_str()]));
        assert_eq!(
            without_a_document(&dir),
            vec![dropped.clone()],
            "the fixture carries one candidate with no document of its own: {:?}",
            listing_order(&dir)
        );

        let lines = validate(&dir, &[]).expect("validate always runs");

        assert_eq!(
            named(&lines, "validity"),
            vec![dropped.clone()],
            "the drop is validity's, named once: {lines:?}"
        );
        let resolution = named(&lines, "resolution");
        assert_eq!(
            resolution,
            vec![dependent.clone()],
            "the target is not in the set, so resolution answers for the dependent: {lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.starts_with(&format!("resolution {dependent}"))
                && line.contains(&dropped)),
            "and the line names the target it could not place: {lines:?}"
        );
        assert!(
            named(&lines, "coverage").is_empty(),
            "a target that is not a decision carries no coverage duty: {lines:?}"
        );
        assert!(
            named(&lines, "topology").is_empty(),
            "a target the set does not hold has no position to compare: {lines:?}"
        );
        assert_eq!(
            lines.len(),
            2,
            "the drop's validity line and the dependent's resolution line, and nothing else: {lines:?}"
        );
    }
}
