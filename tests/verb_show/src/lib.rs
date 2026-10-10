//! Gate for M-V-show (the show verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch jj repo — a real
//! workspace whose node files are named by the change ids that own them — and the rendering
//! reaches node_load through the read, so this is a
//! crate of its own, needing serde_json and the schema validator. The compile-time
//! `SHOW_ROW` is the tie to M-route (a Verb row for the handler).

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
#[path = "../../../src/node_resolve.rs"]
mod node_resolve;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_show.rs"]
mod verb_show;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a `Verb` row for `show` (M-route, ynvwvzrm) compiles.
const SHOW_ROW: Verb<Context> = Verb {
    name: "show",
    run: crate::verb_show::run,
};

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::Value;

    use crate::context::resolve;
    use crate::refusal::Refusal;
    use crate::verb_show::run;

    /// A scratch jj repo: a real workspace with a `nodes/` dir. The node schema is carried in
    /// the binary (`include_str!`), never read from the repo.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deccy-show-{}-{}", std::process::id(), name));
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

    /// Write `body` as the node document the current change owns; return the owning change id
    /// (the id that names the document the read reaches).
    fn node(dir: &Path, body: &str) -> String {
        let id = head(dir);
        fs::write(dir.join("nodes").join(format!("{id}.json")), body).unwrap();
        id
    }

    fn a_document(marker: &str) -> String {
        format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"text","ref":"inline","content":"{marker}","rationale":"y"}}}}"#
        )
    }

    /// `deccy show <args>` through the real handler over the scratch repo.
    fn show(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
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
    fn each_valid_id_renders_its_document_as_pretty_json() {
        let dir = scratch("document");
        let stored = a_document("hello");
        let id = node(&dir, &stored);
        let lines = show(&dir, &[id.as_str()]).expect("a valid id shows");
        let printed = lines.join("\n");
        let round: Value = serde_json::from_str(&printed).expect("the output is JSON");
        assert_eq!(round, serde_json::from_str::<Value>(&stored).unwrap());
        assert!(printed.contains('\n'), "O8 renders pretty JSON, not one line");
        assert_eq!(crate::SHOW_ROW.name, "show");
    }

    #[test]
    fn one_output_per_valid_node() {
        let dir = scratch("many");
        let one = node(&dir, &a_document("one"));
        jj(&dir, &["new"]);
        let two = node(&dir, &a_document("two"));
        let both = show(&dir, &[one.as_str(), two.as_str()]).unwrap();
        let mut expected = show(&dir, &[one.as_str()]).unwrap();
        expected.extend(show(&dir, &[two.as_str()]).unwrap());
        assert_eq!(both, expected);
    }

    #[test]
    fn the_root_prints_a_marker_and_no_document() {
        let dir = scratch("root");
        // nodes/zzzzzzzz.json deliberately does not exist.
        let lines = show(&dir, &["zzzzzzzz"]).expect("the root shows");
        assert_eq!(lines.len(), 1, "the marker is one line");
        assert!(!lines[0].is_empty());
        assert!(
            !lines[0].starts_with('{'),
            "the root names no file, so no document is written: {:?}",
            lines[0]
        );
    }

    #[test]
    fn an_id_that_maps_to_nothing_refuses_naming_it() {
        let dir = scratch("unknown");
        for bad in ["abcdefgh", "NOPE", "sh-ow"] {
            match show(&dir, &[bad]) {
                Err(refusal) => assert_eq!(refusal.evidence(), [bad]),
                Ok(lines) => panic!("{bad:?} must refuse, got {lines:?}"),
            }
        }
    }

    #[test]
    fn a_node_whose_document_cannot_be_read_refuses_naming_it() {
        let dir = scratch("unreadable");
        // A real change owns the document, but the read's outcome is not a node document.
        let id = node(&dir, "{ not json");
        match show(&dir, &[id.as_str()]) {
            Err(refusal) => assert_eq!(refusal.evidence(), [id]),
            Ok(lines) => panic!("an unreadable document must refuse, got {lines:?}"),
        }
    }

    #[test]
    fn show_changes_nothing() {
        let dir = scratch("readonly");
        let id = node(&dir, &a_document("x"));
        let before = listing(&dir);
        let _ = show(&dir, &[id.as_str()]).unwrap();
        let _ = show(&dir, &["zzzzzzzz"]).unwrap();
        assert_eq!(listing(&dir), before);
    }

    #[test]
    fn help_is_the_verb_s_own_and_plain() {
        let dir = scratch("help");
        let lines = show(&dir, &["--help"]).expect("--help shows");
        assert!(!lines.is_empty(), "the verb carries its own help");
        assert!(
            lines.iter().all(|line| !line.contains('\u{1b}')),
            "help is plain UTF-8 with no styling escapes"
        );
    }

    #[test]
    fn an_id_less_invocation_refuses() {
        let dir = scratch("no-id");
        match show(&dir, &[]) {
            Err(refusal) => assert!(!refusal.cause().is_empty()),
            Ok(lines) => panic!("an id-less invocation must refuse, got {lines:?}"),
        }
    }
}
