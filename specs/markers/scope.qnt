// T-scope: only `bracket` writes markers. Bracketing appends the pair in
// order, clears U — the bracket supersedes the undone work — and leaves the
// bound standing; undo and redo have no capture path.
module marker_scope {
  import marker_state.* from "./state"

  // otmxqplr + okqmlwmp + kupzywsq + nptquxlz: read-run-read pair appended in
  // order, position advanced to its post-boundary.
  // lmwpuqlx + pmxnnvvq: only mutating ops bracket (see rationale).
  action bracket(pre: int, post: int): bool = all {
    markers' = markers.append({ pre: pre, post: post }),
    u' = 0,
    b' = b,
    at' = post
  }
}
