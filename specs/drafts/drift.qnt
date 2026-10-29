// T-draft-drift: the two readings of the draft registry - the reconcile reading and the
// `deccy drafts` listing - held as one machine model. Each reading answers one question
// per record: does the workspace the record names still exist while the record still
// asserts a draft? Reconcile reports the records whose answer is yes; the listing reports
// every record with its answer. Both change nothing.
//
// The record's form, its two states and its references are the registry model's and are
// imported, not restated (tvtnokzu). Existence is jj's answer, not the registry's, so the
// workspaces jj has are an input to each reading and not a variable of this module: the
// reading adds no registry state. The verb's name and output form, the scan of
// `.deccy/work/` and any repair are outside this module.
module draft_drift {
  import draft_state.* from "./state"

  // kwqnrzoz + yuomrusm + qspsyqlp: one report per record, keyed by the handle that
  // identifies it and carrying what that record says - its state and the workspace
  // reference it names - plus the drift determination the reading makes for it. The node
  // and the edit are not part of a report: the listing lists records, never nodes or
  // edits (nzrnrlky).
  type Report = {
    handle: str,
    state: State,
    workspace: Workspace,
    drift: bool,
  }

  // yozqornv + xkolyytz: whether jj has the workspace a record names is jj's answer,
  // asked of jj and never inferred from the registry - the registry cannot make jj keep a
  // workspace, so it cannot read existence off its own records. It is an input to the
  // reading: a predicate over the workspace reference, holding of the workspaces jj has.
  // The match of a record's name and path to one of jj's workspaces is not fixed here,
  // because it is not fixed by any prerequisite (nzrnrlky leaves how jj is asked and how
  // a name and path are matched to it open); a set of paths or any other keying would be
  // that unstated rule, so the model fixes none.
  pure def missing(ws: Workspace, jjHas: (Workspace) => bool): bool =
    not(jjHas(ws))

  // yozqornv + qspsyqlp + vkkwusty: drift is asked per record, and the record's state
  // alone decides whether a missing workspace counts. `open` is the record of a draft
  // that has not landed, and a draft is written in its own workspace, so an absent
  // workspace under an open record is drift; `landed` is the record of a draft that has
  // landed, so no draft is written there any more and the same absence is the normal end
  // of a draft, not drift. The state module's two values are the only states, so there is
  // no third determination and no other outcome (tvtnokzu, yuomrusm).
  pure def drift(r: Record, jjHas: (Workspace) => bool): bool =
    (r.state == Open) and missing(r.workspace, jjHas)

  // qspsyqlp + kwqnrzoz: the report for one record is what that record says - the handle
  // it is filed under, its state and the workspace it names - and the drift flag for it.
  // Nothing else is reported: the record's node, its edit and its base are not in the
  // report, because the listing reports records and not nodes or edits (nzrnrlky), and no
  // count, order or presentation rule is stated (vuopwuyo, twpvqstu leave it open).
  pure def reportOf(r: Record, jjHas: (Workspace) => bool): Report =
    {
      handle: r.handle,
      state: r.state,
      workspace: r.workspace,
      drift: drift(r, jjHas),
    }

  // qspsyqlp + nzrnrlky + twpvqstu: the listing ranges over every record the registry
  // holds - each exactly once under the handle that keys it, landed records included,
  // since landing removes no record and the registry's keys are the whole scan. The drift
  // flag is asked per record and is answered for every row, not summarised.
  def listing(jjHas: (Workspace) => bool): str -> Report =
    records.keys().mapBy(h => reportOf(records.get(h), jjHas))

  // yozqornv + vuopwuyo: reconcile reports exactly the records that still assert a draft
  // while jj has no workspace they name - the registry's records filtered by the drift
  // determination, keyed by handle and carrying the same per-record report. A landed
  // record is retained in the registry and is not reported; leaving a record out of the
  // report is not dropping it (vkkwusty).
  def reconcile(jjHas: (Workspace) => bool): str -> Report =
    // One entry per drifted record and no others. The fold's order cannot show, because
    // the registry's keys are distinct.
    records.keys().fold(Map(), (acc, h) =>
      if (drift(records.get(h), jjHas)) acc.put(h, reportOf(records.get(h), jjHas)) else acc)

  // yozqornv + qspsyqlp + vuopwuyo: the invocation is a read. The reconcile reading
  // changes nothing: no record added, removed or rewritten, no state moved, no workspace
  // created or removed - existence is an input and this module holds no workspace - so the
  // step leaves `records` exactly as it stands and a drifted record stays open, still
  // naming the workspace it names. The report is the whole effect.
  action reconcileRead(jjHas: (Workspace) => bool): bool = all {
    records' = records,
  }

  // qspsyqlp + vuopwuyo: the listing is the same read, reporting every record instead of
  // the drifted ones; it too leaves the registry as it stands and repairs nothing.
  action listingRead(jjHas: (Workspace) => bool): bool = all {
    records' = records,
  }

  // yozqornv + qspsyqlp: the two readings are the module's only steps, and both are reads.
  // They differ in what they report, not in what they change, so every step of the model
  // leaves the registry unchanged.
  action step(jjHas: (Workspace) => bool): bool = any {
    reconcileRead(jjHas),
    listingRead(jjHas),
  }
}
