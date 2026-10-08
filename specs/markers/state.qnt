// Marker state: every bracketed operation's kind, the change it targeted, its kept
// entries, and the reverts recorded against it while it is undone, plus the position:
// how many markers are applied, with the markers from it onward the undone tail.
module marker_state {
  // G-operation: an operation is a creation, a recording or a removal.
  type Kind = Create | Record | Remove

  // G-marker + S5-ids-only: the operation's kind, the change it targeted, the entry
  // ids it kept and the reverts recorded against it — nothing else.
  type Marker = { kind: Kind, target: int, entries: List[int], reverts: List[int] }

  var markers: List[Marker]
  var pos: int

  action init = all {
    markers' = [],
    pos' = 0
  }
}
