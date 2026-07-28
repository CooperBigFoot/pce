# ADR-0005: Scheduling is a computation, not an orchestrator judgement

## Status

Accepted

## Context

`SKILL.md:255` reads: "A readiness decision begins with fresh JSON status and a verbatim snapshot.
A step is ready when dependencies are merged. Parallelize ready steps only with disjoint
`files_touched`." The binary hands over the snapshot and the orchestrator holds the graph in
context and decides which node to start.

That fold has no judgement in it. Its inputs are the graph's ordering edges and the per-node merge
status `pce status` already derives, and its output is the set of nodes whose dependencies have all
merged and which have not been dispatched. It is the same shape as everything the status verb
already computes.

Two facts make leaving it in context unsafe. Merge status is deliberately three-valued — merged,
not merged, or inconclusive when git and `gh` disagree or one is unreachable — and the placement
rule forbids collapsing inconclusive into either side; an actor folding this by hand thirty
dispatches deep collapses it, either stalling or dispatching onto an unmerged base. And this is
the class the Program's founding hypothesis says prose fails at: an invariant the orchestrator must
apply to itself under context pressure. The #38 run is the evidence — twelve edges over thirteen
steps, zero parallel dispatches, against a repository contract that explicitly permitted
parallelism.

Retiring `files_touched` from the graph sharpens the stakes. It was the only graph field any rule
consumed. If ordering edges are likewise read only by the orchestrator's own judgement, the
planning descent replaces one unenforced prose rule with another.

## Decision

The `pce` binary computes the dispatchable set. The orchestrator asks which nodes may start now and
dispatches them, and `SKILL.md` stops carrying the readiness rule.

The graph does not become a fourth authority. Each `planning-artifact-approved` record already
carries the artifact's path and SHA-256, so the computation reads the artifact the event log
identifies and the three authorities stay as ADR 0002 fixed them.

A repository's version-bump policy is an input to this computation, supplied per invocation from the
live orientation result that `SKILL.md:179` already forbids writing anywhere. It is never an
ordering edge. A non-`NONE` policy narrows the returned set within that repository; the planner
never learns the word "version", so a critic can refute any edge knowing nothing but the code.

## Consequences

The descent's product gains a consumer that cannot be forgotten, which is the difference between
this vision mechanizing something and it rewriting a paragraph.

The inconclusive case must now be answered explicitly rather than tacitly, because a verb cannot
decline to return it.

Most of this cannot be verified by the run that delivers it. `SKILL.md` loads once per invocation,
so the scheduling change is inert mid-run, while the binary is invoked per dispatch, so the verb
itself applies to later dispatches in the same run. Acceptance is therefore mechanical — the
computation is correct in isolation — and the behavioural claim, that a subsequent run dispatches
nodes concurrently, is checked by a human on the next round.

Reversing this means returning the fold to context, and it inherits the collapse: no orchestrator
holding thirty dispatches of history reliably keeps a three-valued merge status three-valued.
