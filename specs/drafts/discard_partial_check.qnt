// T-discard-partial's check: the partial policy read over the draft registry and the
// commit model.
//
// The record's form, its two states, its base and the registry variable `records` are
// `draft_state`'s (tvtnokzu, `specs/drafts/state.qnt`); this module imports them and
// states nothing of its own about a record. `specs/drafts/discard_commit.qnt`
// (nytszsml) is the commit policy of one discard call, and the policy read here
// (qznqvpyr) is the one it leaves open: what a failure before the commit leaves of the
// acts that already completed, and what a second invocation of the same discard finds.
//
// THE INTERFACE THIS CHECK FIXES. The model at `specs/drafts/discard_partial.qnt` must
// carry:
//
//   module draft_discard_partial
//
//   type Act = Change | Forget | Remove
//     the three acts one discard performs for a draft: the abandonment of its change,
//     the forget of its workspace and the removal of its directory. The three names,
//     not their order, are the interface; nothing here fixes which act runs first.
//
//   var acts: str -> Set[Act]
//     the acts a discard has completed, per draft handle - the per-draft act state.
//     A handle the map does not hold has had no act completed. This is the only
//     variable the check reads besides `records`, and the model must step both.
//
//   action discardPartial(
//     handles: List[str],
//     completed: str -> Set[Act],
//     failed: bool,
//   ): bool
//     one attempt of a discard over the handles it names. `completed` maps a named
//     handle to the acts this attempt completes beyond the ones already completed
//     (a handle absent from it completes nothing); `failed` says whether this attempt
//     failed before the commit. The action is total over those three inputs: every
//     handle list, every `completed` map and either failure flag has exactly one
//     outcome, and it steps no variable but `records` and `acts`.
//
// The model imports the registry from `./state` and the commit model from
// `./discard_commit`; the commit itself is the commit model's action, not a second
// reading of it here. The model may carry helpers and its own initializer under a name
// other than `init` (`draft_state` already exports `init`, and redefining it in the
// importing module is a name conflict), but it must not define any `gate`-prefixed
// name: the TLA+ flattener merges the model's definitions with this module's, and a
// shared name would shadow the check's condition.
//
// THE CONTRACT THE INVARIANTS PIN. On every step, with the registry and the act state
// recorded immediately before it and the inputs that step's call was given:
//
//   (a) Acts only grow. For every named handle the act state after the step is the one
//       before it together with the acts this attempt completed - never less, so a
//       failure never rolls back an act that completed; a handle the call does not
//       name keeps its act state, and no handle gains one.
//   (b) A failed attempt leaves the registry exactly as it stood and every named
//       record at its handle, field for field - an open draft is still open, and the
//       acts it completed still stand.
//   (c) A call reaches the commit exactly when it failed nothing, every named
//       handle's three acts are complete, and every named handle picks out an open
//       record; that call removes exactly the named records and nothing else. Any
//       other call leaves the registry unchanged. Because the retry step below is
//       enabled in every reachable state over an open named draft, a second
//       invocation that completes the remaining acts - or that finds every act
//       already cleared - reaches the commit from any reachable partial act state.
//
// The check also carries the retention the commit model already fixes: a landed record
// is never removed or rewritten, no record the call does not name changes, and a
// discard files no record.
//
// The check's own helpers carry the `gate` prefix for the flattening reason above. Its
// seeded handles, node ids, edits, workspaces, bases and act sets are values it
// chooses; nothing here mints a handle, places a workspace or reads a content
// identity. The change abandonment, the workspace forget and the directory removal a
// real discard performs are jj and filesystem effects and are outside this model.
module draft_discard_partial_check {
  import draft_state.* from "./state"
  // Only the interface is imported: the action, the act state and the three acts. The
  // check reads no helper the model carries.
  import draft_discard_partial.discardPartial from "./discard_partial"
  import draft_discard_partial.acts from "./discard_partial"
  import draft_discard_partial.Act from "./discard_partial"
  import draft_discard_partial.Change from "./discard_partial"
  import draft_discard_partial.Forget from "./discard_partial"
  import draft_discard_partial.Remove from "./discard_partial"

