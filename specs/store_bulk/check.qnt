// The set read's invariants (T-bulk-read). The model lives in `read.qnt`; this file composes
// it with the checks the verifier runs.
module store_bulk {
  import store_bulk_read.* from "./read"

  // Every served result equals the single-document read's outcome.
  val ResultsMatchReference =
    ids.forall(id => source.get(id) == Unserved or outcome.get(id) == reference(id))

  // A change with more than one commit is never served from the batch.
  val DivergenceFallsBack =
    ids.forall(id =>
      commitCount(id) > 1 implies source.get(id) != ServedFromBatch)

  // When the mapping or git is unavailable, every id is served from the fallback.
  val FallbackTotal =
    (not(mapping_available) or not(git_available)) implies
      ids.forall(id => source.get(id) != ServedFromBatch)

  // At Done every id has an outcome.
  val Totality =
    phase == Done implies ids.forall(id => source.get(id) != Unserved)

  // The batch phase runs at most once.
  val SingleBatch = batch_runs <= 1
}
