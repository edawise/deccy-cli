// T-draft-land: one landing. A recording that lands a draft addresses exactly one record
// by the handle it is filed under, compares the base that record carries against the owned
// set as it stands at the landing, refuses the landing when any open record naming the
// addressed record's node is stale - placing nothing and naming every stale handle on that
// node together with the paths that differ for it - and otherwise moves the addressed
// record to `landed`: the record's one state move, inside the one recording the landing
// already is.
//
// The record's form, its two states and its content-identity base are the registry
// model's and are imported, not restated (tvtnokzu). The owned set's content is read from
// jj and arrives as an input, not as registry state (zsvukpls); what a landing places into
// the node's change and the operation boundary that makes placement and the state turn one
// step are outside this module (vmwvnnzv). The record document, its path and
// encoding, and the refusal's wording, separators, ordering, stream and exit status are not
// modelled. This module steps only `records`.
module draft_land {
  import draft_state.* from "./state"

  // zsvukpls: the base is one entry per owned path, each the identity of that path's
  // content, with the identity's encoding left open (tvtnokzu). Whether the two sides agree
  // at one path is equality of the identities the record and the owned set carry - the
  // opaque value's own equality, not an encoding this module chooses.
  pure def differsAt(base: str -> Identity, now: str -> Identity, path: str): bool =
    if (base.keys().contains(path) and now.keys().contains(path))
      base.get(path) != now.get(path)
    else
      base.keys().contains(path) != now.keys().contains(path)

  // ptlmptoy + zsvukpls: the differing paths are the paths on which the base and the owned
  // set disagree in either direction - an entry the base holds and the owned set no longer
  // does, an owned path the base does not carry, and a path both hold under different
  // identities. The owned set is `now` as it stands at the landing, so a path outside it is
  // not among them.
  pure def differing(base: str -> Identity, now: str -> Identity): Set[str] =
    base.keys().union(now.keys()).filter(path => differsAt(base, now, path))

  // ptlmptoy: a record is stale exactly when its base no longer matches the owned set, that
  // is, when the record has at least one differing path.
  pure def stale(base: str -> Identity, now: str -> Identity): bool =
    differing(base, now).size() > 0

  // ptlmptoy + vmwvnnzv (uvrltuvo): the stale test is node-wide. A record is a draft a
  // landing tests while it is open and names the target node; "names the node" is equality
  // on the record's single `node` field (tvtnokzu), so every open record naming that node is
  // tested, not only the one the landing addresses. The test fixes which records it ranges
  // over and no order among them and no cap on how many there are.
  pure def staleHandlesOn(node: str, rs: str -> Record, now: str -> Identity): Set[str] =
    rs.keys().filter(h =>
      (rs.get(h).node == node) and (rs.get(h).state == Open) and stale(rs.get(h).base, now))

  // ptlmptoy + vmwvnnzv (uvrltuvo): a landing is refused when at least one open record
  // naming the target node is stale, even when the record the landing addresses is itself
  // fresh. The addressed record's own freshness is necessary but not sufficient.
  pure def nodeStale(node: str, rs: str -> Record, now: str -> Identity): bool =
    staleHandlesOn(node, rs, now).size() > 0

  // ptlmptoy + uvrltuvo: a refused landing names every stale open record on the addressed
  // record's node by the handle that identifies it, together with the paths that differ for
  // it - one entry per stale handle, not only the addressed one. The order of the handles
  // and of the paths, the shape of the message and its delivery are not fixed here.
  pure def refusal(node: str, rs: str -> Record, now: str -> Identity): str -> Set[str] =
    staleHandlesOn(node, rs, now).mapBy(h => differing(rs.get(h).base, now))

  // tvtnokzu + vmwvnnzv: a landing addresses exactly one record, selected by the handle
  // alone - not by its node, its edit or its workspace, and not by a path a call names. A
  // handle the registry does not hold addresses no record, so a call carrying it is not a
  // landing: it places nothing and leaves every record as it was. Several records may name
  // the same node; each is still picked out only by its own handle.
  action refuseUnknown(handle: str): bool = all {
    not(records.keys().contains(handle)),
    records' = records,
  }

  // ptlmptoy + vmwvnnzv (uvrltuvo): a stale landing is refused: when any open record naming
  // the addressed record's node is stale, the landing places nothing and leaves every record
  // exactly as it stood, so no record's state moves and no base is rewritten - including when
  // the addressed record is itself fresh. The names it gives are `refusal`'s: every stale
  // handle on that node with its differing paths. Only an open record is a draft a landing
  // would land; a landed record is not re-checked or refused again.
  action refuseStale(handle: str, now: str -> Identity): bool = all {
    records.keys().contains(handle),
    records.get(handle).state == Open,
    nodeStale(records.get(handle).node, records, now),
    records' = records,
  }

  // vmwvnnzv + tvtnokzu (yuomrusm): a fresh landing - the node-wide test finds no stale open
  // record naming the addressed record's node - moves exactly that record to `landed` and
  // changes nothing else in the registry. The state module's `landed` keeps the handle, the
  // three references and the base as filed, so the record is retained; every other record,
  // including another record naming the same node, is left untouched. That the placement of
  // the changed owned paths into the node's change happens in this same recording is the
  // operation's, not a second step here (vmwvnnzv).
  action land(handle: str, now: str -> Identity): bool = all {
    records.keys().contains(handle),
    records.get(handle).state == Open,
    not(nodeStale(records.get(handle).node, records, now)),
    records' = records.set(handle, landed(records.get(handle))),
  }

  // vmwvnnzv + ptlmptoy: a landing is exactly one of these steps: it lands the one open
  // record its handle names when no open record on that node is stale, refuses the landing
  // of the addressed open record when one is, or names no record at all. The composition
  // adds no operation and no storage fact - the record registry is the only variable
  // stepped, and the landing's placement is outside it.
  action step(handle: str, now: str -> Identity): bool = any {
    land(handle, now),
    refuseStale(handle, now),
    refuseUnknown(handle),
  }
}
