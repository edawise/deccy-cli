//! Gate for M-help (the bare verb-list renderer).
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parent
//! (route, plus route's own dep refusal, since Verb's handler type names Refusal), mirroring
//! the crate root. No shared test-support module.

#[path = "../src/refusal.rs"]
mod refusal;
#[path = "../src/route.rs"]
mod route;
#[path = "../src/help.rs"]
mod help;

use refusal::Refusal;
use route::Verb;

fn ok(_context: &(), _args: &[String]) -> Result<Vec<String>, Refusal> {
    Ok(Vec::new())
}

fn verb(name: &'static str) -> Verb<()> {
    Verb { name, run: ok }
}

#[test]
fn lists_every_verb_one_per_line_in_order() {
    let verbs = [verb("show"), verb("new"), verb("log")];
    assert_eq!(help::bare_list(&verbs), "show\nnew\nlog\n");
}

#[test]
fn an_empty_table_lists_nothing() {
    let empty: [Verb<()>; 0] = [];
    assert_eq!(help::bare_list(&empty), "");
}
