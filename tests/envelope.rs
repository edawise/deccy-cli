//! Gate for M-envelope (channels, status, refusal rendering, stopping, globals).
//!
//! Self-contained per M-tests: the shim declares the envelope's code parents (refusal,
//! route, version, help) and the module under test, mirroring the crate root. Run via
//! `tests/envelope.run`, which sets the build stamp version.rs needs. No shared support.

#[path = "../src/refusal.rs"]
mod refusal;
#[path = "../src/route.rs"]
mod route;
#[path = "../src/version.rs"]
mod version;
#[path = "../src/help.rs"]
mod help;
#[path = "../src/envelope.rs"]
mod envelope;

use envelope::{decide, render, status, Outcome};
use refusal::Refusal;
use route::Verb;

fn ok(_context: &(), _args: &[String]) -> Result<Vec<String>, Refusal> {
    Ok(Vec::new())
}

fn boom(_context: &(), _args: &[String]) -> Result<Vec<String>, Refusal> {
    panic!("a refusal must stop before dispatch");
}

fn lines(_context: &(), _args: &[String]) -> Result<Vec<String>, Refusal> {
    Ok(vec!["one".to_string(), "two".to_string()])
}

fn table(run: fn(&(), &[String]) -> Result<Vec<String>, Refusal>) -> Vec<Verb<()>> {
    vec![Verb { name: "show", run }]
}

#[test]
fn version_is_a_global_handled_before_dispatch() {
    assert!(matches!(
        decide(&table(boom), &(), &["--version".to_string()]),
        Outcome::Ok(_)
    ));
}

#[test]
fn help_is_a_global_that_never_reaches_the_table() {
    // The handler would panic if the table were consulted.
    assert_eq!(status(&decide(&table(boom), &(), &["--help".to_string()])), 0);
}

#[test]
fn a_refusal_stops_before_dispatch_and_reads_as_one_line() {
    match decide(&table(boom), &(), &["nope".to_string()]) {
        Outcome::Refused(r) => {
            assert_eq!(status(&Outcome::Refused(r.clone())), 2);
            assert_eq!(render(&r), "unknown verb: nope");
            assert!(!render(&r).contains('\n'));
        }
        Outcome::Ok(_) => panic!("an unknown word must refuse"),
    }
}

#[test]
fn success_is_status_zero() {
    assert_eq!(status(&decide(&table(ok), &(), &["show".to_string()])), 0);
}

#[test]
fn a_dispatched_handler_s_lines_surface_as_stdout_lines() {
    match decide(&table(lines), &(), &["show".to_string()]) {
        Outcome::Ok(out) => assert_eq!(out, ["one", "two"]),
        Outcome::Refused(_) => panic!("a returning handler must not refuse"),
    }
}
