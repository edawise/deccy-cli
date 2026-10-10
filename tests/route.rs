//! Gate for M-route (the dispatcher).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (refusal), mirroring the crate root, so route.rs's `crate::refusal` path resolves. No
//! shared test-support module.

#[path = "../src/refusal.rs"]
mod refusal;
#[path = "../src/route.rs"]
mod route;

use std::cell::RefCell;

use refusal::Refusal;
use route::{route, Verb};

thread_local! {
    static SEEN: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static CONTEXT: RefCell<Option<u32>> = RefCell::new(None);
}

fn record(_context: &(), args: &[String]) -> Result<Vec<String>, Refusal> {
    SEEN.with(|s| s.borrow_mut().extend_from_slice(args));
    Ok(Vec::new())
}

fn record_context(context: &u32, _args: &[String]) -> Result<Vec<String>, Refusal> {
    CONTEXT.with(|c| *c.borrow_mut() = Some(*context));
    Ok(Vec::new())
}

fn boom(_context: &(), _args: &[String]) -> Result<Vec<String>, Refusal> {
    panic!("a refused invocation must not reach a handler");
}

fn echo(_context: &(), _args: &[String]) -> Result<Vec<String>, Refusal> {
    Ok(vec!["a line".to_string(), "another".to_string()])
}

fn verb(name: &'static str, run: fn(&(), &[String]) -> Result<Vec<String>, Refusal>) -> Verb<()> {
    Verb { name, run }
}

#[test]
fn first_word_is_the_candidate_and_the_rest_travel_untouched() {
    let verbs = [verb("show", record)];
    let argv = ["show".to_string(), "--all".to_string(), "abc123".to_string()];
    assert!(route(&verbs, &(), &argv).is_ok());
    SEEN.with(|s| assert_eq!(*s.borrow(), ["--all", "abc123"]));
}

#[test]
fn the_handed_context_reaches_the_handler() {
    let verbs = [Verb { name: "show", run: record_context }];
    assert!(route(&verbs, &7u32, &["show".to_string()]).is_ok());
    CONTEXT.with(|c| assert_eq!(*c.borrow(), Some(7)));
}

#[test]
fn a_non_word_candidate_refuses() {
    let verbs = [verb("show", record)];
    for bad in ["Show", "sh-ow", "sh0w", ""] {
        let argv = [bad.to_string()];
        assert!(route(&verbs, &(), &argv).is_err(), "{bad:?} must not dispatch");
    }
}

#[test]
fn a_missing_verb_refuses() {
    let verbs = [verb("show", record)];
    assert!(route(&verbs, &(), &[]).is_err());
}

#[test]
fn an_unknown_word_refuses_naming_it() {
    let verbs = [verb("show", record)];
    let argv = ["nope".to_string()];
    assert_eq!(route(&verbs, &(), &argv).unwrap_err().evidence(), ["nope"]);
}

#[test]
fn a_refusal_runs_no_handler() {
    let verbs = [verb("show", boom)];
    let argv = ["nope".to_string()];
    assert!(route(&verbs, &(), &argv).is_err());
}

#[test]
fn the_handler_s_lines_travel_out_untouched() {
    let verbs = [verb("show", echo)];
    let argv = ["show".to_string(), "x".to_string()];
    assert_eq!(route(&verbs, &(), &argv).unwrap(), ["a line", "another"]);
}
