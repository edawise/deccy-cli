// T-discard's check: the registry-side discard over the draft registry.
//
// The record's form, its two states, its lifecycle and the registry variable are
// `draft_state`'s (tvtnokzu, `specs/drafts/state.qnt`); this module imports them and
// states nothing of its own about a record. What a discard *is* comes from
// `specs/drafts/discard.qnt` (qmpskomv): module `draft_discard`, an action
// `discard(handles: List[str])` over `records`, total over the handle list it is
// given - every call either removes exactly the records it names, each of them open,
// or is a whole-call refusal that leaves the registry exactly as it was.
//
// This module supplies the step the checker runs. It records the registry as it stood
// immediately before each call and the handles that call named, then runs the discard,
// so the invariants below relate the registry after the call to the registry before
// it. The call pool spans the cases the prerequisites name - open records, landed
// records, a handle the registry does not hold, and mixtures - and each run below
// executes one case and asserts its outcome. The change abandonment and the workspace
// forget a real discard performs are jj and filesystem effects: they are outside this
// model, and nothing here states them.
module draft_discard_check {
  import draft_state.* from "./state"
  import draft_discard.* from "./discard"

  // A registry the prerequisites admit, as the first state the checker explores: three
  // open drafts (h1, h2, h3) over two nodes and two landed records (h4, h5), each filed
  // under its own handle. The handles, node ids, edits, workspaces and bases are values
  // this check chooses; nothing here derives a handle, places a workspace or reads a
  // content identity, and a discard never mints or rewrites one.
  pure val SEED: str -> Record =
    Map()
      .put("h1", filed("h1", "n1", "e1", { name: "w1", path: "/w/1" }, Map("/n1/a" -> "a1")))
      .put("h2", filed("h2", "n2", "e1", { name: "w2", path: "/w/2" }, Map("/n2/a" -> "a2")))
      .put("h3", filed("h3", "n1", "e2", { name: "w3", path: "/w/3" }, Map("/n1/a" -> "a1")))
      .put("h4", landed(filed("h4", "n1", "e1", { name: "w4", path: "/w/4" }, Map("/n1/a" -> "a9"))))
      .put("h5", landed(filed("h5", "n3", "e3", { name: "w5", path: "/w/5" }, Map("/n3/a" -> "a3"))))

  // The registry as it stood immediately before the step, and the handles the step's
  // call named. `step` sets both from the pre-state, so the invariants can compare the
  // registry after the call with the one before it.
  var before: str -> Record
  var offered: List[str]

  // The handles a call names, read as a set: a call is a list, and a handle named twice
  // is still one handle.
  pure def named(handles: List[str]): Set[str] =
    handles.indices().map(i => handles.nth(i))

  // A call is accepted exactly when every handle it names picks out an `open` record in
  // the registry as it stood before the call (lnpnkvkv, yuomrusm): a handle the
  // registry does not hold, or one that holds a `landed` record, is no subject at all.
  pure def accepted(recs: str -> Record, handles: List[str]): bool =
    handles.indices().forall(i =>
      if (recs.keys().contains(handles.nth(i)))
        recs.get(handles.nth(i)).state == Open
      else
        false)

  // The registry with every named handle removed, and nothing else.
  pure def withoutNamed(recs: str -> Record, handles: List[str]): str -> Record =
    recs.keys()
      .filter(h => not(named(handles).contains(h)))
      .mapBy(h => recs.get(h))

  // Handle selection, removal and the whole-call refusal (lnpnkvkv, yomurusp): an
  // accepted call leaves exactly the records it named removed; a refused call leaves
  // the registry exactly as it was. There is no third outcome.
  val SelectionIsExact =
    if (accepted(before, offered))
      records == withoutNamed(before, offered)
    else
      records == before

  // Retention (yomurusp): every record the call did not name stays at its handle, field
  // for field, and no record the call did not name is dropped. Nothing is rewritten,
  // renamed or moved to another handle.
  val OthersUntouched =
    before.keys().forall(h =>
      if (records.keys().contains(h))
        records.get(h) == before.get(h)
      else
        named(offered).contains(h))

