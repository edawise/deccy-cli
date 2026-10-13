// T-draft-guard: remove's draft guard. Removal is refused exactly when some open
// draft record names the node, and only open records block, so a landed record and a
// record naming another node both leave removal free. The refusal names each blocking
// draft by its handle (mvxkmrqv). The record's form, its two state values and its
// constructor are the state module's and are imported, not restated (tvtnokzu); this
// module steps only the registry variable `records`.
module draft_guard {
  import draft_state.* from "./state"

  // mvxkmrqv + kwqnrzoz: a record names exactly one target node, so "names the node"
  // is equality between the id the caller gave `remove` and the record's single `node`
  // field (tvtnokzu).
  pure def namesNode(r: Record, node: str): bool =
    r.node == node

  // mvxkmrqv + yuomrusm: a record blocks removal while it is `open`; `landed` is the
  // only other state value the state module gives, and a landed record does not block
  // (tvtnokzu).
  pure def blocks(r: Record, node: str): bool =
    (r.state == Open) and namesNode(r, node)

  // mvxkmrqv + owyntkvw: `any` ranges across records, so the guard asks every record,
  // and the refusal names each such draft by the handle that keys it - a set of
  // handles, not a count and not the node id (tvtnokzu).
  def blockingDrafts(node: str): Set[str] =
    records.keys().filter(h => blocks(records.get(h), node))

  // mvxkmrqv + yuomrusm: removal is refused exactly when some open record names the
  // node; when every record either names another node or is `landed`, it is not
  // (tvtnokzu).
  def guard(node: str): bool =
    blockingDrafts(node).size() > 0

  // mvxkmrqv: the guard is a precondition on removal, not a state change. The refused
  // step is enabled exactly when the guard holds and leaves the registry as it was;
  // how `landed` is reached, and the removal's own effect when the guard does not
  // hold, are outside this decision (tvtnokzu).
  action refusedRemove(node: str): bool = all {
    guard(node),
    records' = records,
  }
}
