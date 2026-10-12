//! The init verb: prepare a repository for dec-graph use.
//!
//! The init verb: it takes an optional `[path]` (the working directory when none is given,
//! IN-verb, tvppvyqu) and, unless `--help` is asked, refuses when the target is already
//! initialised — a jj workspace or a git repository (IN-guard, vsyytyrt) — before anything
//! is written; otherwise it runs the bootstrap through M-jj-init (IN-mechanism, lmomkluy),
//! creates the `nodes/` and `.deccy/` directories (IN-dirs, rvuunmlm), and returns its lines
//! for the envelope to write (M-output-return, utrzvyyt). A fresh repository carries no
//! marker file: a missing file reads as empty (S5-marker-initial, lspnqzzz), so the verb
//! lays nothing down. It runs on a root-less context (IN-entry, tvwvzluw; S3-root-optional,
//! vxvsvzns) and needs no store of its own; the git and jj binaries come from the injected
//! context (M-context-injection, ryuswoxz). It builds its refusals through M-refusal
//! (popowxnw), carries its own --help text (per oqnsuzqu), and follows M-route's handler
//! shape (ynvwvzrm); adding it is one row in the table (D23, oywywrvu). This module renders
//! nothing.

use std::fs;
use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::exclude::ExcludeError;
use crate::jj::Jj;
use crate::jj_init::{self, BootstrapError};
use crate::refusal::Refusal;
use crate::store::Store;

/// The verb's own `--help` text (per-verb help is the verb's content: oqnsuzqu).
pub const HELP: &str = "deccy init [path]: prepare a repository for dec-graph use.";

/// The init handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    let target = target(context, args)?;
    guard(&target)?;
    prepare(context, &target)?;
    Ok(vec![format!("initialised {}", target.display())])
}

/// The target: the given `[path]`, or the working directory when none is given (IN-verb).
///
/// The working directory is the runner's root, which is the cwd when the context is
/// root-less. A second positional argument is unnamed by the form and refuses, naming it.
fn target(context: &Context, args: &[String]) -> Result<PathBuf, Refusal> {
    match args.first() {
        None => Ok(context.jj().root().to_path_buf()),
        Some(path) => {
            if let Some(extra) = args.get(1) {
                return Err(Refusal::new("an unnamed argument").with_evidence(extra.clone()));
            }
            Ok(PathBuf::from(path))
        }
    }
}

/// Refuse a target that is already initialised — a jj workspace or a git repository — before
/// anything is written (IN-guard). The refusal names the marker it found.
fn guard(target: &Path) -> Result<(), Refusal> {
    for marker in [".jj", ".git"] {
        if target.join(marker).exists() {
            return Err(Refusal::new("the target is already initialised").with_evidence(marker));
        }
    }
    Ok(())
}

/// Run the ordered bootstrap on the target (IN-mechanism), then lay down the two directories
/// (IN-dirs). No marker file is laid down: a missing file reads as empty (S5-marker-initial).
fn prepare(context: &Context, target: &Path) -> Result<(), Refusal> {
    let store = Store::new(target);
    let git = Jj::new(context.git()?, target);
    let jj = Jj::new(context.jj().binary(), target);
    jj_init::bootstrap(&git, &jj, &store).map_err(bootstrap_failed)?;
    fs::create_dir_all(target.join("nodes")).map_err(|error| Refusal::new(error.to_string()))?;
    fs::create_dir_all(target.join(".deccy")).map_err(|error| Refusal::new(error.to_string()))?;
    Ok(())
}

/// A bootstrap that did not complete: its reason is carried (S3-err).
fn bootstrap_failed(error: BootstrapError) -> Refusal {
    match error {
        BootstrapError::Spawn(reason) => Refusal::new(reason),
        BootstrapError::Failed(output) => Refusal::new(output.stderr.trim().to_string()),
        BootstrapError::Exclude(error) => Refusal::new(exclude_reason(&error)),
    }
}

/// Why the exclude file could not be read or written: the reason is carried
/// (S2-exclude-read-fail, S2-exclude-append-fail).
fn exclude_reason(error: &ExcludeError) -> String {
    match error {
        ExcludeError::Read(reason) | ExcludeError::Write(reason) => reason.clone(),
    }
}