  // The landed end (yomurusp, slwvtspu): a record that had landed before the call is
  // still there afterwards, unchanged - removal is confined to a draft that did not
  // land.
  val LandedNeverRemoved =
    before.keys().forall(h =>
      if (before.get(h).state == Landed)
        if (records.keys().contains(h)) records.get(h) == before.get(h) else false
      else
        true)

  // Nothing is added (lnpnkvkv): a discard writes no node and no change, and files no
  // record, so the registry after a call holds a subset of what it held before.
  val NoRecordAdded =
    records.keys().subseteq(before.keys())

  // The calls the checker explores. Together they cover an empty call, a single open
  // record, a single landed record, a handle the registry does not hold, two open
  // records, an open record plus a landed one, an open record plus an absent handle,
  // two landed records, a handle named twice, and every open record.
  pure val CALLS: Set[List[str]] = Set(
    List(),
    List("h1"),
    List("h2"),
    List("h4"),
    List("h_missing"),
    List("h1", "h3"),
    List("h1", "h4"),
    List("h1", "h_missing"),
    List("h4", "h5"),
    List("h1", "h1"),
    List("h1", "h2", "h3"),
    List("h1", "h2", "h3", "h4")
  )

  // The first state: the seeded registry, with no call made yet. `draft_state` already
  // carries an `init` (the empty registry), so this module's initializer is
  // `initCheck`; the verifier is run with `--init=initCheck`.
  action initCheck = all {
    records' = SEED,
    before' = SEED,
    offered' = List(),
  }

  // One step is one discard call over the registry. The check records the registry and
  // the handles before the call; the discard module steps `records`.
  action step = any {
    nondet handles = oneOf(CALLS)
    all {
      before' = records,
      offered' = handles,
      discard(handles),
    }
  }

  // The same call as the runs execute it: the registry is stepped by the discard and
  // the check's own bookkeeping is left as it stands.
  action call(handles: List[str]): bool = all {
    before' = before,
    offered' = offered,
    discard(handles),
  }

  // An accepted call removes exactly the open record it names and keeps every other
  // record at its handle.
  run discardOneTest =
    initCheck
      .then(call(List("h1")))
      .expect(and {
        records.keys() == Set("h2", "h3", "h4", "h5"),
        records.get("h2") == SEED.get("h2"),
        records.get("h3") == SEED.get("h3"),
        records.get("h4") == SEED.get("h4"),
        records.get("h5") == SEED.get("h5"),
      })

  // An accepted call naming two open records removes both, keeps the third open record
  // at its handle, and leaves both landed records untouched.
  run discardManyTest =
    initCheck
      .then(call(List("h1", "h3")))
      .expect(and {
        records.keys() == Set("h2", "h4", "h5"),
        records.get("h2") == SEED.get("h2"),
        records.get("h4") == SEED.get("h4"),
        records.get("h5") == SEED.get("h5"),
      })

  // A call naming a landed record refuses whole: every record is still at its handle
  // and the landed record is still landed.
  run discardLandedRefusedTest =
    initCheck
      .then(call(List("h4")))
      .expect(and {
        records.keys() == SEED.keys(),
        records.get("h4") == SEED.get("h4"),
        records.get("h1") == SEED.get("h1"),
      })

  // A call naming a handle the registry does not hold refuses whole.
  run discardAbsentRefusedTest =
    initCheck
      .then(call(List("h_missing")))
      .expect(records == SEED)

  // A call mixing an open record with a landed one refuses whole: the open record it
  // named is still there.
  run discardMixedRefusedTest =
    initCheck
      .then(call(List("h1", "h4")))
      .expect(and {
        records == SEED,
        records.keys().contains("h1"),
      })

  // A call naming only landed records refuses whole.
  run discardAllLandedRefusedTest =
    initCheck
      .then(call(List("h4", "h5")))
      .expect(records == SEED)
}
