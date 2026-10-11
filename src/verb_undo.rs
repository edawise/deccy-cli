//! The undo verb: take the most recent applied operation by the inverse its kind names.
//!
//! D15 (lummwqtx) fixes that undo undoes the last applied operation — the marker before
//! the position (kpptprql, yrnkmrqx) — and never part of one (qmynqmwy); D8a (prsoxsuo)
//! fixes whole-operation scope
//! and the inverse each kind names, over D8b's (txqmnvup) operation record, and the refusal
//! semantics of a merge entry and of a revert that refuses partway are qxmzpupt's. A leading
//! --help answers with the verb's own text (xlvoxmyl/rwlywqrk, per oqnsuzqu); any other
//! argument refuses, named, before the store is taken or anything is read (U1, puuvpqzl), so
//! the surface answers on a root-less context. It loads the marker state
//! through M-markers-persist (wzyorrny), answers with the state undo leaves and the operation
//! it names — its kind, the change it targeted and its kept entry ids (S5-undo-effect,
//! zuqzrlrx) — and takes the inverse the kind names: a creation is removed by abandoning the
//! change it targeted through M-jj-abandon (urtkpxwv); a recording's journal entry is read
//! through M-journal (zutqsqtu) and its before side applied to the target while the after side
//! stays in the working copy (M-journal-apply, wmwkkqxl; D15c, mnwqksmt), the entry id recorded
//! against the operation; a removal's entries are reverted newest first through the revert
//! module (znmsuqlm, S3-revert kttmwrsv's mechanism), recording each revert's operation id,
//! read through M-jj-oplog (mvwnksou), against the operation it undoes, and then writes the
//! state it leaves (S5-save-after-revert, kqqysmxn), so a revert cannot take the record of the
//! operation with it. A save that cannot
//! land after the write half reverses the crossings it made and refuses, naming the cause
//! (S5-save-failure, ntlspwxk), so the graph and the marker file never disagree. A crossing that
//! cannot spawn or fails refuses with its reason, naming the change or the entry; a revert
//! that refuses partway reverts the reverts already made and refuses, naming the entry. Each
//! revert is then checked for divergence through M-jj-divergence (xoroywoy): a revert that
//! leaves a change with more than one commit is itself reverted, so the state
//! returns to its pre-undo state, and the invocation refuses, naming the entry
//! (S5-revert-divergent, ppytsptl). A position at zero and a position past the markers
//! refuse inside M-markers-guards (lqtqzznn), their refusals passing through unchanged; a
//! marker file that cannot be read refuses, naming the corruption
//! (S5-refusal-site, tkqvkzsy), and one that cannot be read or written refuses through the
//! invocation that touched it (S2-markers-refusal-site, sprvousp). The invocation never
//! brackets (S5-lifecycle, vwlrqoxk) and writes nothing on success (O7-undo, tmxutlwm). It
//! takes the injected context (M-context zttrwlts, M-context-injection ryuswoxz), builds its
//! refusals through M-refusal (popowxnw), and returns its lines — none on success — for the
//! envelope to write (M-output-return, utrzvyyt) under M-route's handler shape (ynvwvzrm).
//! M-layout (szmuyqrq) places the module flat at src/verb_undo.rs; I1-serde (orrvsxqq) fixes
//! the JSON layer the module is written against. This module renders nothing.

use crate::context::Context;
use crate::divergence::{self, DivergenceError};
use crate::jj::{Jj, JjError};
use crate::jj_abandon;
use crate::journal::{self, JournalError};
use crate::journal_apply::{self, Apply, ApplyError};
use crate::markers_guards;
use crate::markers_persist::{self, PersistError};
use crate::oplog::{self, OplogError};
use crate::refusal::Refusal;
use crate::revert;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy undo: undo the most recent applied operation.";

/// The undo handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    undo(context, args)
}

/// The undo operation (D15, D8a): answer the argument surface before the store is taken — a
/// --help invocation needs no workspace and a bad argument is refused by name (U1,
/// xlvoxmyl) — then load the marker state, answer with the state undo leaves and the entries
/// it names, revert them newest first recording each revert, then write that state back.
/// Success is silent (O7-undo).
fn undo(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    if let Some(argument) = args.first() {
        return Err(Refusal::new("unexpected argument").with_evidence(argument.clone()));
    }
    let store = context.store()?;
    let jj = context.jj();
    let state = markers_persist::load(store).map_err(|error| Refusal::new(persist_reason(&error)))?;
    let transition = markers_guards::undo(state)?;
    let mut made: Vec<String> = Vec::new();
    let kind = transition.kind;
    match kind {
        crate::markers::Kind::Create => {
            // A creation's undo removes the created change (D8a): one crossing, recorded as one
            // revert so redo can restore it.
            made.push(abandon(jj, &transition.target)?);
        }
        crate::markers::Kind::Record => {
            // A recording's undo applies its journal entry's before side and returns the after
            // side to the working copy (D8c, D15c).
            let id = transition
                .entries
                .first()
                .ok_or_else(|| Refusal::new("the recording names no journal entry"))?;
            let entry = journal::get(store, id).map_err(journal_failed)?;
            journal_apply::apply(store, jj, &entry, Apply::Undo).map_err(apply_failed)?;
            made.push(id.clone());
        }
        crate::markers::Kind::Remove => {
            for entry in transition.entries.iter().rev() {
                match cross(jj, entry) {
                    Ok(revert_id) => made.push(revert_id),
                    Err(refusal) => {
                        // The reverts already made are reverted, so the graph returns to its
                        // pre-undo state; the refusing entry is named (qxmzpupt, qmynqmwy).
                        roll_back(jj, &made);
                        return Err(refusal);
                    }
                }
            }
        }
    }
    let next = transition.recorded(made.clone());
    if let Err(error) = markers_persist::save(store, &next) {
        // The save half failed (S5-save-failure, ntlspwxk): reverse the inverse already made
        // through the same remedy a partway refusal uses (qxmzpupt), so the graph and the
        // marker file agree again.
        match kind {
            crate::markers::Kind::Record => {
                if let Ok(entry) = journal::get(store, &made[0]) {
                    let _ = journal_apply::apply(store, jj, &entry, Apply::Redo);
                }
            }
            _ => roll_back(jj, &made),
        }
        return Err(Refusal::new(persist_reason(&error)));
    }
    Ok(Vec::new())
}

