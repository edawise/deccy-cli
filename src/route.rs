//! Routing machinery: first-word split, shape check, delegation, and refusal.
//!
//! One dispatcher serves S1's clauses: split the first word from the rest (S1-split),
//! dispatch only a single lowercase word (S1-shape/D10), hand the rest on untouched
//! (S1-args), refuse an unknown word by name (S1-refuse/D20) without running anything
//! (S1-quiet), and stay additive (S1-extend/D23). It is generic over the table and the
//! ambient context it is handed — both assembled with the rest of the crate — and it
//! threads that context to the handler alongside the verb's own args. It threads
//! the handler's normal lines out and produces refusals, never rendering either.

use crate::refusal::Refusal;

/// One verb: a single lowercase name and the handler it dispatches to.
///
/// The handler receives the ambient context (the substrate's resolved handles) and the
/// verb's own args; the context's shape is fixed where the table is assembled. The handler
/// returns its normal lines for the envelope to write.
pub struct Verb<C> {
    pub name: &'static str,
    pub run: fn(&C, &[String]) -> Result<Vec<String>, Refusal>,
}

/// Route argv (after the program name) through `verbs`, handing `context` to the handler.
///
/// The first word is the only candidate; every word after it travels untouched. A candidate
/// that is not a single lowercase word — or has no row — refuses naming the word, and no
/// handler runs.
pub fn route<C>(verbs: &[Verb<C>], context: &C, argv: &[String]) -> Result<Vec<String>, Refusal> {
    let Some(first) = argv.first() else {
        return Err(Refusal::new("no verb given"));
    };
    if !is_dispatchable(first) {
        return Err(Refusal::new("unknown verb").with_evidence(first.clone()));
    }
    match verbs.iter().find(|v| v.name == first) {
        Some(verb) => (verb.run)(context, &argv[1..]),
        None => Err(Refusal::new("unknown verb").with_evidence(first.clone())),
    }
}

/// A candidate dispatches only if it is a single lowercase word.
fn is_dispatchable(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase())
}
