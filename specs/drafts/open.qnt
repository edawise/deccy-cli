// T-draft-open: one opening call. The call names a list of nodes and the edit it was
// invoked with, and files, for each name in that list, one record `open` under a handle
// of its own, in a workspace of its own, against the base read at the moment of opening;
// a call naming no node is refused at the argument surface, before any of it exists.
//
// The record's form, its states and its references are the registry's
// (`draft_state`, kwqnrzoz + yuomrusm + qrqsxyzz) - this module steps them and states
// nothing of its own about a record. Where the workspace lives and how its path is
// derived (lpkssxpp), the record's document and where it is found (nolspvpw,
// rqtowtqn), minting a handle, the landing and the stale comparison are outside it: the
// opening writes records and no graph.
module draft_open {
  import draft_state.* from "./state"

  // vnwmpwrx + szvotxpq: the opening action is the call that names at least one node.
  // `nodes` is the surface's list read as namings - one draft per naming, so a node
  // named twice is two drafts and the count is the list's length, not the distinct
  // names. `edit` is the one edit the call names (the surface's optional edit position),
  // carried by every record this call files. For each name, the handle the record is
  // filed under, the workspace that holds the draft and the base the draft is opened
  // against are the values this call fixes and this module does not derive: nothing here
  // mints a handle, places a workspace or reads a content identity.
  action open(
    nodes: List[str],
    edit: str,
    handles: List[str],
    workspaces: List[Workspace],
    bases: List[str -> Identity],
  ): bool = all {
    // xzrkrmmr: the surface refuses a call naming no node, so such a call is not this
    // action; what it does is `openNone` below.
    nodes.length() > 0,
    // vnwmpwrx: one record for each name in the list - none skipped (the record lists
    // are as long as the name list) and none written twice (one position, one record).
    handles.length() == nodes.length(),
    workspaces.length() == nodes.length(),
    bases.length() == nodes.length(),
    // kwqnrzoz + rqtowtqn: one record per handle. A handle keys exactly one record, so
    // every handle this call files under is fresh - no filed record is overwritten - and
    // no two of the call's records share one.
    handles.indices().forall(i => not(records.keys().contains(handles[i]))),
    handles.indices().forall(i =>
      handles.indices().forall(j => (i != j) implies (handles[i] != handles[j]))),
    // vnwmpwrx + lpkssxpp: each draft is in a workspace of its own, and the record
    // carries it through the `workspace` reference (kwqnrzoz). The place a draft's
    // workspace is created at is fixed by its handle, one per record, so no two drafts
    // hold the same path - neither within this call nor against a draft already filed.
    // The workspace's `name` is fixed by no prerequisite to be unique, so it is left
    // free: the distinct thing is the place, not its spelling.
    workspaces.indices().forall(i =>
      workspaces.indices().forall(j =>
        (i != j) implies (workspaces[i].path != workspaces[j].path))),
    workspaces.indices().forall(i =>
      records.keys().forall(k => records.get(k).workspace.path != workspaces[i].path)),
    // vnwmpwrx + yuomrusm + qrqsxyzz: every record filed by this call is the registry's
    // own record - made by its constructor, in its first state `open`, keyed by its
    // handle, naming the node it was opened for, the call's edit, its own workspace and
    // the base it was opened against. The base is the identity read once, at filing: the
    // model re-reads nothing afterwards, so a draft's base is the one it was opened with.
    // Nothing else in the registry changes, and no variable but the record registry is
    // written - opening writes no node, edit or marker. The fold's order cannot show: the
    // handles are distinct, so every order builds the same map.
    //
    // vnwmpwrx: filing is a key-inserting write, not an assignment at a key that must
    // already exist. `Map::put` inserts the handle (or replaces it, which the freshness
    // guard above forbids), so the action steps from the registry's empty initial map;
    // `Map::set` and `Map::setBy` require the key to be present and would file nothing
    // on a fresh registry.
    records' = nodes.indices().fold(records, (acc, i) =>
      acc.put(handles[i], filed(handles[i], nodes[i], edit, workspaces[i], bases[i]))),
  }

  // xzrkrmmr: a call naming no node is refused at the argument surface, before any
  // workspace or record exists. Nothing in the registry witnesses that the call was
  // empty - the emptiness is the call's, not the state's - so the refused call is the
  // step that writes no record and creates no workspace, available whatever the
  // registry holds because the surface refuses the call whatever the registry holds.
  action openNone = all {
    records' = records,
  }
}