  // A registry the prerequisites admit, as the first state the checker explores:
  // three open drafts (h1, h2, h3) over two nodes and two landed records (h4, h5),
  // each filed under its own handle through the state module's own constructors.
  pure val SEED: str -> Record =
    Map()
      .put("h1", filed("h1", "n1", "e1", { name: "w1", path: "/w/1" }, Map("/n1/a" -> "a1")))
      .put("h2", filed("h2", "n2", "e1", { name: "w2", path: "/w/2" }, Map("/n2/a" -> "a2")))
      .put("h3", filed("h3", "n1", "e2", { name: "w3", path: "/w/3" }, Map("/n1/a" -> "a1")))
      .put("h4", landed(filed("h4", "n1", "e1", { name: "w4", path: "/w/4" }, Map("/n1/a" -> "a9"))))
      .put("h5", landed(filed("h5", "n3", "e3", { name: "w5", path: "/w/5" }, Map("/n3/a" -> "a3"))))

  // One call: the handles it names, the acts it completes on them, and whether it
  // failed before the commit.
  type Call = {
    handles: List[str],
    completed: str -> Set[Act],
    failed: bool,
  }

  // The calls the checker explores. Together they cover an empty call; a failure after
  // one act, after two, and after all three; a non-failing attempt that completes one
  // act and one that completes two; a commit in one call; a two-draft failure with one
  // draft fully cleared and the other part-way; a two-draft commit; a two-draft call
  // whose second draft is incomplete; a two-draft failure after every act; a landed
  // record prepared; an open record mixed with a landed one; and an open record mixed
  // with a handle the registry does not hold. A call that finds every act already
  // cleared is the retry below, since `completed` is then empty.
  pure val CALLS: List[Call] = List(
    { handles: List(), completed: Map(), failed: false },
    { handles: List("h1"), completed: Map("h1" -> Set(Change)), failed: true },
    { handles: List("h1"), completed: Map("h1" -> Set(Change, Forget)), failed: true },
    { handles: List("h1"), completed: Map("h1" -> Set(Change, Forget, Remove)), failed: true },
    { handles: List("h1"), completed: Map("h1" -> Set(Change)), failed: false },
    { handles: List("h1"), completed: Map("h1" -> Set(Forget)), failed: false },
    {
      handles: List("h1"),
      completed: Map("h1" -> Set(Change, Forget)),
      failed: false,
    },
    {
      handles: List("h1"),
      completed: Map("h1" -> Set(Change, Forget, Remove)),
      failed: false,
    },
    {
      handles: List("h1", "h2"),
      completed: Map("h1" -> Set(Change, Forget), "h2" -> Set(Change)),
      failed: true,
    },
    {
      handles: List("h1", "h2"),
      completed: Map(
        "h1" -> Set(Change, Forget, Remove),
        "h2" -> Set(Change, Forget, Remove),
      ),
      failed: false,
    },
    {
      handles: List("h1", "h2"),
      completed: Map("h1" -> Set(Change, Forget, Remove), "h2" -> Set(Change)),
      failed: false,
    },
    {
      handles: List("h1", "h3"),
      completed: Map(
        "h1" -> Set(Change, Forget, Remove),
        "h3" -> Set(Change, Forget, Remove),
      ),
      failed: true,
    },
    {
      handles: List("h4"),
      completed: Map("h4" -> Set(Change, Forget, Remove)),
      failed: false,
    },
    {
      handles: List("h1", "h4"),
      completed: Map(
        "h1" -> Set(Change, Forget, Remove),
        "h4" -> Set(Change, Forget, Remove),
      ),
      failed: false,
    },
    {
      handles: List("h1", "h_missing"),
      completed: Map(
        "h1" -> Set(Change, Forget, Remove),
        "h_missing" -> Set(Change, Forget, Remove),
      ),
      failed: false,
    },
  )

  // The registry and the act state immediately before the step, the inputs that step's
  // call was given, and whether the step was the retry below (and over which handle).
  var before: str -> Record
  var actsBefore: str -> Set[Act]
  var offered: List[str]
  var completedIn: str -> Set[Act]
  var failedIn: bool
  var retryIn: bool
  var retried: str

  // The three acts, read by the check itself rather than by any helper of the model.
  pure def gateAll: Set[Act] = Set(Change, Forget, Remove)

  // The handles a call names, read as a set: a handle named twice is one handle.
  pure def gateNamed(handles: List[str]): Set[str] =
    handles.indices().map(i => handles.nth(i))

