// T-discard-commit's check: the commit policy read over the draft registry.
//
// The record's form, its two states and the registry variable `records` are
// `draft_state`'s (tvtnokzu, `specs/drafts/state.qnt`); this module imports them and
// states nothing of its own about a record. What a discard selects and removes comes
// from `specs/drafts/discard.qnt` (qmpskomv, module `draft_discard`): a call names a
// list of handles, it is accepted exactly when every named handle picks out an `open`
// record, and an accepted call removes exactly the records it names.
//
// The commit policy (nwswzlnw) orders that removal last. This check fixes the
// interface the model must carry: module `draft_discard_commit`, one action
// `discardCommit(handles: List[str], prepared: Set[str], failed: bool)` over the
// registry variable `records`, total over those three inputs. `prepared` abstracts the
// per-draft change, workspace and directory work: a handle in `prepared` is a named
// draft whose three acts have all completed before this call decides. `failed` says
// whether any of those acts - or the removal itself, while no record has left the
// store - failed before the commit point. On those inputs the action says: if every
// named handle is prepared, nothing failed, and every named handle is an open record,
// then the call commits and removes exactly the named records; otherwise the call
// refuses and leaves the registry exactly as it was. There is no third outcome and no
// per-handle partial removal.
//
// The check records the registry and the three inputs of each call immediately before
// it, then runs the model's action, so the invariants relate the registry after the
// call to the registry before it and to the inputs the call was given. It seeds a
// registry the prerequisites admit - open drafts and landed records - and explores
// commit traces, refusal traces and failure traces. The change abandonment and the
// workspace forget a real discard performs are jj and filesystem effects and are
// outside this model; nothing here states them.
//
// The check's own helpers carry the `gate` prefix because the TLA+ flattener merges
// this module's definitions with the model's: a model helper sharing a name with one
// of the check's would shadow it and the invariants would silently compare against the
// wrong condition. The model must not define `gateNamed`, `gateAccepted`,
// `gateWithoutNamed` or `gateCommitReady`.
module draft_discard_commit_check {
  import draft_state.* from "./state"
  // Only the one action is imported: the check fixes the interface it calls and does
  // not read any helper the model carries.
  import draft_discard_commit.discardCommit from "./discard_commit"

  // A registry the prerequisites admit, as the first state the checker explores:
  // three open drafts (h1, h2, h3) over two nodes and two landed records (h4, h5),
  // each filed under its own handle. The handles, node ids, edits, workspaces and
  // bases are values this check chooses; nothing here mints a handle, places a
  // workspace or reads a content identity, and a discard never does any of those.
  pure val SEED: str -> Record =
    Map()
      .put("h1", filed("h1", "n1", "e1", { name: "w1", path: "/w/1" }, Map("/n1/a" -> "a1")))
      .put("h2", filed("h2", "n2", "e1", { name: "w2", path: "/w/2" }, Map("/n2/a" -> "a2")))
      .put("h3", filed("h3", "n1", "e2", { name: "w3", path: "/w/3" }, Map("/n1/a" -> "a1")))
      .put("h4", landed(filed("h4", "n1", "e1", { name: "w4", path: "/w/4" }, Map("/n1/a" -> "a9"))))
      .put("h5", landed(filed("h5", "n3", "e3", { name: "w5", path: "/w/5" }, Map("/n3/a" -> "a3"))))

  // One call: the handles it names, the named drafts whose three acts have completed,
  // and whether an act failed before the commit point.
  type Call = {
    handles: List[str],
    prepared: Set[str],
    failed: bool,
  }

  // The calls the checker explores. Together they cover an empty call; one, two and
  // three open records; a handle named twice; prepared extras that are not named; a
  // failure after every named draft is prepared; a failure while some named draft is
  // unprepared; a call with no named draft prepared; a landed record prepared; an open
  // record mixed with a landed one; an open record mixed with a handle the registry
  // does not hold; two landed records; and an absent handle on its own.
  pure val CALLS: List[Call] = List(
    { handles: List(), prepared: Set(), failed: false },
    { handles: List("h1"), prepared: Set("h1"), failed: false },
    { handles: List("h2"), prepared: Set("h2"), failed: false },
    { handles: List("h1", "h3"), prepared: Set("h1", "h3"), failed: false },
    { handles: List("h1", "h2", "h3"), prepared: Set("h1", "h2", "h3"), failed: false },
    { handles: List("h1", "h1"), prepared: Set("h1"), failed: false },
    { handles: List("h1", "h2"), prepared: Set("h1", "h2", "h3", "h4", "h5"), failed: false },
    { handles: List("h1"), prepared: Set("h1"), failed: true },
    { handles: List("h1", "h3"), prepared: Set("h1", "h3"), failed: true },
    { handles: List("h1", "h3"), prepared: Set("h1"), failed: false },
    { handles: List("h1", "h3"), prepared: Set("h1"), failed: true },
    { handles: List("h1", "h3"), prepared: Set(), failed: false },
    { handles: List("h4"), prepared: Set("h4"), failed: false },
    { handles: List("h1", "h4"), prepared: Set("h1", "h4"), failed: false },
    { handles: List("h1", "h_missing"), prepared: Set("h1", "h_missing"), failed: false },
    { handles: List("h4", "h5"), prepared: Set("h4", "h5"), failed: false },
    { handles: List("h_missing"), prepared: Set("h_missing"), failed: false },
    { handles: List("h1", "h3"), prepared: Set("h1", "h3", "h4"), failed: false },
  )

