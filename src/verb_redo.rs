//! The redo verb: revert the reverts recorded against the next undone operation.
//!
//! D15b (qwqwqtuq) fixes that redo reverts the reverts undo recorded (lxvppqom, oozpzlzn)
//! and is valid while the redo tail holds the operation — while the position is before the
//! marker count (kpptprql) — and never part of one (qmynqmwy); D8a
//! (prsoxsuo) fixes whole-operation scope over D8b's (txqmnvup) operation entries, and the
//! refusal semantics of a merge entry and of a revert that refuses partway are qxmzpupt's. A
//! leading --help answers with the verb's own text (xlvoxmyl/rwlywqrk, per oqnsuzqu); any
//! other argument refuses, named, before the store is taken or anything is read (U1,
//! puuvpqzl), so the surface answers on a root-less context. It loads the marker state through
//! M-markers-persist (wzyorrny), answers with the state redo leaves and the entries it names —
//! the reverts recorded against the next undone operation (S5-redo-effect, xovuxpvq): a
//! recording's journal entry is read through M-journal (zutqsqtu) and its after side applied to
//! the target while the working copy is left at the after side — the recorded diff gone —
//! (M-journal-apply, wmwkkqxl; D15c, mnwqksmt), the entry id cleared; a change-level
//! operation's recorded reverts are reverted newest first through the revert module (znmsuqlm,
//! S3-revert kttmwrsv's mechanism), reading each revert's operation id through M-jj-oplog
//! (mvwnksou) so a partway refusal can be undone. It then writes the state it leaves
//! (S5-save-after-revert, kqqysmxn), clearing the reverts it reverted. A save that cannot
//! land after the write half reverses the re-reverts it made and refuses, naming the cause
//! (S5-save-failure, ntlspwxk), so the graph and the marker file never disagree. A crossing that cannot
//! spawn or fails refuses with its reason, naming the entry; a revert that refuses partway
//! reverts the re-reverts already made and refuses, naming the entry — symmetric with undo.
//! Each revert is then checked for divergence through M-jj-divergence (xoroywoy): a revert that
//! leaves a change with more than one commit is itself reverted, so the state
//! returns to its pre-redo state, and the invocation refuses, naming the entry
//! (S5-revert-divergent, ppytsptl).
//! Nothing undone and a position past the markers refuse inside M-markers-guards
//! (lqtqzznn), their refusals passing through unchanged; a marker file that
//! cannot be read refuses, naming the corruption (S5-refusal-site, tkqvkzsy), and one that
//! cannot be read or written refuses through the invocation that touched it
//! (S2-markers-refusal-site, sprvousp). The invocation never brackets (S5-lifecycle,
//! vwlrqoxk) and writes nothing on success (O7-redo, lnulwqwt). It takes the injected context
//! (M-context zttrwlts, M-context-injection ryuswoxz), builds its refusals through M-refusal
//! (popowxnw), and returns its lines — none on success — for the envelope to write
//! (M-output-return, utrzvyyt) under M-route's handler shape (ynvwvzrm). M-layout (szmuyqrq)
//! places the module flat at src/verb_redo.rs; I1-serde (orrvsxqq) fixes the JSON layer the
//! module is written against. This module renders nothing.

use crate::context::Context;
use crate::divergence::{self, DivergenceError};
use crate::jj::{Jj, JjError};
use crate::journal::{self, JournalError};
use crate::journal_apply::{self, Apply, ApplyError};
use crate::markers_guards;
use crate::markers_persist::{self, PersistError};
use crate::oplog::{self, OplogError};
use crate::refusal::Refusal;
use crate::revert;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy redo: revert the reverts recorded against the next undone operation.";

/// The redo handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    redo(context, args)
}

/// The redo operation (D15b, D8a): answer the argument surface before the store is taken — a
/// --help invocation needs no workspace and a bad argument is refused by name (U1,
/// xlvoxmyl) — then load the marker state, answer with the state redo leaves and the entries
/// it names, revert them newest first, then write that state back. Success is silent
/// (O7-redo).
fn redo(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    if let Some(argument) = args.first() {
        return Err(Refusal::new("unexpected argument").with_evidence(argument.clone()));
    }
    let store = context.store()?;
    let jj = context.jj();
    let state = markers_persist::load(store).map_err(|error| Refusal::new(persist_reason(&error)))?;
    let transition = markers_guards::redo(state)?;
    let mut made: Vec<String> = Vec::new();
    let mut recording: Option<journal::Entry> = None;
    if transition.kind == crate::markers::Kind::Record {
        // A recording's redo applies its journal entry's after side and takes the before side
        // back into the working copy (D8c, D15c).
        let id = transition
            .entries
            .first()
            .ok_or_else(|| Refusal::new("the recording names no journal entry"))?;
        let entry = journal::get(store, id).map_err(journal_failed)?;
        journal_apply::apply(store, jj, &entry, Apply::Redo).map_err(apply_failed)?;
        recording = Some(entry);
    } else {
        for entry in transition.entries.iter().rev() {
            match cross(jj, entry) {
                Ok(revert_id) => made.push(revert_id),
                Err(refusal) => {
                    // The re-reverts already made are reverted, so the state returns to its
                    // pre-redo state; the refusing entry is named (qxmzpupt, qmynqmwy).
                    roll_back(jj, &made);
                    return Err(refusal);
                }
            }
        }
    }
    if let Err(error) = markers_persist::save(store, &transition.next) {
        // The save half failed (S5-save-failure, ntlspwxk): reverse the inverse already made
        // through the same remedy a partway refusal uses (qxmzpupt), so the graph and the
        // marker file agree again.
        match &recording {
            Some(entry) => {
                let _ = journal_apply::apply(store, jj, entry, Apply::Undo);
            }
            None => roll_back(jj, &made),
        }
        return Err(Refusal::new(persist_reason(&error)));
    }
    Ok(Vec::new())
}

/// Cross one entry out of the log and answer with the revert operation's id, which a partway
/// refusal reverts again to return the state. A crossing that cannot spawn or fails refuses
/// with its reason, naming the entry (qxmzpupt, S3-revert).
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

/// Revert the re-reverts already made, so the state returns to its pre-redo state
/// (qmynqmwy, qxmzpupt): a revert that refuses partway refuses the whole redo.
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
