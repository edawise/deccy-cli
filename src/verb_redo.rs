//! The redo verb: restore the next undone operation's post-operation boundary.
//!
//! D15b (qwqwqtuq) fixes that redo restores an operation boundary (lxvppqom) — the next undone
//! operation's post-operation boundary, and only while no newer operation exists (oozpzlzn) —
//! and never a partial restore (qmynqmwy); D8a (prsoxsuo) fixes whole-operation scope over D8b's
//! (txqmnvup) boundary pairs. A leading --help answers with the verb's own text
//! (xlvoxmyl/rwlywqrk, per oqnsuzqu); any other argument refuses, named, before anything is read
//! (U1, puuvpqzl). It loads the marker state through M-markers-persist (wzyorrny), answers with
//! the state redo leaves — the undone count one forward and the position at the post-boundary of
//! the operation it re-applies (S5-redo-effect, xovuxpvq) — restores that position through the
//! restore module (znmsuqlm, S3-restore kttmwrsv's mechanism), a crossing that cannot spawn or
//! exits non-zero refusing with its reason and naming the boundary, and then writes the state it
//! leaves (S5-save-after-restore, kqqysmxn), so the restore cannot take the record of the
//! operation with it. Nothing undone, a newer operation and a count or bound past the pairs
//! refuse inside M-markers-guards (lqtqzznn), their refusals passing through unchanged; a marker
//! file that cannot be read refuses, naming the corruption (S5-refusal-site, tkqvkzsy). The
//! invocation never brackets (S5-lifecycle, vwlrqoxk) and writes nothing on success (O7-redo,
//! lnulwqwt). It takes the injected context (M-context zttrwlts, M-context-injection ryuswoxz),
//! builds its refusals through M-refusal (popowxnw), and returns its lines — none on success —
//! for the envelope to write (M-output-return, utrzvyyt) under M-route's handler shape
//! (ynvwvzrm). M-layout (szmuyqrq) places the module flat at src/verb_redo.rs; I1-serde
//! (orrvsxqq) fixes the JSON layer the module is written against. This module renders nothing.

use crate::context::Context;
use crate::jj::{Jj, JjError};
use crate::markers_guards;
use crate::markers_persist::{self, PersistError};
use crate::refusal::Refusal;
use crate::restore;
use crate::store::Store;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy redo: restore the post-operation boundary of the next undone operation.";

/// The redo handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    let store = context.store()?;
    redo(store, context.jj(), args)
}

/// The redo operation (D15b, D8a): answer the argument surface, load the marker state, answer
/// with the state redo leaves, restore its position, then write that state back. Success is
/// silent (O7-redo).
fn redo(store: &Store, jj: &Jj, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    if let Some(argument) = args.first() {
        return Err(Refusal::new("unexpected argument").with_evidence(argument.clone()));
    }
    let state = markers_persist::load(store).map_err(|error| Refusal::new(persist_reason(&error)))?;
    let next = markers_guards::redo(state)?;
    let boundary = next.at.clone();
    let output = restore::restore(jj, &boundary)
        .map_err(|error| spawn_failed(error).with_evidence(boundary.clone()))?;
    if output.status != 0 {
        return Err(Refusal::new(output.stderr.trim().to_string()).with_evidence(boundary));
    }
    markers_persist::save(store, &next).map_err(|error| Refusal::new(persist_reason(&error)))?;
    Ok(Vec::new())
}

/// Why the marker file did not load or save: the reason is carried, not collapsed
/// (S5-marker-corrupt, vqqmwsml).
fn persist_reason(error: &PersistError) -> String {
    match error {
        PersistError::Unreadable(reason)
        | PersistError::NotJson(reason)
        | PersistError::Invalid(reason)
        | PersistError::Schema(reason)
        | PersistError::Unwritable(reason) => reason.clone(),
    }
}

/// A restore that could not spawn jj: the reason is carried (S3-err).
fn spawn_failed(error: JjError) -> Refusal {
    match error {
        JjError::Spawn(reason) => Refusal::new(reason),
    }
}
