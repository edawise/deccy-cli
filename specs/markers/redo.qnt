// T-redo: restore post of the next undone pair and step U down; nothing undone
// refuses, and a pair appended since the bound refuses as newer once a bracket
// has superseded the undo run (u = 0). Len(markers) = b is the no-newer check
// since markers only grow.
module marker_redo {
  import marker_state.* from "./state"

  // qtysmzxy + kttmwrsv: restore post of the next undone pair, stepping U
  // down; a repeat at nothing-undone refuses. Len(markers) = b means no
  // newer, since markers only grows.
  action redo = all {
    markers.length() == b,
    u != 0,
    at' = markers[b - u].post,
    u' = u - 1,
    b' = b,
    markers' = markers
  }

  // zqrvtyrv + ukulkqqw: newer exists, the superseded case: a pair appended since
  // the bound after a bracket closed the undo run (u = 0).
  action redoNewer = all {
    b != 0,
    u == 0,
    markers.length() > b,
    markers' = markers,
    u' = u,
    b' = b,
    at' = at
  }

  action redoEmpty = all {
    u == 0,
    markers' = markers,
    u' = u,
    b' = b,
    at' = at
  }
}
