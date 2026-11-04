// T-discard: one discard call over the draft registry.
//
// The call names a list of handles. A call is accepted exactly when every handle it
// names picks out an `open` record in the registry as it then stands; an accepted call
// removes exactly the records it names and nothing else, and a refused call is refused
// whole, leaving the registry exactly as it was. There is no third outcome.
//
// The record's form, its two states, the content-identity base and the registry
// variable `records` are `draft_state`'s (tvtnokzu, `specs/drafts/state.qnt`); this
// module imports them and states nothing of its own about a record, a state value or a
// base. What a discard ends is slwvtspu's (an open draft, ended without landing); which
// handles select and when a call is refused is lnpnkvkv's; the store-side removal and
// retention are yomurusp's. The change abandonment and the workspace forget a real
// discard performs are jj and filesystem effects and are outside this model, and so is
// the verb's wording, stream and exit status.
//
// The helpers are prefixed because an importing module flattens this module's
// definitions next to its own; `discard` is the module's one exported step, and the
// names `named`, `accepted` and `withoutNamed` belong to the check that composes it.
module draft_discard {
  import draft_state.* from "./state"

  // lnpnkvkv: the handles a call names, read as a set. A call is a list, and the
  // handle is the only key that addresses a draft, so a handle named twice names the
  // same record once; the order of the list carries no meaning.
  pure def discardNamed(handles: List[str]): Set[str] =
    handles.indices().map(i => handles.nth(i))

  // lnpnkvkv + slwvtspu: a handle is a subject of a discard exactly when the registry
  // holds a record under it and that record is `open`. A handle the registry does not
  // hold names no record, and a `landed` record is past the end a discard can reach
  // (tvtnokzu, yuomrusm), so neither is a subject.
  pure def discardSubject(recs: str -> Record, handle: str): bool =
    recs.keys().contains(handle) and recs.get(handle).state == Open

  // lnpnkvkv: a call is accepted exactly when every handle it names is a subject. An
  // empty list names no handle, so every handle it names is vacuously a subject and an
  // empty call removes nothing; a list naming one non-subject is refused whole.
  pure def discardAccepted(recs: str -> Record, handles: List[str]): bool =
    handles.indices().forall(i => discardSubject(recs, handles.nth(i)))

  // yomurusp: the registry with every named handle removed, and nothing else - each
  // removed record's document goes, every other record stays at its handle. Nothing is
  // rewritten, renamed or moved, so this rebuilds the map from the same records and
  // never edits one in place.
  pure def discardWithout(recs: str -> Record, handles: List[str]): str -> Record =
    recs.keys()
      .filter(h => not(discardNamed(handles).contains(h)))
      .mapBy(h => recs.get(h))

  // lnpnkvkv + slwvtspu + yomurusp: the one action, total over the handle list it is
  // given. An accepted call steps the registry to exactly the records it did not name;
  // a refused call steps it to itself, so the whole call leaves the registry as it was
  // and no named record is half-removed. The action carries no precondition, so every
  // handle list - empty, open, landed, absent, mixed, repeated - has exactly one
  // outcome, and no other variable is written: a discard writes no node and no change
  // and files no record (lnpnkvkv).
  action discard(handles: List[str]): bool =
    records' =
      if (discardAccepted(records, handles)) discardWithout(records, handles)
      else records
}
