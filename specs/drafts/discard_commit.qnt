// T-discard-commit: the commit policy of one discard call, read over the draft
// registry.
//
// A discard is not a registry removal that happens to be preceded by other work: the
// removal is the commit point and it is taken last, only after every named draft's
// change act, workspace act and directory act have completed (nwswzlnw). Until a
// record leaves the store the call has committed nothing, so a failure at any point
// before the commit - on an act, or on the removal itself while no record has left -
// refuses the invocation and leaves the registry exactly as it stood (nwswzlnw). The
// commit itself is qmpskomv's: a call is accepted exactly when every handle it names
// picks out an `open` record, and an accepted call removes exactly the records it
// names and nothing else. The record's form, its two states and the registry variable
// `records` are tvtnokzu's (`draft_state`); this module imports them and states
// nothing of its own about a record, a state value or a base.
//
// `prepared` abstracts the per-draft work the commit stands behind: a handle in it is
// a named draft whose three acts have completed before the call decides. `failed` says
// whether any of those acts failed, or the removal failed while no record had yet left
// the store; either is a failure before the commit. The order of the three acts among
// themselves, the change abandonment, the workspace forget and the directory removal
// are the workspace half's and the change act's own commitments and are outside this
// module: nothing here abandons a change, forgets a workspace or touches a path.
//
// The action is total over its three inputs and has exactly two outcomes, never a
// third. A call that has failed nothing, has every named draft prepared and names only
// open records steps the registry to exactly qmpskomv's removal of the named records; a
// call that has not reached that point steps the registry to itself, so no named record
// is half-removed and nothing else is written. The action steps no variable but
// `records`.
module draft_discard_commit {
  import draft_state.* from "./state"
  // The selection and the removal are qmpskomv's, imported rather than restated: this
  // module adds the commit condition in front of them, not another reading of which
  // handles select and what leaves the store.
  import draft_discard.discardAccepted from "./discard"
  import draft_discard.discardNamed from "./discard"
  import draft_discard.discardWithout from "./discard"

  // nwswzlnw: the commit is reached only when no act has failed before it and every
  // named draft has had its three acts completed, and the removal it performs is
  // qmpskomv's accepted call - every named handle picks out an `open` record, so a
  // landed record or an absent handle reaches no commit. Nothing failed is
  // `not(failed)`; every named draft is prepared is `discardNamed(handles)` contained
  // in `prepared`; every named handle names an open record is `discardAccepted`. A
  // handle the call does not name is not consulted, so prepared extras do not widen
  // the removal.
  pure def commitReached(
    recs: str -> Record,
    handles: List[str],
    prepared: Set[str],
    failed: bool,
  ): bool =
    not(failed) and discardNamed(handles).subseteq(prepared) and
      discardAccepted(recs, handles)

  // nwswzlnw + qmpskomv: one call, total over the handles it names, the named drafts
  // whose acts completed and the failure flag. Before the commit the registry is left
  // exactly as it was; at the commit it is qmpskomv's registry without the named
  // records. The removal is the commit point, so there is no state in which some named
  // record has left while another named draft's work is still to run, and no variable
  // but `records` is written.
  action discardCommit(handles: List[str], prepared: Set[str], failed: bool): bool =
    records' =
      if (commitReached(records, handles, prepared, failed))
        discardWithout(records, handles)
      else
        records
}
