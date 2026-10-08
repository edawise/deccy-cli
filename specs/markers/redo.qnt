// T-redo: revert the reverts recorded against the marker at the position and step the
// position one forward; a position at the end of the list refuses. There is no newer
// case: a new operation discards the undone tail before it appends (kpptprql).
module marker_redo {
  import marker_state.* from "./state"

  pure def cleared(m: Marker): Marker =
    { kind: m.kind, target: m.target, entries: m.entries, reverts: [] }

  // qtysmzxy + kttmwrsv: revert the recorded reverts of markers[pos], clear them, and
  // step the position forward one; a repeat at the end of the list refuses.
  action redo = all {
    pos < markers.length(),
    markers' = markers.replaceAt(pos, cleared(markers[pos])),
    pos' = pos + 1
  }

  action redoEmpty = all {
    pos == markers.length(),
    markers' = markers,
    pos' = pos
  }
}
