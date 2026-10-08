// Marker lifecycle: whole-operation undo/redo over the jj op log.
// Check assembly: composes the four leg modules into one step and asserts
// the invariants the checker runs. Scope: only `bracket` touches markers;
// undo/redo have no capture path. The position names how many markers are
// applied: markers before it are applied, markers from it onward are the
// undone tail.
module markers {
  import marker_state.* from "./state"
  import marker_scope.* from "./scope"
  import marker_undo.* from "./undo"
  import marker_redo.* from "./redo"

  action step = any {
    nondet first = oneOf(Set(1, 2, 3)); nondet second = oneOf(Set(1, 2, 3));
    nondet kind = oneOf(Set(Create, Record, Remove)); nondet target = oneOf(Set(1, 2, 3));
    bracket(kind, target, List(first, second)),
    undo,
    undoEmpty,
    redo,
    redoEmpty
  }

  val TypeOK = (pos >= 0) and (pos <= markers.length())

  // S5-ids-only + D8a: a creation's undo records one crossing; a recording's or
  // removal's records one per kept entry.
  val RevertsShaped =
    markers.foldl(true, (ok, m) =>
      ok and ((m.reverts.length() == 0)
        or (m.kind == Create and m.reverts.length() == 1)
        or (m.kind != Create and m.reverts.length() == m.entries.length())))

  // D-linear (kpptprql): every applied marker carries no reverts and every
  // marker in the undone tail carries them.
  val TailUndone =
    markers.indices().forall(i =>
      (i < pos and markers[i].reverts.length() == 0)
      or (i >= pos and markers[i].reverts.length() > 0))
}
