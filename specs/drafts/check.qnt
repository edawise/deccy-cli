// T-draft-check: the draft registry's five leg models composed into one step, with the
// invariants a checker runs over all traces to the bound. The legs are imported, never
// restated: the record, its two states, its constructor and its content-identity base
// (tvtnokzu), the opening call (rnwlstks), the landing, its node-wide stale test and its
// refusals (tvsqxrmo), the reconcile reading and the listing (svmuxsmq), and remove's guard
// (kzltplnz). This module declares no type, no field and no variable of its own, and names
// no operation the legs do not declare: `records` is the state module's variable and every
// action below is one of the legs'.
//
// What the harness supplies. Each leg takes as arguments the values it does not derive:
// `open` takes the call's node list, edit, handles, workspaces and bases; `land` takes the
// handle and the owned set as it stands at the landing; each reading takes jj's answer; the
// guard takes the node. The harness's pools are small and fixed, so that what a state can
// show is enough to check the claims:
//
//   - two handles, two node ids, one edit and a pool of two workspaces - the registry can
//     hold at most two records, which is the bound `BoundedByHandles` states;
//   - two content bases: one equals the owned set every landing reads and one differs from
//     it at a shared path, so staleness is a function of the record's own base and is
//     readable from the state;
//   - three of jj's answers to the existence question: no workspace, every workspace, and
//     every workspace but the first.
//
// One harness discipline, stated because it is the harness's and not the model's. A node's
// drafts are filed before that node's first landing and never after it (`unlanded`). `open`
// places no such condition (rnwlstks) and nothing below claims it does; the discipline
// exists because a state cannot tell "the stale sibling was filed after the landing" from
// "the landing was taken while the sibling was stale", and the first order is removed here
// so that the second is the only way to reach a landed record beside a stale open record on
// one node. That is what lets the node-wide rule of tvsqxrmo be stated as a state invariant
// (`NoStaleSiblingLands`); it removes an order, not a case - the node-wide refusal itself is
// reachable, from the state in which a fresh open record sits beside a stale sibling.
//
// What the step reaches. Opening one name, two names and the refused empty call; `land` on
// a fresh record of a node whose open records are all fresh; `refuseStale` on a fresh record
// whose node holds a stale sibling - the node-wide test, where the addressed record's own
// freshness would permit the landing - and on the stale record itself; `refuseUnknown` on a
// handle no record holds; both readings against all three answers, including the absent-
// workspace answer; and remove's guarded refusal. The stale sibling is reachable because the
// harness files one handle under the base that matches the owned set and the other under a
// base that differs at a shared path. What it does not reach is outside the legs, not inside
// this composition: the refusal's wording, order, stream and exit status, the placement of
// changed paths, the scan of the record directory, the listing's formatting and how jj is
// asked.
//
// A `val` cannot see a transition. The transition claims the legs rest on are therefore
// carried structurally here, and the one transition claim a state predicate makes vacuous is
// witnessed by a run: opening is the only action that adds a record and it does so at fresh
// handles only; landing is the only action that rewrites a record, the leg guards it on
// `Open` and on the node-wide test; every refusal, every reading and the guard's step set
// `records' = records`, so a refusal leaves the registry exactly as it stood; and no action
// removes a record. `step` names those leg actions and no other, so no other move exists. The
// state-level shadows of the landing claims are `LandedRecordsAreFresh` and
// `NoStaleSiblingLands`.
//
// The witness is a test, not an invariant. `quint test specs/drafts/check.qnt
// --main=draft_check` drives one fresh opening and one landing and asserts the final state:
// the addressed handle's record is `Landed` and keeps the handle, node, edit, workspace and
// base it was filed with. A `landed` that left the record `Open` would satisfy every
// invariant vacuously - no state would hold a landed record - but this run would fail, so
// the run and the invariants together cover the landing's effect and the safety facts it
// implies. The nine invariants and the `quint verify` command below are unchanged.
module draft_check {
  import draft_state.* from "./state"
  import draft_open.* from "./open"
  import draft_land.land from "./land"
  import draft_land.refuseStale from "./land"
  import draft_land.refuseUnknown from "./land"
  import draft_land.stale from "./land"
  import draft_land.staleHandlesOn from "./land"
  import draft_drift.drift from "./drift"
  import draft_drift.reconcile from "./drift"
  import draft_drift.listing from "./drift"
  import draft_drift.reconcileRead from "./drift"
  import draft_drift.listingRead from "./drift"
  import draft_guard.* from "./guard"

