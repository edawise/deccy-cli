// T-TWO-ROOTS (wxsynqwm): the trunk state root an invocation opens its store
// on, resolved from the invocation workspace's repository entry, or the refusal
// when that entry cannot lead to it. The jj runner's anchor stays the invocation
// workspace throughout, so the store root and the working copy the invocation
// edits are two answers to two questions.
//
// The model is a transition system: a state is one invocation (its workspace,
// its repository entry, the store root resolved from it, and the runner anchor).
// It abstracts the entry's representation: an entry is the workspace's own,
// leads to another working copy, or is missing, unreadable or dangling. It never
// spells a filename, a path shape, the pointer's text, how far the pointer is
// followed, or what may sit between the invocation and the root; those belong to
// the module decision.
module trunk_root {
  // A working copy is represented only by its identity, and a state root by the
  // identity of the working copy whose root it is: equality, and nothing about
  // names, placement or checkout.
  type Workspace = int

  // The invocation workspace's repository entry, abstracted to the cases the
  // prerequisites name. `Leads(root)` already denotes the working copy the entry
  // leads to; the yielded store root is that working copy's root, and how many
  // steps the entry took to reach it stays outside the model.
  type Entry = Own | Leads(Workspace) | Missing | Unreadable | Dangling

  // The cause a refusal names. The prerequisites require the refusal to name the
  // cause it met; its wording, stream and exit status stay outside the model.
  type Cause = MissingEntry | UnreadableEntry | DanglingEntry

  // Three outcome shapes: the invocation workspace's own root, the led working
  // copy's root, and the refusal. A refusal carries no workspace, so it can
  // never be a root; there is no fourth shape.
  type Outcome = Root(Workspace) | Refused(Cause)

  // The workspaces and entries the checker explores. These bound a run; they are
  // not rules about which workspaces or entries exist.
  pure val WORLDS = Set(0, 1, 2)
  pure val ENTRIES = Set(
    Own,
    Leads(0), Leads(1), Leads(2),
    Missing, Unreadable, Dangling
  )

  // The entries available to an invocation: `Leads` always targets another
  // working copy, so the self case is exactly `Own`. This is the "another
  // working copy" the prerequisites name, not a rule about paths or follow
  // distance.
  pure def entriesFor(ws: Workspace): Set[Entry] =
    ENTRIES.filter(e => e != Leads(ws))

  var invocation: Workspace   // the invocation workspace root
  var entry: Entry            // that workspace's repository entry
  var storeRoot: Outcome      // the trunk state root, or the refusal
  var runnerAnchor: Workspace // the jj runner's anchor

  // Resolve the trunk state root from the invocation workspace alone: the only
  // inputs are the workspace and its own entry, with no other starting point and
  // no fallback default.
  pure def resolve(ws: Workspace, e: Entry): Outcome =
    match e {
      | Own => Root(ws)
      | Leads(root) => Root(root)
      | Missing => Refused(MissingEntry)
      | Unreadable => Refused(UnreadableEntry)
      | Dangling => Refused(DanglingEntry)
    }

  // The first layout: a workspace the prerequisites admit, its entry, the store
  // root resolved once, and the runner anchored on that same invocation. `init`
  // establishes the anchor; resolution below never assigns it.
  action init = any {
    nondet ws = oneOf(WORLDS); nondet e = oneOf(entriesFor(ws));
    all {
      invocation' = ws,
      entry' = e,
      storeRoot' = resolve(ws, e),
      runnerAnchor' = ws
    }
  }

  // The next resolved layout for the same invocation: another entry from that
  // workspace's repository-entry cases. The step explores the resolution
  // relation, not a live workspace whose entry mutates; `e != entry` only keeps
  // the transition from stuttering. The runner anchor is framed, never assigned
  // by resolution.
  action step = any {
    nondet e = oneOf(entriesFor(invocation));
    all {
      e != entry,
      invocation' = invocation,
      entry' = e,
      storeRoot' = resolve(invocation, e),
      runnerAnchor' = runnerAnchor
    }
  }

  // Resolve the current layout once without changing the invocation; the anchor
  // is a frame condition, so resolution never assigns it. This is the resolution
  // the runs below execute and assert.
  action resolveCurrent = all {
    invocation' = invocation,
    entry' = entry,
    storeRoot' = resolve(invocation, entry),
    runnerAnchor' = runnerAnchor
  }

  // The jj runner's anchor is the invocation workspace root in every reachable
  // state; resolution never assigns it the store root.
  val AnchorIsInvocation = runnerAnchor == invocation

  // The store root is exactly what resolving the invocation's own entry yields.
  val StoreRootIsResolved = storeRoot == resolve(invocation, entry)

  // The self case: the entry is the invocation workspace's own, so the store
  // root is that workspace's root and the anchor stays the same workspace.
  run selfTest =
    (all { invocation' = 0, entry' = Own, storeRoot' = Root(0), runnerAnchor' = 0 })
    .then(resolveCurrent)
    .expect(and {
      invocation == 0,
      storeRoot == Root(invocation),
      runnerAnchor == invocation
    })

  // The other case: the entry leads to workspace 1, so the store root is that
  // working copy's root, the led root is distinct from the invocation, and the
  // runner anchor is still the invocation workspace, not the store root.
  run leadsElsewhereTest =
    (all { invocation' = 0, entry' = Leads(1), storeRoot' = Root(1), runnerAnchor' = 0 })
    .then(resolveCurrent)
    .expect(and {
      storeRoot == Root(1),
      invocation != 1,
      storeRoot != Root(runnerAnchor),
      runnerAnchor == invocation
    })

  // Each unresolved mode refuses, names its cause, yields no root, and leaves
  // the runner anchor on the invocation workspace.
  run missingTest =
    (all { invocation' = 0, entry' = Missing, storeRoot' = Refused(MissingEntry), runnerAnchor' = 0 })
    .then(resolveCurrent)
    .expect(and {
      storeRoot == Refused(MissingEntry),
      storeRoot != Root(invocation),
      runnerAnchor == invocation
    })

  run unreadableTest =
    (all { invocation' = 0, entry' = Unreadable, storeRoot' = Refused(UnreadableEntry), runnerAnchor' = 0 })
    .then(resolveCurrent)
    .expect(and {
      storeRoot == Refused(UnreadableEntry),
      storeRoot != Root(invocation),
      runnerAnchor == invocation
    })

  run danglingTest =
    (all { invocation' = 0, entry' = Dangling, storeRoot' = Refused(DanglingEntry), runnerAnchor' = 0 })
    .then(resolveCurrent)
    .expect(and {
      storeRoot == Refused(DanglingEntry),
      storeRoot != Root(invocation),
      runnerAnchor == invocation
    })
}
