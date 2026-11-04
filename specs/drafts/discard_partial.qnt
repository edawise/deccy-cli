// T-discard-partial: the partial policy of one discard call, read over the draft
// registry and the act state.
//
// A discard that fails before its commit leaves the acts it already completed
// standing, and a second invocation of the same discard goes on through what is
// already clear to the commit instead of refusing (qznqvpyr). This module states that
// policy as Quint state: one action over the registry variable `records` and the
// per-draft act state `acts`.
//
// The record's form, its two states, the content-identity base and the registry
// variable `records` are `draft_state`'s (tvtnokzu, `specs/drafts/state.qnt`); this
// module imports them and states nothing of its own about a record, a state value or a
// base. The commit policy of one call - where the removal sits, what an accepted call
// removes and what a failure before the commit leaves in the store - is `discard_commit`'s
// (nytszsml, `specs/drafts/discard_commit.qnt`); this module imports its action and
// adds only the act state in front of it, so the commit itself stays that model's.
//
// What a real discard performs - abandoning the change, forgetting the workspace,
// removing the directory - is jj and filesystem work and is outside this model: the
// three acts are values in the act state, and their order among themselves, the verb
// surface, its wording, stream and exit status, and the registry write's mechanism,
// atomicity and durability are not stated here.
//
// The helpers are prefixed because an importing module flattens this module's
// definitions next to its own; `discardPartial` is the module's one exported step, and
// the names the check carries belong to the check. `draft_state` already exports `init`,
// so this module carries no initializer.
module draft_discard_partial {
  import draft_state.* from "./state"
  // The commit is `discard_commit`'s action, imported rather than restated: this module
  // hands it the named handles whose acts the attempt completed and the failure flag,
  // and leaves the selection, the removal and the refusal to it.
  import draft_discard_commit.discardCommit from "./discard_commit"

  // The three acts one discard performs for a draft: the abandonment of its change,
  // the forget of its workspace and the removal of its directory. The names, not their
  // order, are the interface.
  type Act = Change | Forget | Remove

  // The acts a discard has completed, per draft handle. A handle the map does not hold
  // has had no act completed. This is the only variable beside `records` the action
  // steps.
  var acts: str -> Set[Act]

  // The three acts, as a set.
  pure def partialAll: Set[Act] = Set(Change, Forget, Remove)

  // The handles a call names, read as a set: a handle named twice is one handle, and
  // the order of the list carries no meaning.
  pure def partialNamed(handles: List[str]): Set[str] =
    handles.indices().map(i => handles.nth(i))

  // The acts completed for a handle, where a handle the map does not hold has none.
  pure def partialActsOf(a: str -> Set[Act], h: str): Set[Act] =
    if (a.keys().contains(h)) a.get(h) else Set()

  // The acts a call completes for a handle, where a handle its map does not hold
  // completes nothing.
  pure def partialCompleted(h: str, completed: str -> Set[Act]): Set[Act] =
    if (completed.keys().contains(h)) completed.get(h) else Set()

  // The act state a handle holds after the attempt: for a named handle the acts before
  // the attempt together with the acts this attempt completed - never less, so no
  // completed act is rolled back - and for an unnamed handle exactly the acts before.
  pure def partialAfterActs(
    a: str -> Set[Act],
    handles: List[str],
    completed: str -> Set[Act],
    h: str,
  ): Set[Act] =
    if (partialNamed(handles).contains(h))
      partialActsOf(a, h).union(partialCompleted(h, completed))
    else
      partialActsOf(a, h)

  // The whole act state after the attempt: every handle already held keeps its entry -
  // grown by this attempt for the named ones - and a named handle not yet held gains
  // one. No other handle appears.
  pure def partialAfterActState(
    a: str -> Set[Act],
    handles: List[str],
    completed: str -> Set[Act],
  ): str -> Set[Act] =
    a.keys()
      .union(partialNamed(handles))
      .mapBy(h => partialAfterActs(a, handles, completed, h))

  // The named handles whose three acts are complete after this attempt. The commit
  // policy is reached for exactly these handles: a named handle still short of an act
  // is not prepared, so the call cannot commit on it.
  pure def partialPrepared(
    a: str -> Set[Act],
    handles: List[str],
    completed: str -> Set[Act],
  ): Set[str] =
    partialNamed(handles).filter(h =>
      partialAll.subseteq(partialAfterActs(a, handles, completed, h)))

  // qznqvpyr + nytszsml: one attempt of a discard, total over the handles it names, the
  // acts it completes and the failure flag. The act state after the attempt is the one
  // before it plus the acts this attempt completed, for the named handles, and unchanged
  // for every other handle - a failure rolls no completed act back. The registry is
  // `discard_commit`'s outcome for the named handles whose three acts are complete after
  // this attempt and for the same failure flag: a call commits - removes exactly the
  // named open records - exactly when nothing failed, every named handle's three acts
  // are complete and every named handle is an open record; any other call leaves the
  // registry exactly as it stood. Because the attempt's completed acts stay in `acts`,
  // a later call from that partial state sees them and can reach the commit.
  action discardPartial(
    handles: List[str],
    completed: str -> Set[Act],
    failed: bool,
  ): bool = all {
    acts' = partialAfterActState(acts, handles, completed),
    discardCommit(handles, partialPrepared(acts, handles, completed), failed),
  }
}
