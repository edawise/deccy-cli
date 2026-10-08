// T-undo: take the marker before the position by the inverse its kind records —
// reverting its entries newest first for a recording or removal, recording the removal
// of the created change for a creation — record the reverts it makes, and move the
// position one back; a position at zero refuses, and a revert that refuses partway
// refuses the whole undo, leaving the state as it began.
module marker_undo {
  import marker_state.* from "./state"

  // The recorded reverts stand for the revert entries' ids — ids the model does not
  // generate, so the value here is a placeholder and only their number is modelled.
  // A creation's undo is one crossing (the created change removed), so it records one
  // revert; a recording's or removal's undo crosses one entry per kept entry (D8a).
  pure def recorded(m: Marker): Marker =
    if (m.kind == Create)
      { kind: m.kind, target: m.target, entries: m.entries, reverts: [0] }
    else
      { kind: m.kind, target: m.target, entries: m.entries, reverts: m.entries }

  // qkqkuzxw + kttmwrsv: take markers[pos-1] by the inverse its kind records, record
  // the reverts it made, and step the position back one (kpptprql).
  action undo = all {
    pos > 0,
    markers' = markers.replaceAt(pos - 1, recorded(markers[pos - 1])),
    pos' = pos - 1
  }

  action undoEmpty = all {
    pos == 0,
    markers' = markers,
    pos' = pos
  }
}
