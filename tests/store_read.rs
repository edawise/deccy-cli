//! Gate for M-store-read (read a node document — whole, absent, or failed).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parents
//! (jj, jjread); no shared test-support module. Plain `rustc --test`, std only — it runs a
//! real jj in a scratch repo.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/jjread.rs"]
mod jjread;
#[path = "../src/store_path.rs"]
mod store_path;
#[path = "../src/store_read.rs"]
mod store_read;

use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;
use store_path::Id;
use store_read::{read, read_node, Document, ReadError};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-store-read-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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

#[test]
fn a_document_at_the_revision_is_read_whole() {
    let dir = scratch("whole");
    jj(&dir, &["git", "init"]);
    std::fs::create_dir_all(dir.join("nodes")).unwrap();
    std::fs::write(dir.join("nodes/abcdefgh.json"), "{\"kind\":\"decision\"}\n").unwrap();
    jj(&dir, &["st"]);
    let jjrun = Jj::new("jj", dir.as_path());
    match read(&jjrun, "nodes/abcdefgh.json", "@") {
        Ok(Document::Whole(text)) => assert_eq!(text, "{\"kind\":\"decision\"}\n"),
        other => panic!("expected the whole document, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_not_present_reads_as_absence_not_a_failure() {
    let dir = scratch("absent");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    assert!(matches!(
        read(&jjrun, "nodes/abcdefgh.json", "@"),
        Ok(Document::Absent)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failure_other_than_absence_is_carried() {
    let dir = scratch("failed");
    jj(&dir, &["git", "init"]);
    let jjrun = Jj::new("jj", dir.as_path());
    assert!(matches!(
        read(&jjrun, "nodes/abcdefgh.json", "no-such-rev"),
        Err(ReadError::Failed(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_absent_binary_is_a_spawn_failure() {
    let dir = scratch("spawn");
    let jjrun = Jj::new("deccy-no-such-jj-binary", dir.as_path());
    assert!(matches!(
        read(&jjrun, "nodes/abcdefgh.json", "@"),
        Err(ReadError::Spawn(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_ids_document_is_read_at_the_change_it_names() {
    let dir = scratch("entry");
    jj(&dir, &["git", "init"]);
    std::fs::create_dir_all(dir.join("nodes")).unwrap();
    let id = jj(
        &dir,
        &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
    );
    std::fs::write(dir.join(format!("nodes/{id}.json")), "{\"kind\":\"decision\"}\n").unwrap();
    jj(&dir, &["st"]);
    let jjrun = Jj::new("jj", dir.as_path());
    match read_node(&jjrun, &Id::parse(&id).expect("a change id parses")) {
        Ok(Document::Whole(text)) => assert_eq!(text, "{\"kind\":\"decision\"}\n"),
        other => panic!("expected the id's whole document, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_id_with_no_document_reads_as_absence_through_the_entry() {
    let dir = scratch("entry-absent");
    jj(&dir, &["git", "init"]);
    let id = jj(
        &dir,
        &["log", "-r", "@", "--no-graph", "-T", "change_id.shortest(8)"],
    );
    let jjrun = Jj::new("jj", dir.as_path());
    assert!(matches!(
        read_node(&jjrun, &Id::parse(&id).expect("a change id parses")),
        Ok(Document::Absent)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
