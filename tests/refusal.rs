//! Gate for M-refusal (the refusal value type).
//!
//! Self-contained per M-tests: pulls the module under test in with a #[path] shim and
//! carries its own fixtures — no shared test-support module.

#[path = "../src/refusal.rs"]
mod refusal;

use refusal::Refusal;

#[test]
fn a_refusal_carries_its_cause() {
    let r = Refusal::new("unknown id");
    assert_eq!(r.cause(), "unknown id");
}

#[test]
fn evidence_accumulates_in_order() {
    let r = Refusal::new("unknown --on id")
        .with_evidence("abc123")
        .with_evidence("def456");
    assert_eq!(r.evidence(), ["abc123", "def456"]);
}

#[test]
fn refusal_is_a_value() {
    let a = Refusal::new("boom").with_evidence("x");
    assert_eq!(a.clone(), a);
}