  // The acts completed for a handle, where a handle the map does not hold has none.
  pure def gateActsOf(a: str -> Set[Act], h: str): Set[Act] =
    if (a.keys().contains(h)) a.get(h) else Set()

  // The acts a call completes for a handle, where a handle its map does not hold
  // completes nothing.
  pure def gateCompleted(h: str, completed: str -> Set[Act]): Set[Act] =
    if (completed.keys().contains(h)) completed.get(h) else Set()

  // The act state a named handle must hold after the step, and the unchanged act state
  // a handle the call does not name must hold.
  pure def gateAfterActs(
    a: str -> Set[Act],
    handles: List[str],
    completed: str -> Set[Act],
    h: str,
  ): Set[Act] =
    if (gateNamed(handles).contains(h))
      gateActsOf(a, h).union(gateCompleted(h, completed))
    else
      gateActsOf(a, h)

  // The acts a draft still has to complete: the three, less the ones already done.
  pure def gateRest(a: str -> Set[Act], h: str): Set[Act] =
    gateAll.filter(x => not(gateActsOf(a, h).contains(x)))

  // A call is accepted exactly when every handle it names picks out an open record in
  // the registry as it stood before the call (qmpskomv through nytszsml): a handle the
  // registry does not hold, or one that holds a landed record, is no subject.
  pure def gateAccepted(recs: str -> Record, handles: List[str]): bool =
    handles.indices().forall(i =>
      if (recs.keys().contains(handles.nth(i)))
        recs.get(handles.nth(i)).state == Open
      else
        false)

  // The registry with every named handle removed, and nothing else.
  pure def gateWithoutNamed(recs: str -> Record, handles: List[str]): str -> Record =
    recs.keys()
      .filter(h => not(gateNamed(handles).contains(h)))
      .mapBy(h => recs.get(h))

  // The commit condition: nothing failed, every named draft's three acts are complete
  // after this attempt, and every named handle is an open record. Only such a call may
  // remove anything.
  pure def gateCommitReady(
    a: str -> Set[Act],
    recs: str -> Record,
    handles: List[str],
    completed: str -> Set[Act],
    failed: bool,
  ): bool =
    not(failed) and
      gateNamed(handles).forall(h =>
        gateAll.subseteq(gateAfterActs(a, handles, completed, h))) and
      gateAccepted(recs, handles)

  // The removal is last: before the commit condition holds nothing leaves the store.
  val NoRemovalBeforeCommit =
    not(gateCommitReady(actsBefore, before, offered, completedIn, failedIn)) implies
      records == before

  // At the commit the call removes exactly the records it names, every one of them
  // open, and nothing else. Together with the previous invariant this fixes `records`
  // after every call, so a call has the two outcomes the policy gives it and no third.
  val CommitIsExact =
    gateCommitReady(actsBefore, before, offered, completedIn, failedIn) implies
      records == gateWithoutNamed(before, offered)

  // A failure before the commit leaves the registry exactly as it stood.
  val FailureLeavesRegistry =
    failedIn implies records == before

  // A failed call leaves every named record at its handle, field for field - an open
  // draft is still an open draft and is still there to be ended.
  val FailureLeavesNamedOpen =
    failedIn implies
      offered.indices().forall(i =>
        if (before.keys().contains(offered.nth(i)))
          records.keys().contains(offered.nth(i)) and
            records.get(offered.nth(i)) == before.get(offered.nth(i))
        else
          true)

  // The acts a failed call completed stand: what the attempt finished before it failed
  // is in the act state after it, never rolled back.
  val FailureKeepsCompletedActs =
    failedIn implies
      gateNamed(offered).forall(h =>
        gateCompleted(h, completedIn).subseteq(gateActsOf(acts, h)))

  // No rollback (a): a handle's act state never shrinks, no handle loses its entry, and
  // no handle that the call does not name gains one.
  val ActsOnlyGrow =
    actsBefore.keys().forall(h =>
      acts.keys().contains(h) and actsBefore.get(h).subseteq(acts.get(h))) and
      acts.keys().forall(h => actsBefore.keys().contains(h) or gateNamed(offered).contains(h))

  // The act state after a step is exactly the one before it plus the acts the attempt
  // completed on the handles it named - so an attempt that completed an act is
  // observed by it, and an attempt that completed none adds nothing.
  val ActsGrowByCall =
    gateNamed(offered).forall(h =>
      acts.keys().contains(h) and
        acts.get(h) == gateAfterActs(actsBefore, offered, completedIn, h))