  // The registry as it stood immediately before the step, and the inputs that step's
  // call was given. `call` sets all four from the pre-state, so the invariants compare
  // the registry after the call with the one before it and with the inputs.
  var before: str -> Record
  var offered: List[str]
  var preparedIn: Set[str]
  var failedIn: bool

  // The handles a call names, read as a set: a call is a list, and a handle named
  // twice is still one handle.
  pure def gateNamed(handles: List[str]): Set[str] =
    handles.indices().map(i => handles.nth(i))

  // A call is accepted exactly when every handle it names picks out an `open` record
  // in the registry as it stood before the call (lnpnkvkv through qmpskomv): a handle
  // the registry does not hold, or one that holds a `landed` record, is no subject.
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

  // The commit condition: nothing failed, every named draft's three acts completed,
  // and every named handle is an open record. Only such a call may remove anything.
  pure def gateCommitReady(
    recs: str -> Record,
    handles: List[str],
    prepared: Set[str],
    failed: bool,
  ): bool =
    not(failed) and gateNamed(handles).subseteq(prepared) and gateAccepted(recs, handles)

  // The commit point (nwswzlnw): a record leaves the store only when the call has
  // reached its commit. If any named draft is unprepared or an act failed, nothing is
  // removed; the refusal leaves the registry exactly as it was.
  val NoRemovalBeforeCommit =
    not(gateCommitReady(before, offered, preparedIn, failedIn)) implies records == before

  // The commit itself (nwswzlnw, yomurusp through qmpskomv): a call that reaches the
  // commit removes exactly the records it names, every one of them open, and nothing
  // else.
  val CommitIsExact =
    gateCommitReady(before, offered, preparedIn, failedIn) implies
      records == gateWithoutNamed(before, offered)

  // A failure before the commit refuses the invocation (nwswzlnw): the registry is
  // exactly as it stood and no named record has left it.
  val FailureLeavesRegistry =
    failedIn implies records == before

  // Every named draft that had a record before a failed call still has that same
  // record - so an open draft is still open and no named record began to leave.
  val FailureLeavesNamedOpen =
    failedIn implies
      offered.indices().forall(i =>
        if (before.keys().contains(offered.nth(i)))
          records.keys().contains(offered.nth(i)) and
            records.get(offered.nth(i)) == before.get(offered.nth(i))
        else
          true)

  // Retention (yomurusp through qmpskomv): every record the call did not name stays
  // at its handle, field for field, and no record the call did not name is dropped.
  // Nothing is rewritten, renamed or moved, so no record turns from `open` to
  // `landed` and no record's fields change.
  val OthersUntouched =
    before.keys().forall(h =>
      if (records.keys().contains(h))
        records.get(h) == before.get(h)
      else
        gateNamed(offered).contains(h))

  // The landed end (yuomrusm through tvtnokzu): a record that had landed before the
  // call is still there afterwards, unchanged. Removal is confined to a draft that
  // did not land.
  val LandedNeverRemoved =
    before.keys().forall(h =>
      if (before.get(h).state == Landed)
        records.keys().contains(h) and records.get(h) == before.get(h)
      else
        true)

  // Nothing is added (lnpnkvkv through qmpskomv): a discard files no record, so the
  // registry after a call holds a subset of what it held before.
  val NoRecordAdded =
    records.keys().subseteq(before.keys())

  // The removal is one act of the whole call: either no record leaves or every named
  // record leaves. There is no per-handle partial registry removal, so the registry
  // after a call is either exactly the one before it or exactly the one before it
  // without every named handle.
  val WholeCallAtomic =
    records == before or records == gateWithoutNamed(before, offered)

  // The first state: the seeded registry, with no call made yet. `draft_state` already
  // carries an `init` (the empty registry), so this module's initializer is
  // `initCheck`; the verifier is run with `--init=initCheck`.
  action initCheck = all {
    records' = SEED,
    before' = SEED,
    offered' = List(),
    preparedIn' = Set(),
    failedIn' = false,
  }

  // One call, given its three inputs explicitly. The check records the registry and
  // the inputs before the call and lets the model step `records`.
  action call(handles: List[str], prepared: Set[str], failed: bool): bool = all {
    before' = records,
    offered' = handles,
    preparedIn' = prepared,
    failedIn' = failed,
    discardCommit(handles, prepared, failed),
  }

