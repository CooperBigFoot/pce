# PCE as a specification-and-verification layer

Design record, 2026-08-12. Supersedes the step-tier execution model.

## What PCE becomes

PCE stops being an orchestrator. It becomes the layer that says **what the work is**, **what would
prove it**, and **whether the built thing survives attack**. It no longer decides who runs what,
where, or how.

- **PCE owns:** the Program and its tickets, the vision and its executable criteria, the
  work-package graph, the repository contract, the event log, the cost meter, and the gates.
- **herdr owns:** worktrees, spawning, liveness, and the human-visible pane state.
- **prime-agent owns:** how a work package actually gets built.

Tool-specific knowledge may leak freely into the workflow, the verbs, and the skill. It may not
leak into the **event log** or the **acceptance criteria**, because those are records that get
replayed by future code and must outlive the current harness.

## The human boundary

Two recognition moments. Nothing technical, ever.

1. **The vision.** Charted with an LLM as a thinking partner: `/chart-program`, then a grill per
   ticket. This is the part that works and does not change.
2. **The shape of the finished world.** One look at the rendered graph before execution starts —
   not to review it technically, but to flinch at a world that is not the one intended. The
   corpus's most expensive failure (eleven providers' legacy pipelines deleted, correct against
   the vision, approved by two critics) was caught only when a human saw "~6,000 lines deleted"
   in passing. The graph is the last cheap moment to show that picture.

After that, three things and only three things may reach the human:

- spending money,
- an act that cannot be undone or that leaves the machine,
- the work having drifted from the world that was asked for.

Anything else that stops the system is a defect in PCE, to be reported — never a decision handed
to the human. A question the human cannot answer from taste is a defect in the question.

## Where the graph comes from

This is the only planning that survives, and it happens once per plan version.

**1. Authoring.** One dispatched pass reads the vision, its ratified acceptance criteria, and the
source at a named ref, and emits the work-package graph. Each node carries: an id, the
repositories it touches, its acceptance criteria as executable commands, and for each
`depends_on` entry the code fact that makes this node unbuildable until that predecessor merges.
This is the descent obligation from #68, unchanged — the author reads the code, not the prose.

**2. Mechanical check, not review.** The graph is not reviewed for taste. It is checked against a
predicate that terminates:

- every node's acceptance criteria are executable commands, not prose;
- every edge names a code fact, not a preference;
- the graph is acyclic;
- the union of node criteria covers the vision's ratified criteria.

A failure names the offending node and returns for one correction. This replaces the plan-critic
loop, which had no fixed point — observed blocker counts of 4→2→4 and 1→2→1.

**3. Recognition.** The graph renders as an artifact and the human looks at it once. Not to
review it technically — to flinch if it is not the world they asked for. This is the second and
last human touch before execution.

**4. Freeze.** The approved graph becomes plan version 1, immutable and digest-verified.

**Re-authoring.** The recovery ladder's third rung mints a *new plan version* rather than mutating
the graph. That re-enters step 1 for the affected region, re-runs the check, and re-renders. The
human is notified only when the new version changes which of the vision's criteria are covered —
that is checkable, and it is the "drifted from what you asked for" condition. A re-cut that
changes only internal boundaries re-renders without interrupting.

## Execution model

An explicit static DAG of work packages, executed by a scheduler. In the vocabulary of
*From Agent Loops to Structured Graphs* (arXiv:2604.11378): a multi-ready-unit scheduler with an
explicit, deterministic policy. PCE already satisfies most of this — `pce ready` computes the
dispatchable set in the binary rather than in the orchestrator's head, graph approval is
digest-verified, and the event log is append-only. The changes below close the two gaps.

**A work package is the largest unit whose acceptance criteria can be written as executable
commands.** Not a size, a property. Too large and adversarial review degrades into reading a
diff, which catches nothing. Too small and it is a step again.

**A node's interior is fully autonomous.** prime-agent decides how. Control returns at the edge.

**A node is not complete when the builder stops.** It completes when its acceptance criteria
execute and pass, and a gate with different capabilities tries to break the built artifact and
fails. Building and attacking never share permissions.

**Recovery is a bounded ladder:** retry, then local patch, then a new plan version. Each level
must be exhausted before the next. No level requires a human. This replaces
cap → escalate → permanent refusal, and it is the fix for the observed pathology where a capped
node is superseded by a duplicate node with a fresh counter — five deep in one milestone.

**Plan versions are immutable.** A structural change mints a new version rather than mutating the
graph mid-execution. Today's runtime delta stubs mutate it, which is why `pce ready` cannot
classify a milestone whose merged step set exceeds its approved graph.

## What survives

- The Program layer: map, tickets, Fog, `/chart-program`, `/work-ticket`, grill-with-docs,
  `CONTEXT.md`, ADRs.
- The vision and its ratified, executable acceptance criteria.
- The repository contract, measured at base on the default branch.
- The append-only event log as the only durable state.
- The graph, its justified edges, and the computed ready set.
- Adversarial gates against the built artifact.
- Worktree isolation, two-tier merge, human-only tag and release.

## What dies

- `steps.json` and the entire step tier.
- Step plan documents and their critics — measured at 63 of 197 dispatches, a third of a run.
- The blind cold executor and the serialized universe a plan had to carry for it.
- Dispatch route anchors, the closed placeholder vocabulary, the exactly-three-`--env`
  cardinality rule, and the anchored role registry — roughly 100 of 542 lines of `SKILL.md`,
  and the source of the `TMPDIR` strand and much of #167.