  // A handle the call does not name keeps its act state exactly.
  val UnnamedActsUntouched =
    actsBefore.keys().forall(h =>
      if (gateNamed(offered).contains(h))
        true
      else
        acts.keys().contains(h) and acts.get(h) == actsBefore.get(h))

  // The retry (c): a second invocation that completes every act still to be done and
  // fails nothing reaches the commit and removes exactly the named record, whatever
  // partial act state it starts from. The retry is enabled in every state over an open
  // named record, so this holds from every reachable partial act state.
  val RetryCommits =
    retryIn implies records == gateWithoutNamed(before, List(retried))

  // The retry really completes the three acts, so `RetryCommits` is the commit at the
  // end of a discard that finished a part-cleared draft, not a stuttering step.
  val RetryClearsActs =
    retryIn implies gateActsOf(acts, retried) == gateAll

  // Retention: every record the call does not name stays at its handle, field for
  // field, and no record it does not name is dropped.
  val OthersUntouched =
    before.keys().forall(h =>
      if (records.keys().contains(h))
        records.get(h) == before.get(h)
      else
        gateNamed(offered).contains(h))

  // The landed end: a record that had landed before the call is still there afterwards,
  // unchanged. Removal is confined to a draft that did not land.
  val LandedNeverRemoved =
    before.keys().forall(h =>
      if (before.get(h).state == Landed)
        records.keys().contains(h) and records.get(h) == before.get(h)
      else
        true)

  // A discard files no record: the registry after a call holds a subset of what it held
  // before.
  val NoRecordAdded =
    records.keys().subseteq(before.keys())

  // The removal is one act of the whole call: either no record leaves or every named
  // record leaves, with no per-handle partial removal.
  val WholeCallAtomic =
    records == before or records == gateWithoutNamed(before, offered)

  // The first state: the seeded registry, no act completed, no call made. `draft_state`
  // already carries an `init` (the empty registry), so this module's initializer is
  // `initCheck`; the verifier is run with `--init=initCheck`.
  action initCheck = all {
    records' = SEED,
    before' = SEED,
    acts' = Map(),
    actsBefore' = Map(),
    offered' = List(),
    completedIn' = Map(),
    failedIn' = false,
    retryIn' = false,
    retried' = "",
  }

  // One call, given its three inputs explicitly. The check records the registry and the
  // act state before the call and lets the model step `records` and `acts`.
  action call(handles: List[str], completed: str -> Set[Act], failed: bool): bool = all {
    before' = records,
    actsBefore' = acts,
    offered' = handles,
    completedIn' = completed,
    failedIn' = failed,
    retryIn' = false,
    retried' = "",
    discardPartial(handles, completed, failed),
  }

  // The retry: a second invocation over one open named draft that completes every act
  // still to be done and fails nothing. It is enabled in every state whose named
  // record is open, so the invariants relate it to every reachable partial act state.
  action retry(handle: str): bool = all {
    gateAccepted(records, List(handle)),
    before' = records,
    actsBefore' = acts,
    offered' = List(handle),
    completedIn' = Map().put(handle, gateRest(acts, handle)),
    failedIn' = false,
    retryIn' = true,
    retried' = handle,
    discardPartial(List(handle), Map().put(handle, gateRest(acts, handle)), false),
  }

  // One step of the checker is one call drawn from the pool, or the retry over one open
  // named draft.
  action step = any {
    nondet i = oneOf(CALLS.indices())
    call(CALLS.nth(i).handles, CALLS.nth(i).completed, CALLS.nth(i).failed),
    nondet h = oneOf(Set("h1", "h2", "h3"))
    retry(h),
  }

