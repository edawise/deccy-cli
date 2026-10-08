// T-undo: restore pre of the most recent applied pair and step U up; nothing
// applied refuses. The bound is set here for redo to read.
module marker_undo {
  import marker_state.* from "./state"

  pure def nextB = if (u == 0) markers.length() else b

  // qkqkuzxw + kttmwrsv: restore pre of the most recent applied pair and
  // step U up; the bound is what Redo reads its window from.
  action undo = all {
    nextB - u > 0,
    at' = markers[nextB - u - 1].pre,
    u' = u + 1,
    b' = nextB,
    markers' = markers
  }

  action undoEmpty = all {
    nextB == u,
    markers' = markers,
    u' = u,
    b' = b,
    at' = at
  }
}