- `pce dispatch` spawn and liveness machinery, including the pid-plus-start-identity sidecar.
- Runtime delta stubs as a mechanism for changing the graph.

## What a worker's brief carries

A worker sees more than its own package. The brief carries, in this order:

1. **The vision's goal and its acceptance criteria** — what the whole thing is for.
2. **The whole graph, in summary** — every package's id, title and criteria. Not full detail.
3. **Its own package, in full** — criteria with commands, repositories, dependencies and their kinds.
4. **An explicit boundary statement**: every other package in that graph is someone else's work and
   is out of bounds.

The graph is supplied so the worker knows **what is not its job**. That is the framing, and it
matters: a worker that can see neighbouring packages could otherwise justify doing one because it
is convenient, which is the scope creep that makes a review unit unreviewable. Naming the boundary
converts shared context from a licence into a constraint.

The second reason is coherence. The corpus's most expensive failure — eleven providers' legacy
pipelines deleted, correct against the vision as written, approved by two critics — was work that
was locally right and globally wrong. A worker holding the whole graph can see that its change
would strand a sibling.

**A third return value follows from this.** A worker that reads the vision and the graph may
conclude its own package is mis-specified — that its criteria do not serve the goal, or that a
dependency is missing. That is neither a defect in its work nor a retry: it routes to re-authoring
as plan version `n+1`. Without it, a worker that notices the graph is wrong has no way to say so
and will either build the wrong thing or stall.

## The human surface is an artifact

Every human-facing output is a published artifact, not terminal text. The graph before execution,
the run's current state, a deviation notice, the finished result. This is the concrete answer to
the surfacing problem — a human awake at 3am unable to tell where a run was stuck, because every
surface reaching them was written in role names, verdict fields and file paths.

An artifact is readable by someone who does not know the workflow's vocabulary, which is the
actual requirement.

## Multi-repository work

A node names the repositories it touches. herdr creates one worktree per repository per node, and
the worker receives all of them. The repository contract is per repository and already works this
way.

What is *not* settled: whether a cross-repo node needs a rule beyond running the consumer's own
stated `test` command. The Program map records this as open, with the note that the evidence needs
a deliberately multi-repository run — two consecutive visions touched cross-repo behaviour and
neither produced a single observation because both were single-repo. Treat it as unresolved rather
than designed.

## The work packages

| id | package | depends on |
|---|---|---|
| WP1 | The work-package graph: node contract (id, repositories, executable criteria), edges naming code facts, immutable plan versions, `pce ready` at package altitude | — |
| WP2 | Graph authoring: vision → graph in one pass, the mechanical predicate check, re-authoring on a new plan version | WP1 |
| WP3 | Bounded recovery ladder: retry → local patch → new plan version, each exhausted before the next, no human required | — |
| WP4 | Dispatch leaves the binary: herdr owns worktrees (one per repository per node), spawn, liveness and visible state; the route-anchor doctrine is deleted | — |
| WP5 | The work-package worker: brief composed from a node, `prime-agent -p`, result collection, criteria execution as the completion test | WP1, WP4 |
| WP6 | The gate as node exit: adversarial attack on the built artifact under different capabilities from the builder | WP5 |
| WP7 | The driver: drains the graph — ready, spawn, wait, criteria, gate, merge, append — escalating only on the three human conditions | WP2, WP3, WP6 |
| WP8 | The artifact surface: the graph picture, run state, and deviation notices as published artifacts | WP1 |
| WP9 | Cut over: retire the step tier, and produce the migration document that carries the six live repositories across | WP7 |

Opening ready set is `{WP1, WP3, WP4}` — three packages dispatchable in parallel.

## Acceptance criterion for the whole redesign

**Pick one of the six live repositories, convert its in-flight vision to the new format, and
resume it from where it actually stands. It runs unattended and lands.** Then the remaining five
are converted the same way.

This is stronger than a greenfield vision, because it proves the migration and the mechanism at
the same time, on work that is already half-done and already stuck.

Supporting criteria, all executable:

- No node can become permanently undispatchable.
- A node whose recovery ladder is exhausted parks, stays resumable, and the run continues
  around it.
- The three human conditions are the only paths that stop and notify.

**Backwards compatibility is explicitly not required.** Old event logs need not replay under the
new code. What is required instead is a written migration procedure: for each of the six
repositories, how to express its current position in the new format and continue from there.
That is WP9's deliverable.

## Not in scope

- The knowledge-interface question: PCE has seven overlapping knowledge stores (`CONTEXT.md`,
  ADRs, both halves of the repository contract, event-log key-findings, the Program map,
  `SKILL.md`). Unifying them, and deciding what falsifies each kind, is open design work near
  #105 and is deliberately excluded here.
- The eight legacy prose visions that `pce status` cannot parse. Pre-existing, unchanged by
  this design.

## Sequencing note

The repair branch `repair/orchestration-dead-ends` raises the snapshot schema to v2, and strict
v1 consumers reject v2. Merge it, rebuild, then restart all six live sessions — in that order.
Do not merge while runs are in flight; mid-run schema churn has already discarded correct
reviews once.
