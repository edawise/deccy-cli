// T-scope: only `bracket` writes markers. Bracketing discards the undone tail at the
// position, appends the operation's kind, the change it targeted and its kept entries
// in run order, and advances the position to the new length; undo and redo have no
// capture path.
module marker_scope {
  import marker_state.* from "./state"

  // otmxqplr + okqmlwmp + kupzywsq + nptquxlz: the undone tail discarded at the
  // position, the operation's kind, the change it targeted and its kept entries
  // appended in run order, the position advanced.
  // lmwpuqlx + pmxnnvvq: only mutating ops bracket (see rationale).
  action bracket(kind: Kind, target: int, kept: List[int]): bool = all {
    markers' = markers.slice(0, pos).append({ kind: kind, target: target, entries: kept, reverts: [] }),
    pos' = pos + 1
  }
}
