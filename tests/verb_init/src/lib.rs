//! Gate for M-init (the init verb).
//!
//! Self-contained per M-tests: the shims pull in the module under test and the siblings it
//! calls, mirroring the crate root. The handler is driven over a scratch directory — it runs
//! the bootstrap and creates the two directories, laying no marker file down (a missing file
//! reads as empty, S5-marker-initial). The compile-time `INIT_ROW` is the tie to M-route (a
//! Verb row for the handler).

#[path = "../../../src/jj.rs"]
mod jj;
#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/exclude.rs"]
mod exclude;
#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/context.rs"]
mod context;
#[path = "../../../src/jj_init.rs"]
mod jj_init;
#[path = "../../../src/route.rs"]
mod route;
#[path = "../../../src/verb_init.rs"]
mod verb_init;

use crate::context::Context;
use crate::route::Verb;

/// The handler is the table's shape: a Verb row for `init` (M-route, ynvwvzrm) compiles.
const INIT_ROW: Verb<Context> = Verb {
    name: "init",
    run: crate::verb_init::run,
};

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::context::resolve;
    use crate::refusal::Refusal;
    use crate::verb_init::{run, HELP};

    /// A bare scratch directory under the temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "deccy-verb-init-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// `deccy init <args>` through the real handler over a scratch directory.
    fn init(dir: &Path, args: &[&str]) -> Result<Vec<String>, Refusal> {
        let context = resolve(dir).expect("the scratch directory resolves");
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        run(&context, &owned)
    }

    #[test]
    fn the_prepared_repository_carries_the_directories_and_no_marker_file() {
        let dir = scratch("prepared");
        let lines = init(&dir, &[]).expect("the working directory is prepared");

        assert!(dir.join(".git").is_dir(), "a git repository is created");
        assert!(dir.join(".jj").is_dir(), "a jj workspace is created");
        assert!(dir.join("nodes").is_dir(), "the node directory is created");
        assert!(dir.join(".deccy").is_dir(), "the state directory is created");

        // No marker file is laid down: a missing file reads as empty (S5-marker-initial).
        assert!(
            !dir.join(".deccy/markers.json").exists(),
            "init lays down no marker file"
        );
        assert!(
            !dir.join("markers.json").exists(),
            "no stray marker file at the root either"
        );

        assert_eq!(lines, vec![format!("initialised {}", dir.display())]);
        assert_eq!(crate::INIT_ROW.name, "init");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_working_directory_keeps_the_state_directory_untracked() {
        let dir = scratch("untracked");
        init(&dir, &[]).expect("the working directory is prepared");
        let exclude = fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
        assert!(
            exclude.lines().any(|line| line == ".deccy/"),
            "the state directory is untracked: {exclude:?}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn help_is_the_verbs_own_text_and_writes_nothing() {
        let dir = scratch("help");
        let lines = init(&dir, &["--help"]).expect("help answers");
        assert_eq!(lines, vec![HELP.to_string()]);
        assert!(!dir.join(".jj").exists(), "help writes nothing");
        assert!(!dir.join("nodes").exists(), "help writes nothing");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_explicit_path_is_prepared_instead_of_the_working_directory() {
        let dir = scratch("explicit");
        let target = dir.join("elsewhere");
        fs::create_dir_all(&target).unwrap();

        let lines = init(&dir, &[target.to_str().unwrap()]).expect("the explicit path is prepared");

        assert!(target.join(".jj").is_dir(), "the named target is prepared");
        assert!(target.join(".git").is_dir());
        assert!(target.join("nodes").is_dir());
        assert!(target.join(".deccy").is_dir());
        assert!(!dir.join(".jj").exists(), "the working directory is untouched");
        assert_eq!(lines, vec![format!("initialised {}", target.display())]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_already_initialised_target_refuses_and_changes_nothing() {
        let dir = scratch("guard");
        let target = dir.join("taken");
        fs::create_dir_all(target.join(".jj")).unwrap();

        match init(&dir, &[target.to_str().unwrap()]) {
            Err(refusal) => assert!(
                refusal.evidence().iter().any(|evidence| evidence == ".jj"),
                "the refusal names the marker: {refusal:?}"
            ),
            Ok(lines) => panic!("an initialised target must refuse, got {lines:?}"),
        }
        assert!(!target.join("nodes").exists(), "nothing is written");
        assert!(!target.join(".deccy").exists(), "nothing is written");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_git_repository_refuses_too() {
        let dir = scratch("guard-git");
        let target = dir.join("gitty");
        fs::create_dir_all(target.join(".git")).unwrap();

        match init(&dir, &[target.to_str().unwrap()]) {
            Err(refusal) => assert!(
                refusal.evidence().iter().any(|evidence| evidence == ".git"),
                "the refusal names the marker: {refusal:?}"
            ),
            Ok(lines) => panic!("an initialised target must refuse, got {lines:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
