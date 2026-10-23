// T-bulk-read: the set read's result semantics.
//
// The model starts from ids supplied by the node set, maps each change id to the commit(s)
// it names, batches the owning commits' documents, and falls back per unservable id or for
// the whole set when the mapping or git is unavailable. It fixes the result invariants; it
// does not model the wire format, the path spelling, or the Rust code.
module store_bulk_read {
  type Id = int

  // The ids the node set supplies, and the single-document read's outcome for each.
  // Id 2 has no document; id 3 names two commits (divergent).
  pure val ids = Set(1, 2, 3)
  pure val broken = Set()
  pure def commitCount(id: Id): int = if (id == 3) 2 else 1

  // The single-document read's outcome, as an opaque token: a document, absence, or failure.
  pure def reference(id: Id): str =
    if (broken.contains(id)) "failed"
    else if (id == 2) "absent"
    else "document"

  // Where a served outcome came from, or that the id is not served yet.
  type Source = Unserved | ServedFromBatch | ServedFromFallback

  type Phase = Mapping | BatchPhase | FallbackPhase | Done

  var mapping_available: bool
  var git_available: bool
  var phase: Phase
  var outcome: Id -> str
  var source: Id -> Source
  var pending: Set[Id]
  var batch_runs: int

  action init = {
    nondet has_mapping = oneOf(Bool);
    nondet has_git = oneOf(Bool);
    all {
      mapping_available' = has_mapping,
      git_available' = has_git,
      phase' = Mapping,
      outcome' = ids.mapBy(id => "unserved"),
      source' = ids.mapBy(id => Unserved),
      pending' = Set(),
      batch_runs' = 0
    }
  }

  // With no mapping or no git, the whole set is served by the fallback.
  action startUnavailable = all {
    phase == Mapping,
    not(mapping_available) or not(git_available),
    phase' = FallbackPhase,
    pending' = ids,
    mapping_available' = mapping_available,
    git_available' = git_available,
    outcome' = outcome,
    source' = source,
    batch_runs' = batch_runs
  }

  action startBatch = all {
    phase == Mapping,
    mapping_available,
    git_available,
    phase' = BatchPhase,
    pending' = Set(),
    mapping_available' = mapping_available,
    git_available' = git_available,
    outcome' = outcome,
    source' = source,
    batch_runs' = batch_runs
  }

  // A divergent change names more than one commit and is never served from the batch;
  // a missing or failed document is not batchable either.
  pure def batchable(id: Id): bool =
    not(broken.contains(id)) and id != 2 and commitCount(id) == 1

  // One batch crossing serves every batchable id; the rest wait for the fallback.
  action batch = all {
    phase == BatchPhase,
    phase' = FallbackPhase,
    batch_runs' = batch_runs + 1,
    outcome' = ids.mapBy(id => reference(id)),
    source' = ids.mapBy(id =>
      if (batchable(id)) ServedFromBatch else Unserved),
    pending' = ids.filter(id => not(batchable(id))),
    mapping_available' = mapping_available,
    git_available' = git_available
  }

  // A per-id fallback serves the single-document read's outcome.
  action fallback(id) = all {
    phase == FallbackPhase,
    pending.contains(id),
    phase' = FallbackPhase,
    pending' = pending.exclude(Set(id)),
    outcome' = outcome.set(id, reference(id)),
    source' = source.set(id, ServedFromFallback),
    mapping_available' = mapping_available,
    git_available' = git_available,
    batch_runs' = batch_runs
  }

  action done = all {
    phase == FallbackPhase,
    pending == Set(),
    phase' = Done,
    outcome' = outcome,
    source' = source,
    pending' = pending,
    mapping_available' = mapping_available,
    git_available' = git_available,
    batch_runs' = batch_runs
  }

  // Once Done the state stutters: the model stays enabled after the last outcome.
  action stay = all {
    phase == Done,
    mapping_available' = mapping_available,
    git_available' = git_available,
    phase' = phase,
    outcome' = outcome,
    source' = source,
    pending' = pending,
    batch_runs' = batch_runs
  }

  action step = any {
    startUnavailable,
    startBatch,
    batch,
    fallback(1),
    fallback(2),
    fallback(3),
    done,
    stay
  }
}
