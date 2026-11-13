//! Gate for U3: the content-custody read (`src/jj_custody.rs`).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (jj); no shared test-support module. Plain `rustc --test`, std only.
//!
//! The read is expected to cross the process boundary once — a single spawned `jj log` whose
//! template carries each change's diff summary — and to return every change the listing
//! reports, each paired with the paths that change touched. The crossing itself is observed:
//! a recording stand-in stands in front of a real jj in a scratch repo, so how many processes
//! the read spawns, what it asks the one it spawns, and what it pairs are all visible here.

#[path = "../../src/jj.rs"]
mod jj;
#[path = "../../src/jj_custody.rs"]
mod jj_custody;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use jj::Jj;

/// A throwaway scratch root with a `repo` (the jj repo) and a `bin` (stand-ins live outside
/// the repo, so they never show up as paths the working copy touched).
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "deccy-jj-custody-{}-{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("repo")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    root
}

/// The real jj on PATH: the stand-in hands every crossing to it, so the output the read parses
/// is jj's own.
fn real_jj() -> PathBuf {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        if dir.is_empty() {
            continue;
        }
        let candidate = Path::new(dir).join("jj");
        if candidate.is_file() {
            return fs::canonicalize(&candidate).unwrap_or(candidate);
        }
    }
    panic!("jj is on PATH for the gate");
}