/// Cross one entry out of the log and answer with the revert operation's id, which undo
/// records so redo can revert it. A crossing that cannot spawn or fails refuses with its
/// reason, naming the entry (qxmzpupt, S3-revert).
fn cross(jj: &Jj, entry: &str) -> Result<String, Refusal> {
    let output = revert::revert(jj, &[entry.to_string()])
        .map_err(|error| spawn_failed(error).with_evidence(entry.to_string()))?;
    if output[0].status != 0 {
        return Err(Refusal::new(output[0].stderr.trim().to_string())
            .with_evidence(entry.to_string()));
    }
    let reverted =
        oplog::oplog(jj).map_err(|error| oplog_failed(error).with_evidence(entry.to_string()))?;
    // The verdict is taken after the revert (S5-revert-divergent, ppytsptl): a revert that
    // splits a change is itself reverted before the invocation refuses, so no
    // intermediate state is divergent.
    if divergence::divergent(jj)
        .map_err(|error| divergence_failed(error).with_evidence(entry.to_string()))?
    {
        let _ = revert::revert(jj, std::slice::from_ref(&reverted));
        return Err(diverged().with_evidence(entry.to_string()));
    }
    Ok(reverted)
}

/// Remove the created change (D8a) through M-jj-abandon and answer with the operation's id,
/// which undo records so redo can restore the change. A crossing that cannot spawn or fails
/// refuses with its reason, naming the change.
fn abandon(jj: &Jj, target: &str) -> Result<String, Refusal> {
    let output = jj_abandon::abandon(jj, target)
        .map_err(|error| spawn_failed(error).with_evidence(target.to_string()))?;
    if output.status != 0 {
        return Err(
            Refusal::new(output.stderr.trim().to_string()).with_evidence(target.to_string())
        );
    }
    oplog::oplog(jj).map_err(|error| oplog_failed(error).with_evidence(target.to_string()))
}

/// Revert the reverts already made, so the state returns to its pre-undo state
/// (qmynqmwy, qxmzpupt): a revert that refuses partway refuses the whole undo.
fn roll_back(jj: &Jj, made: &[String]) {
    for revert_id in made.iter().rev() {
        let _ = revert::revert(jj, std::slice::from_ref(revert_id));
    }
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

/// A revert that could not spawn jj: the reason is carried (S3-err).
fn spawn_failed(error: JjError) -> Refusal {
    match error {
        JjError::Spawn(reason) => Refusal::new(reason),
    }
}

/// An op-log read that could not run: the reason is carried (S3-err).
fn oplog_failed(error: OplogError) -> Refusal {
    match error {
        OplogError::Spawn(reason) => Refusal::new(reason),
        OplogError::Failed(output) => Refusal::new(output.stderr.trim().to_string()),
    }
}

/// The refusal a revert that leaves a change divergent gets (S5-revert-divergent).
fn diverged() -> Refusal {
    Refusal::new("the revert leaves a change divergent")
}

/// The journal entry or blob could not be read: the reason is carried (S3-err).
fn journal_failed(error: JournalError) -> Refusal {
    Refusal::new(format!("{error:?}"))
}

/// The journal apply refused: a mismatch names every path that does not hold its expected
/// side; every other failure carries its reason (D15c, S3-err).
fn apply_failed(error: ApplyError) -> Refusal {
    match error {
        ApplyError::Mismatch(paths) => {
            let mut refusal = Refusal::new("the recording does not hold the expected side");
            for path in paths {
                refusal = refusal.with_evidence(path);
            }
            refusal
        }
        ApplyError::Conflict(reason) => Refusal::new(reason),
        ApplyError::Journal(error) => journal_failed(error),
        ApplyError::Spawn(reason)
        | ApplyError::Failed(reason)
        | ApplyError::WorkingCopy(reason) => Refusal::new(reason),
    }
}

/// A divergence read that could not run: the reason is carried (S3-err).
fn divergence_failed(error: DivergenceError) -> Refusal {
    match error {
        DivergenceError::Spawn(reason) => Refusal::new(reason),
        DivergenceError::Failed(output) => Refusal::new(output.stderr.trim().to_string()),
    }
}