  // The harness's pool of names, handles, workspaces and the one edit a call carries. The
  // legs do not mint a handle (tvtnokzu through rnwlstks) or derive a place from it
  // (lpkssxpp), so the harness supplies them; a small fixed pool is also what makes the
  // registry's size bound checkable.
  pure val NodeIds: Set[str] = Set("n1", "n2")
  pure val HandleIds: Set[str] = Set("h1", "h2")
  pure val HarnessEdit: str = "edit"

  // The places a draft's workspace can be given, by name; the legs carry the workspace as a
  // name and a path (kwqnrzoz) and the harness supplies both.
  pure val PlaceIds: Set[str] = Set("w1", "w2")
  pure def placeOf(id: str): Workspace =
    if (id == "w1") { name: "w1", path: "drafts/w1" }
    else if (id == "w2") { name: "w2", path: "drafts/w2" }
    else { name: "w3", path: "drafts/w3" }
  pure val Workspaces: Set[Workspace] = Set(placeOf("w1"), placeOf("w2"))

  // qrqsxyzz (through tvtnokzu): the base is one entry per owned path, each the identity of
  // that path's content; the identity's carrier is a value the state module holds, and its
  // encoding is not this module's to choose. `OwnedNow` is the owned set every landing reads
  // as it stands at the landing; `BaseCurrent` is the base that matches it; `BaseStale`
  // differs from it at the shared path, which `differing` reports as one way a base and the
  // owned set can disagree (zsvukpls, ptlmptoy).
  pure val OwnedNow: str -> Identity = Map("src/main.rs" -> "id-current")
  pure val BaseCurrent: str -> Identity = Map("src/main.rs" -> "id-current")
  pure val BaseStale: str -> Identity = Map("src/main.rs" -> "id-stale")

  // The harness files each handle under its own base: `h1` under the base that matches the
  // owned set, `h2` under one that differs at the shared path. The base is fixed per handle
  // so that a record's staleness is determined by which record it is, not by a choice made
  // at filing; the legs take the base as an input either way (qrqsxyzz through tvtnokzu).
  pure def baseFor(handle: str): str -> Identity =
    if (handle == "h1") BaseCurrent else BaseStale

  // yozqornv (through svmuxsmq): existence is jj's answer, not the registry's, and it is an
  // input to each reading. Three answers reach both sides of it and the per-record flag: no
  // workspace present, every workspace present, and every workspace but the first.
  pure def NoWorkspaces(w: Workspace): bool = false
  pure def AllWorkspaces(w: Workspace): bool = true
  pure def AllButFirstWorkspace(w: Workspace): bool = w.path != "drafts/w1"

  // The harness discipline described above: the node has no landed record yet. It reads
  // only the registry, and it is a precondition of this harness's opening calls, not of the
  // opening leg - `open` itself permits a call on a node that already holds a landed draft.
  def unlanded(node: str): bool =
    records.keys().forall(h =>
      (records.get(h).node != node) or (records.get(h).state == Open))

  // rnwlstks: one opening call naming one node, filed before that node's first landing. The
  // nondet picks are the call's arguments - the name, a handle, a place and a base. Nothing
  // here adds a condition the leg does not have: the leg's own guards refuse the call when
  // the handle is not fresh or the place is taken, and the base is one the call may file.
  action openOne = {
    nondet node = oneOf(NodeIds);
    nondet handle = oneOf(HandleIds);
    nondet placeId = oneOf(PlaceIds);
    all {
      unlanded(node),
      open(
        List(node), HarnessEdit, List(handle), List(placeOf(placeId)),
        List(baseFor(handle))),
    }
  }