/// Run a real jj in `dir`, failing the test on a bad exit.
fn jj(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("jj")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("jj is on PATH for the gate");
    assert!(
        out.status.success(),
        "jj {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Install an executable `jj` stand-in at `dir/name` and return its path. It records one byte
/// per invocation in `<path>.calls` and the arguments of each in `<path>.argv`, then runs
/// `body`.
fn stand_in(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    let script = format!(
        "#!/bin/sh\nprintf x >> \"$0.calls\"\nprintf '%s\\n' \"$*\" >> \"$0.argv\"\n{}\n",
        body
    );
    fs::write(&path, script).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

/// Whatever the stand-in recorded under `<path>.<suffix>`; empty if it never ran.
fn recorded(path: &Path, suffix: &str) -> String {
    fs::read_to_string(format!("{}.{}", path.display(), suffix)).unwrap_or_default()
}

/// The changes the substrate reports, as `(full id, shortest id, description)`, read here with
/// jj directly so the module's own listing is not the source of its own expectation.
fn reported(repo: &Path) -> Vec<(String, String, String)> {
    jj(
        repo,
        &[
            "log",
            "--no-graph",
            "-r",
            "::",
            "-T",
            "change_id ++ \"\\t\" ++ change_id.shortest(8) ++ \"\\t\" ++ description.first_line() ++ \"\\n\"",
        ],
    )
    .lines()
    .filter(|line| !line.is_empty())
    .filter_map(|line| {
        let mut parts = line.split('\t');
        let full = parts.next()?.to_string();
        let short = parts.next()?.to_string();
        let description = parts.next().unwrap_or("").to_string();
        Some((full, short, description))
    })
    .collect()
}

/// Which reported change a returned id names: its full id, its shortest id, or a unique longer
/// prefix of one. The id's spelling is the module's business; that it names a change is not.
fn description_of(reported: &[(String, String, String)], id: &str) -> Option<String> {
    let mut hits = reported
        .iter()
        .filter(|(full, short, _)| full == id || short == id || (id.len() >= 4 && full.starts_with(id)));
    let hit = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    Some(hit.2.clone())
}

/// The paths the read paired with one change, by description.
fn paths_for(paired: &[(String, Vec<String>)], description: &str) -> Vec<String> {
    paired
        .iter()
        .filter(|(change, _)| change == description)
        .flat_map(|(_, paths)| paths.clone())
        .collect()
}

#[test]
fn the_read_crosses_once_and_pairs_every_change_with_the_paths_it_touched() {
    let root = scratch("pairs");
    let repo = root.join("repo");
    jj(&repo, &["git", "init"]);
    fs::write(repo.join("alpha.txt"), "a\n").unwrap();
    jj(&repo, &["describe", "-m", "alpha change"]);
    jj(&repo, &["new", "-m", "beta change"]);
    fs::write(repo.join("beta.txt"), "b\n").unwrap();
    jj(&repo, &["new", "-m", "gamma change"]);
    fs::write(repo.join("gamma.txt"), "c\n").unwrap();
    jj(&repo, &["new", "-m", "work rename"]);
    fs::rename(repo.join("gamma.txt"), repo.join("delta.txt")).unwrap();

    // The substrate's own view, taken before the read runs and independently of it.
    let changes = reported(&repo);

    let recorder = stand_in(
        &root.join("bin"),
        "jj-record",
        &format!("exec \"{}\" \"$@\"", real_jj().display()),
    );
    let runner = Jj::new(&recorder, repo.as_path());
    let pairs = match jj_custody::read(&runner) {
        Ok(pairs) => pairs,
        Err(_) => panic!("the custody read must succeed on a healthy repo"),
    };

    // One crossing for the whole set: a read per change, or a second crossing to collect the
    // paths, shows up here as more than one spawn.
    let calls = recorded(&recorder, "calls");
    assert_eq!(
        calls.len(),
        1,
        "the read must cross the process boundary once for the whole set, not {} times",
        calls.len()
    );

    // The one crossing is the listing, and the template it is asked with carries the summary
    // of the paths each change touched — so the pairing rides that crossing rather than a
    // second one.
    let argv = recorded(&recorder, "argv");
    let asked = argv.to_lowercase();
    assert_eq!(
        asked.split_whitespace().next(),
        Some("log"),
        "the one crossing must be a jj log: {argv}"
    );
    assert!(
        asked.contains("-t ") || asked.contains("--template"),
        "the summary must ride the listing's template: {argv}"
    );
    assert!(
        asked.contains("diff") && (asked.contains("summary") || asked.contains("files")),
        "the listing's template must carry each change's diff summary: {argv}"
    );

    // The pairs: every change the listing reports, each with its own paths.
    let mut paired: Vec<(String, Vec<String>)> = Vec::new();
    for (id, paths) in &pairs {
        let description = match description_of(&changes, id) {
            Some(description) => description,
            None => panic!("the read returned a change id the substrate does not report: {id}"),
        };
        assert!(
            !paired.iter().any(|(change, _)| change == &description),
            "a change arrives once, carrying its own paths: {id} ({description})"
        );
        paired.push((description, paths.clone()));
    }

    // No ownership test and no path filter narrows the set: this scratch repo carries no
    // nodes at all, so a read that kept only the changes a node claims would keep nothing.
    assert_eq!(
        paths_for(&paired, "alpha change"),
        vec!["alpha.txt".to_string()],
        "a change must arrive with the path it touched"
    );
    assert_eq!(
        paths_for(&paired, "beta change"),
        vec!["beta.txt".to_string()],
        "a change must arrive with the path it touched"
    );
    assert_eq!(
        paths_for(&paired, "gamma change"),
        vec!["gamma.txt".to_string()],
        "a change must arrive with the path it touched"
    );

    // A rename's two sides are the two paths it touched — the side it left and the side it
    // reached — not one string standing in for both.
    let mut renamed = paths_for(&paired, "work rename");
    renamed.sort();
    assert_eq!(
        renamed,
        vec!["delta.txt".to_string(), "gamma.txt".to_string()],
        "a rename's two sides are the two paths it touched"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failed_crossing_comes_back_as_a_value_not_a_panic() {
    let root = scratch("failed");
    let repo = root.join("repo");
    jj(&repo, &["git", "init"]);
    let failing = stand_in(
        &root.join("bin"),
        "jj-fail",
        "printf 'simulated failure\\n' >&2\nexit 1",
    );
    let runner = Jj::new(&failing, repo.as_path());
    match jj_custody::read(&runner) {
        Ok(pairs) => panic!("a failed crossing is not a listing: {:?}", pairs),
        Err(_) => {}
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_absent_binary_comes_back_as_a_value_not_a_panic() {
    let root = scratch("absent");
    let repo = root.join("repo");
    jj(&repo, &["git", "init"]);
    let runner = Jj::new(root.join("bin").join("deccy-no-such-jj-binary"), repo.as_path());
    assert!(
        jj_custody::read(&runner).is_err(),
        "a crossing that cannot spawn is a carried failure, not a panic"
    );
    let _ = fs::remove_dir_all(&root);
}
