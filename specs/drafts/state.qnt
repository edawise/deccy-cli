// T-draft-state: the draft registry's records, the states they take and the
// references they name, and the content-identity base each carries. One record per
// draft, keyed by the handle it is filed under: one target node, one edit, the
// workspace that holds the draft by name and path, the record's state and the base
// the draft was opened against (kwqnrzoz, yuomrusm, qrqsxyzz).
//
// The record's form is here; the document that carries it is not. Where the registry
// is stored and that it is untracked (nolspvpw), that the record is one JSON document
// at `.deccy/work/<handle>.json` found by scanning the directory (rqtowtqn), the
// opening action and the landing are outside this module.
module draft_state {
  // yuomrusm: a record is `open` for as long as its draft has not landed and
  // `landed` from that landing onward. Two named values, and no third.
  type State = Open | Landed

  // kwqnrzoz: the workspace that holds the draft, by name and path - the workspace
  // is one reference among the three, not two further top-level fields.
  type Workspace = { name: str, path: str }

  // qrqsxyzz: the identity of the content a path holds, determined by that content
  // and by nothing else - equal content gives an equal entry, different content a
  // different one. It is not a change id, an edit, a workspace, a path or a copy of
  // the content.
  //
  // The carrier is `str` so the model can hold, compare and supply identity values
  // and so the module can actually be exercised (flattened and model-checked). The
  // carrier is not the encoding. What an identity *is* - the digest rule that
  // derives it from content - is not this module's decision: a landed decision
  // elsewhere fixes the encoding. Here no algorithm, length, textual form or
  // validation is claimed; `str` is only an interpretable value the base can hold.
  type Identity = str

  // kwqnrzoz + yuomrusm + qrqsxyzz: exactly the fields the record document fixes
  // (wrvsxwmq) - the handle the record is filed under, its state, one target node,
  // one edit, the workspace by name and path, and the base. Nothing else is a
  // record's field: no version, no path or filename, no tracking flag (rqtowtqn,
  // nolspvpw).
  type Record = {
    handle: str,
    state: State,
    node: str,
    edit: str,
    workspace: Workspace,
    // One entry per path the target node's owned set holds when the record is
    // filed, keyed by the repo-relative path, each the identity of that path's
    // content (qrqsxyzz).
    base: str -> Identity,
  }

  // kwqnrzoz: the registry is keyed by the generated draft handle - not a node id,
  // not a change id, not derived from either - so a handle picks out exactly one
  // record and one record is filed per handle.
  var records: str -> Record

  // No draft has been filed yet: the registry begins with no records.
  action init = records' = Map()

  // yuomrusm + qrqsxyzz: a record begins `open`, filed under the handle that keys
  // it and carrying the base it was opened against. This is the record's shape at
  // the moment it exists, not the opening action: minting the handle, reading the
  // owned content and writing the document are outside this module.
  pure def filed(
    handle: str,
    node: str,
    edit: str,
    workspace: Workspace,
    base: str -> Identity,
  ): Record =
    {
      handle: handle,
      state: Open,
      node: node,
      edit: edit,
      workspace: workspace,
      base: base,
    }

  // yuomrusm: the record's only state move is `open` to `landed`, at its draft's
  // landing. There is no move into `open` and none out of `landed`, so a landing is
  // never reversed and a later draft arrives as its own record rather than by
  // reopening this one.
  pure def mayMove(before: State, after: State): bool =
    (before == Open) and (after == Landed)

  // yuomrusm: landing moves the state and nothing else. The record is kept at its
  // handle - its references and its base stay as filed - and no record is removed,
  // so the registry only ever gains records and rewrites one.
  pure def landed(r: Record): Record =
    {
      handle: r.handle,
      state: Landed,
      node: r.node,
      edit: r.edit,
      workspace: r.workspace,
      base: r.base,
    }

  // kwqnrzoz: the handle is the key the record is filed under, so a record's handle
  // field is the key that holds it - one record per handle.
  val IdentifiedByHandle =
    records.keys().forall(h => records.get(h).handle == h)
}