  // rnwlstks + vnwmpwrx: one opening call naming two nodes at once - two positions, two
  // records, one edit, one base each - with the two handles and two places the leg's guards
  // require to be distinct. The names may be equal: two drafts of one node are two records,
  // and their bases may differ, so one record on a node can be fresh while its sibling is
  // stale.
  action openTwo = {
    nondet first = oneOf(NodeIds);
    nondet second = oneOf(NodeIds);
    nondet handle = oneOf(HandleIds);
    nondet twin = oneOf(HandleIds);
    nondet placeId = oneOf(PlaceIds);
    nondet elsewhereId = oneOf(PlaceIds);
    all {
      unlanded(first),
      unlanded(second),
      open(
        List(first, second), HarnessEdit, List(handle, twin),
        List(placeOf(placeId), placeOf(elsewhereId)),
        List(baseFor(handle), baseFor(twin))),
    }
  }

  // tvsqxrmo: one landing on one handle, composed of the leg's own three landing actions -
  // `land`, `refuseStale` and `refuseUnknown` - exactly as the leg's `step` composes them.
  // The owned set the landing compares against is `OwnedNow`, the set the current base
  // matches: a record filed under any other base of the pool is stale at this landing, and
  // a node holding one of them refuses the landing of a fresh sibling.
  action landing = {
    nondet handle = oneOf(HandleIds);
    any {
      land(handle, OwnedNow),
      refuseStale(handle, OwnedNow),
      refuseUnknown(handle),
    }
  }

  // kzltplnz: remove's guarded refusal on one node; it is enabled exactly when the guard
  // holds, and it leaves the registry as it stands.
  action guardRefusal = {
    nondet node = oneOf(NodeIds);
    refusedRemove(node)
  }

  // rnwlstks + tvsqxrmo + svmuxsmq + kzltplnz: the five legs' own actions, and nothing else.
  // Opening once for one name, once for two names and once as the refused empty call; the
  // landing's three actions; both reads against all three of jj's answers; and remove's
  // guarded refusal. The two legs that declare a `step` are recomposed from exactly that
  // `step`'s members, so the composition adds no operation and no state fact.
  action step = any {
    openOne,
    openTwo,
    openNone,
    landing,
    any { reconcileRead(NoWorkspaces), listingRead(NoWorkspaces) },
    any { reconcileRead(AllWorkspaces), listingRead(AllWorkspaces) },
    any { reconcileRead(AllButFirstWorkspace), listingRead(AllButFirstWorkspace) },
    guardRefusal,
  }

  // kwqnrzoz + yuomrusm (through tvtnokzu): every record the composed step leaves behind
  // sits under the handle it carries and holds one of the two states the record has. A
  // record filed under any other key, or in a third state, breaks this.
  val RegistryShaped =
    records.keys().forall(h =>
      records.get(h).handle == h
        and (records.get(h).state == Open or records.get(h).state == Landed))

  // vnwmpwrx + lpkssxpp (through rnwlstks): one draft, one place. No two records the step
  // leaves behind name the same workspace path - neither two positions of one call nor a
  // new record and a filed one - so a place holds at most one draft.
  val OneDraftPerPlace =
    records.keys().forall(a =>
      records.keys().forall(b =>
        (a != b) implies (records.get(a).workspace.path != records.get(b).workspace.path)))

  // vnwmpwrx + yuomrusm + qrqsxyzz (through rnwlstks): the record the opening files is the
  // one its call names - the node at that position, the call's one edit, one of the
  // workspaces the call supplied and one of the bases the call could file. A record carrying
  // a value no call gave (a second edit, a node outside the pool, a forged base) breaks this.
  val RecordsAsFiled =
    records.keys().forall(h =>
      NodeIds.contains(records.get(h).node)
        and records.get(h).edit == HarnessEdit
        and Workspaces.contains(records.get(h).workspace)
        and records.get(h).base == baseFor(records.get(h).handle))

  // yozqornv + qspsyqlp (through svmuxsmq): the listing reports every record the registry
  // holds and no other; the reconcile reading reports exactly the records that still assert
  // a draft while jj has no workspace they name - all open records when no workspace exists,
  // none when every workspace does, and exactly the open records whose place is the first
  // workspace when only that one is gone.
  val ReadingIsComplete =
    listing(NoWorkspaces).keys() == records.keys()
      and listing(AllWorkspaces).keys() == records.keys()
      and listing(AllButFirstWorkspace).keys() == records.keys()
      and reconcile(NoWorkspaces).keys()
        == records.keys().filter(h => records.get(h).state == Open)
      and reconcile(AllWorkspaces).keys().size() == 0
      and reconcile(AllButFirstWorkspace).keys()
        == records.keys().filter(h =>
          records.get(h).state == Open
            and records.get(h).workspace.path == "drafts/w1")