  // A failure after the first act leaves that act standing, the registry as it was and
  // the named draft still open.
  run oneActThenFailureTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change)), true))
      .expect(and {
        acts.get("h1") == Set(Change),
        records == SEED,
        records.get("h1").state == Open,
      })

  // A failure after two acts leaves both acts standing and the record untouched.
  run twoActsThenFailureTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change, Forget)), true))
      .expect(and {
        acts.get("h1") == Set(Change, Forget),
        records == SEED,
        records.get("h1") == SEED.get("h1"),
      })

  // A failure after all three acts - the acts all cleared, the commit not reached -
  // still leaves the registry as it was and the record open.
  run allActsThenFailureTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change, Forget, Remove)), true))
      .expect(and {
        acts.get("h1") == Set(Change, Forget, Remove),
        records == SEED,
        records.get("h1").state == Open,
      })

  // A retry after a failure completes the remaining acts and reaches the commit: the
  // named record goes and every other record stays.
  run retryAfterOneActTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change)), true))
      .then(call(List("h1"), Map("h1" -> Set(Forget, Remove)), false))
      .expect(and {
        records.keys() == Set("h2", "h3", "h4", "h5"),
        acts.get("h1") == Set(Change, Forget, Remove),
      })

  // A retry that completes one act at a time does not commit while an act is still
  // outstanding, and commits when the last one is done.
  run retryStepByStepTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change)), true))
      .then(call(List("h1"), Map("h1" -> Set(Forget)), false))
      .expect(records == SEED)
      .then(call(List("h1"), Map("h1" -> Set(Remove)), false))
      .expect(and {
        records.keys() == Set("h2", "h3", "h4", "h5"),
        acts.get("h1") == Set(Change, Forget, Remove),
      })

  // A retry over a draft whose every act a failed call already cleared completes
  // nothing and still reaches the commit: a cleared part is not a failure and does not
  // refuse the second invocation.
  run retryAfterEveryActFailedTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change, Forget, Remove)), true))
      .then(call(List("h1"), Map(), false))
      .expect(records.keys() == Set("h2", "h3", "h4", "h5"))

  // A call that completes every act without failing removes the one record it names
  // and keeps every other record.
  run commitInOneCallTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change, Forget, Remove)), false))
      .expect(and {
        records.keys() == Set("h2", "h3", "h4", "h5"),
        records.get("h2") == SEED.get("h2"),
      })

  // A failure touches no record the call did not name.
  run failureLeavesOthersTest =
    initCheck
      .then(call(List("h1"), Map("h1" -> Set(Change, Forget)), true))
      .expect(and {
        records.get("h2") == SEED.get("h2"),
        records.get("h3") == SEED.get("h3"),
        records.get("h4") == SEED.get("h4"),
        records.get("h5") == SEED.get("h5"),
      })

  // A landed record is never removed, even by a call that clears every act on it and
  // fails nothing: the call refuses whole and both landed records stay as filed.
  run landedNeverTouchedTest =
    initCheck
      .then(call(List("h4"), Map("h4" -> Set(Change, Forget, Remove)), false))
      .expect(and {
        records == SEED,
        records.get("h4").state == Landed,
        records.get("h5").state == Landed,
      })

  // A two-draft failure leaves one draft fully cleared, the other part-way, and both
  // records as they were.
  run twoDraftsPartialFailureTest =
    initCheck
      .then(call(
        List("h1", "h2"),
        Map("h1" -> Set(Change, Forget, Remove), "h2" -> Set(Change)),
        true,
      ))
      .expect(and {
        records == SEED,
        acts.get("h1") == Set(Change, Forget, Remove),
        acts.get("h2") == Set(Change),
      })

  // A retry over both drafts completes the part-way one on the strength of the acts
  // the failed call left and removes both named records.
  run retryTwoDraftsTest =
    initCheck
      .then(call(
        List("h1", "h2"),
        Map("h1" -> Set(Change, Forget, Remove), "h2" -> Set(Change)),
        true,
      ))
      .then(call(List("h1", "h2"), Map("h2" -> Set(Forget, Remove)), false))
      .expect(records.keys() == Set("h3", "h4", "h5"))

  // An empty call names no draft, completes no act and removes nothing - the action is
  // enabled on it too, so the checker is not confined to calls that happen to have a
  // subject.
  run emptyCallTest =
    initCheck
      .then(call(List(), Map(), false))
      .expect(records == SEED)

  // A call mixing an open record with a handle the registry does not hold is refused
  // whole: no record leaves, and the action is enabled on an absent handle too.
  run absentRefusedTest =
    initCheck
      .then(call(
        List("h1", "h_missing"),
        Map("h1" -> Set(Change, Forget, Remove), "h_missing" -> Set(Change, Forget, Remove)),
        false,
      ))
      .expect(and {
        records == SEED,
        records.get("h1").state == Open,
      })
}
