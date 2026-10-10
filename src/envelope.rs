//! The envelope: one owner of channels, exit status, refusal shape, and stopping.
//!
//! S4 fixes one envelope per invocation; its clauses fix what that means — normal lines go
//! to stdout (opmtpuzz), a refusal writes one stderr line and nothing to stdout
//! (wllnuwxu/szyolrkm/omvwkzzk/knuluxtw), exit is 0 without refusal and 2 with, 1 held back
//! (mruplukn/ulxtysml/qxyuwrpu/yqklqwto), and a refusal stops before dispatch (txpzmrto).
//! Globals (`--version`, `--help`) are handled before dispatch, so the table never routes
//! them and no route<->help cycle appears. The envelope is the sole writer of both
//! streams — every normal line, a global's or a dispatched handler's returned lines, to
//! stdout, and the refusal line to stderr — and returns the status, threading the
//! ambient context through to the handler. That write step is exposed (`emit`) so an
//! outcome decided above the dispatch still leaves by the envelope's streams and status.

use crate::help;
use crate::refusal::Refusal;
use crate::route::{route, Verb};
use crate::version;

/// What one invocation decided, before any stream is touched.
pub enum Outcome {
    /// Lines for stdout (a global's, or the dispatched handler's returned lines).
    Ok(Vec<String>),
    /// Stop before dispatch; one stderr line, status 2.
    Refused(Refusal),
}

/// Handle globals first, then dispatch, threading `context` to the handler. Globals sit
/// outside the table.
pub fn decide<C>(verbs: &[Verb<C>], context: &C, argv: &[String]) -> Outcome {
    match argv.first().map(String::as_str) {
        Some("--version") => Outcome::Ok(vec![version::version_line().to_string()]),
        Some("--help") => Outcome::Ok(lines_of(&help::bare_list(verbs))),
        _ => match route(verbs, context, argv) {
            Ok(lines) => Outcome::Ok(lines),
            Err(refusal) => Outcome::Refused(refusal),
        },
    }
}

/// 0 without refusal, 2 with; 1 stays unassigned.
pub fn status(outcome: &Outcome) -> i32 {
    match outcome {
        Outcome::Ok(_) => 0,
        Outcome::Refused(_) => 2,
    }
}

/// The refusal's single line: the cause, then its named evidence.
pub fn render(refusal: &Refusal) -> String {
    let mut line = refusal.cause().to_string();
    for evidence in refusal.evidence() {
        line.push_str(": ");
        line.push_str(evidence);
    }
    line
}

/// Write one already-decided outcome to the streams and return its status. The write step
/// is the envelope's whoever decided the outcome — an outcome decided above the dispatch
/// still leaves by these streams.
pub fn emit(outcome: &Outcome) -> i32 {
    match outcome {
        Outcome::Ok(lines) => lines.iter().for_each(|line| println!("{line}")),
        Outcome::Refused(refusal) => eprintln!("{}", render(refusal)),
    }
    status(outcome)
}

/// The process boundary: decide, then write the streams and return the status.
pub fn run<C>(verbs: &[Verb<C>], context: &C, argv: &[String]) -> i32 {
    emit(&decide(verbs, context, argv))
}

fn lines_of(block: &str) -> Vec<String> {
    block.lines().map(str::to_string).collect()
}