  // yozqornv + vkkwusty (through svmuxsmq): the drift flag is asked per record and the
  // record's state alone decides whether an absent workspace counts. An open record drifts
  // exactly when jj lacks the workspace it names; a landed record never drifts, whether its
  // workspace is gone (the normal end of a draft) or still present.
  val DriftIsByState =
    records.keys().forall(h =>
      (drift(records.get(h), NoWorkspaces)
        == (records.get(h).state == Open))
        and not(drift(records.get(h), AllWorkspaces))
        and (drift(records.get(h), AllButFirstWorkspace)
          == (records.get(h).state == Open
            and records.get(h).workspace.path == "drafts/w1")))

  // mvxkmrqv + yuomrusm (through kzltplnz): only open records block removal. Every open
  // record's node is under the guard, and a landed record is not among the drafts the guard
  // names for the node it names - so a landed record, or a record naming another node,
  // leaves removal free.
  val OnlyOpenBlocks =
    records.keys().forall(h =>
      (records.get(h).state == Open implies guard(records.get(h).node))
        and (records.get(h).state == Landed implies
          not(blockingDrafts(records.get(h).node).contains(h))))

  // kwqnrzoz (through tvtnokzu, rnwlstks): a handle keys one record and opening files only
  // at fresh handles, so the composed step never holds more records than the harness's
  // handle pool - the count grows by opening alone and never past that bound.
  val BoundedByHandles = records.keys().size() <= HandleIds.size()

  // zsvukpls + ptlmptoy (through tvsqxrmo): the addressed record's own freshness is
  // necessary. A landed record keeps the base it was filed with, and `land` is enabled only
  // when the node-wide test finds no stale open record on the node - which includes the
  // addressed record itself - so every landed record's base matches the owned set, and a
  // stale record is never landed. This is the state shadow of the refusal's effect on a
  // stale record: it stays open, at its handle, with its base.
  val LandedRecordsAreFresh =
    records.keys().forall(h =>
      (records.get(h).state == Landed) implies not(stale(records.get(h).base, OwnedNow)))

  // ptlmptoy + uvrltuvo (through tvsqxrmo): the stale test is node-wide, and a landing is
  // never taken while any open record naming the addressed record's node is stale. At every
  // reachable state, a node that holds a landed record holds no stale open record: with the
  // harness filing a node's drafts before its first landing, a landed record beside a stale
  // open sibling on one node can arise only from a landing taken while that sibling was
  // stale. The addressed record's own freshness is necessary but not sufficient, so a fresh
  // record is refused when its node holds a stale one.
  val NoStaleSiblingLands =
    records.keys().forall(h =>
      (records.get(h).state == Landed) implies
        staleHandlesOn(records.get(h).node, records, OwnedNow).size() == 0)

  // The landing's effect, witnessed by a run rather than an invariant: `quint test
  // specs/drafts/check.qnt --main=draft_check` checks that one fresh `open` followed by one
  // `land` leaves the addressed record `Landed`, at its handle, with the node, edit,
  // workspace and base it was filed with, and no other record beside it. The invariants
  // cannot state this: they range over reachable states, so a `landed` that returned the
  // record `Open` would make `LandedRecordsAreFresh` and `NoStaleSiblingLands` vacuous while
  // leaving every invariant green. This run is the discriminating check for the transition.
  run landingTurnsLandedTest =
    init
      .then(open(
        List("n1"), HarnessEdit, List("h1"), List(placeOf("w1")),
        List(BaseCurrent)))
      .then(land("h1", OwnedNow))
      .then(all {
        assert(records.keys() == Set("h1")),
        assert(records.get("h1").state == Landed),
        assert(records.get("h1").handle == "h1"),
        assert(records.get("h1").node == "n1"),
        assert(records.get("h1").edit == HarnessEdit),
        assert(records.get("h1").workspace == placeOf("w1")),
        assert(records.get("h1").base == BaseCurrent),
        // The assertions above read the state the landing produced; this keeps the final
        // step an action over the one variable the legs step, without changing it.
        records' = records,
      })
}