  // One step of the checker is one call drawn from the pool.
  action step = any {
    nondet i = oneOf(CALLS.indices())
    call(CALLS.nth(i).handles, CALLS.nth(i).prepared, CALLS.nth(i).failed)
  }

  // A commit removes the one open record it names and keeps every other record.
  run commitOneTest =
    initCheck
      .then(call(List("h1"), Set("h1"), false))
      .expect(and {
        records.keys() == Set("h2", "h3", "h4", "h5"),
        records.get("h2") == SEED.get("h2"),
        records.get("h3") == SEED.get("h3"),
        records.get("h4") == SEED.get("h4"),
        records.get("h5") == SEED.get("h5"),
      })

  // A commit with two named drafts prepared removes both and keeps the third open
  // record and both landed records.
  run commitTwoTest =
    initCheck
      .then(call(List("h1", "h3"), Set("h1", "h3"), false))
      .expect(and {
        records.keys() == Set("h2", "h4", "h5"),
        records.get("h2") == SEED.get("h2"),
        records.get("h4") == SEED.get("h4"),
        records.get("h5") == SEED.get("h5"),
      })

  // Prepared handles the call does not name are not removed: only the named open
  // records leave.
  run commitPreparedExtrasTest =
    initCheck
      .then(call(List("h1", "h2"), Set("h1", "h2", "h3", "h4", "h5"), false))
      .expect(and {
        records.keys() == Set("h3", "h4", "h5"),
        records.get("h3") == SEED.get("h3"),
        records.get("h4") == SEED.get("h4"),
        records.get("h5") == SEED.get("h5"),
      })

  // A handle named twice names the same draft once, so the commit removes it once.
  run commitDuplicateTest =
    initCheck
      .then(call(List("h1", "h1"), Set("h1"), false))
      .expect(records.keys() == Set("h2", "h3", "h4", "h5"))

  // A failure after every named draft is prepared still refuses before the commit:
  // the registry is exactly as it was and the named record is still open.
  run failureAfterPreparedTest =
    initCheck
      .then(call(List("h1"), Set("h1"), true))
      .expect(and {
        records == SEED,
        records.get("h1") == SEED.get("h1"),
        records.get("h1").state == Open,
      })

  // A failure with every named draft prepared on a two-handle call leaves both
  // records at their handles.
  run failureAllPreparedTest =
    initCheck
      .then(call(List("h1", "h3"), Set("h1", "h3"), true))
      .expect(records == SEED)

  // No removal before every named draft is prepared: with one of two named drafts
  // prepared and no failure, the call does not commit and nothing leaves the store.
  run partialPreparedTest =
    initCheck
      .then(call(List("h1", "h3"), Set("h1"), false))
      .expect(records == SEED)

  // A failure while a named draft is unprepared leaves the registry exactly as it
  // was.
  run partialPreparedFailureTest =
    initCheck
      .then(call(List("h1", "h3"), Set("h3"), true))
      .expect(records == SEED)

  // No named draft prepared: no commit.
  run unpreparedTest =
    initCheck
      .then(call(List("h1", "h3"), Set(), false))
      .expect(records == SEED)

  // A landed record is no subject even when its handle is prepared: the call refuses
  // whole and the landed record stays.
  run landedPreparedRefusedTest =
    initCheck
      .then(call(List("h4"), Set("h4"), false))
      .expect(and {
        records == SEED,
        records.get("h4") == SEED.get("h4"),
      })

  // A call mixing an open record with a landed one refuses whole, so the open record
  // it named is still there.
  run mixedOpenLandedRefusedTest =
    initCheck
      .then(call(List("h1", "h4"), Set("h1", "h4"), false))
      .expect(and {
        records == SEED,
        records.keys().contains("h1"),
      })

  // A call mixing an open record with a handle the registry does not hold refuses
  // whole.
  run absentRefusedTest =
    initCheck
      .then(call(List("h1", "h_missing"), Set("h1", "h_missing"), false))
      .expect(records == SEED)

  // A call naming only landed records refuses whole.
  run allLandedRefusedTest =
    initCheck
      .then(call(List("h4", "h5"), Set("h4", "h5"), false))
      .expect(records == SEED)

  // An empty call names no draft and removes nothing.
  run emptyCallTest =
    initCheck
      .then(call(List(), Set(), false))
      .expect(records == SEED)

  // A failure leaves the named record open, and a later invocation commits over it:
  // the record a failed call left behind is still the open draft another call ends.
  run failureKeepsOpenThenCommitTest =
    initCheck
      .then(call(List("h1", "h2"), Set("h1", "h2"), true))
      .then(call(List("h1", "h2"), Set("h1", "h2"), false))
      .expect(and {
        records.keys() == Set("h3", "h4", "h5"),
        records.get("h3") == SEED.get("h3"),
      })
}
