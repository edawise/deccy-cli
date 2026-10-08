// Marker lifecycle: whole-operation undo/redo over the jj op log.
// Check assembly: composes the four leg modules into one step and asserts
// the invariants the checker runs. Scope: only `bracket` touches markers;
// undo/redo have no capture path. U steps by one and B bounds its window:
// a pair appended since the bound makes redo refuse as newer.
module markers {
  import marker_state.* from "./state"
  import marker_scope.* from "./scope"
  import marker_undo.* from "./undo"
  import marker_redo.* from "./redo"

  action step = any {
    nondet pre = oneOf(Set(0, 1, 2)); nondet post = oneOf(Set(0, 1, 2)); bracket(pre, post),
    undo,
    undoEmpty,
    redo,
    redoNewer,
    redoEmpty
  }

  val TypeOK = (u >= 0) and (u <= b) and (b <= markers.length()) and (at >= 0)

  val AtKnown =
    (at == 0) or markers.foldl(false, (found, r) => found or (r.pre == at) or (r.post == at))

  val Window = (u == 0) or (markers.length() == b)
}
