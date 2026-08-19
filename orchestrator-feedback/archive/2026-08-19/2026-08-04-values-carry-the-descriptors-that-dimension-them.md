# PCE workflow feedback: `pce ready` cannot classify any step graph in a cross-milestone run

- Date: `2026-08-04`
- Orchestrator: `Claude Code (Opus 5), resumed session, ultracode`
- Run: `planning/2026-07-31-values-carry-the-descriptors-that-dimension-them` — cross-repo (palaestra, hdx, orthographos, metis), milestone 8 of 8, resumed at `m8-s4`
- Outcome: `in progress` (this file reports mid-run findings; the run continues)

## Executive summary

One high-severity tool finding, discovered while resuming a run that was 3 of 4 steps into its
final milestone.

**`pce ready` cannot classify any step graph in this run, and never could.** Every milestone's
`steps.json` names cross-milestone step ids in `depends_on`, and the dangling-id validator sees
only one graph at a time, so it rejects each of them. `SKILL.md` names `pce ready` the *sole*
readiness authority at both altitudes, and simultaneously requires step graphs whose edges
routinely cross milestone boundaries. Those two requirements are not jointly satisfiable. The
verb has been silently unusable at step altitude for the entire run — eight milestones, 22 steps
— without ever surfacing as an error, because the default newest-first walk *skips* unparseable
candidates at INFO level rather than failing.

A second, smaller finding: the round-index rule that names review artifacts collides with
artifacts left on disk by a previous session that dispatched through a different mechanism.

## Evidence reviewed

- `planning/2026-07-31-values-carry-the-descriptors-that-dimension-them/events.jsonl` — 419+ records at resume, 124 `key-finding`
- `pce status --file … --vision-dir …` — succeeded at resume; 188 KB snapshot, 170 dispatch records, 12 holds, resume node `m8-s4`
- `pce ready --file … --vision-dir … --graph …/milestone-8/steps.json` — exit 1
- `pce ready --file … --vision-dir …` (default selection) — INFO-level skips for every candidate
- `pce ready --file … --vision-dir … --graph …/milestones.json` — `{"results":[]}`
- The approved graphs for milestones 3, 4, 5, 6, 7, 8 read directly from disk
- `git -C …/metis log --oneline pce/values-carry-the-descriptors-that-dimension-them/milestone-8`
- `pce contract refresh --file … --repo-root …/metis --node m8-s1` — all five gates exit 0

## What worked

### `pce status` as an unambiguous resume authority

- Evidence: the first operation of the resumed session was the status probe, which succeeded and
  returned `resume.node = m8-s4`, `resume.state = candidate`, `cycle_position.state =
  review-dispatched`, and zero open holds. That matched the handover note written by the prior
  session without my having to reconcile them.
- Effect: resume was decided in one command. No inference from branch state, no guessing whether
  a partially-completed step needed unwinding.

### Closed holds carrying the *reasoning* of prior human decisions

- Evidence: hold `m8-s4-split-at-the-seal-boundary` (sequence 396) recorded not just that a
  human chose a split, but why it strengthened the ordering guarantee, and that the orchestrator
  had corrected the presented `m8-s4a`/`m8-s4b` shape to canonical `m8-s4`/`m8-s5` ids because
  non-canonical ids disappear from repository projection.
- Effect: when this session faced a structurally identical decision (split again, at a different
  boundary), the precedent — including that ids must stay canonical — was recoverable from the
  log alone. I reused the same id-contiguity reasoning without rediscovering it.

### The `dispatch`-record round index as the artifact-naming rule

- Evidence: `review-7.json` through `review-11.json` were named from `prior_count + 1` on
  `(m8-s1, step-critic)`, and five successive gates each found a genuinely new, deeper defect.
- Effect: no separate counter to drift. See Friction for the one place this collided.

## Friction and failures

### `pce ready` rejects every step graph because step edges cross milestone boundaries

- Severity: `high`
- Phase: `step planning`, `execution` (readiness at both altitudes)
- Observation: `pce ready --graph <milestone-N/steps.json>` exits 1 with
  `approved graph path … is not a conforming graph: graph node m8-s1 has dangling dependency id
  m7-s3`. The default newest-first walk logs `skipping approved artifact candidate … reason=graph
  node <id> has dangling dependency id <other-milestone-step>` for milestone 8, 7, 5 and 4, then
  falls through. `pce ready --graph <milestones.json>` returns `{"results":[]}`.
- Evidence: milestone-8 `m8-s1` depends on `m7-s3`, `m7-s4`, `m6-s1`, `m6-s2`; milestone-7
  `m7-s1` depends on `m2-s1`; milestone-5 `m5-s1` depends on `m4-s1`; milestone-4 `m4-s1`
  depends on `m3-s3`. The superseded five-node milestone-8 graph
  (sha256 `e04908efd3e77152fe6fc264467656ae52b89945e19b5487299206ce2a83d41d`) carries the
  identical four dangling ids, so this is **not** a regression introduced by this session's
  re-cut — it is a property the graph has always had.
- Inference: `SKILL.md` Phase 2 instructs the step planner to author `depends_on` edges whose
  `reason` names "the source-level code fact that makes the dependent step unbuildable until the
  dependency has merged". At the first step of a milestone that fact almost always lives in a
  *previous milestone's* step. So the skill's own edge-justification rule systematically produces
  the exact edges the single-graph validator treats as dangling. The validator appears to assume
  a step graph is closed under its own node set, which is true only for a milestone with no
  inbound cross-milestone dependency.
- Impact: the run's stated sole readiness authority has been inert at step altitude for its
  entire duration. Readiness was necessarily determined by orchestrator judgement instead —
  which is exactly what `SKILL.md` forbids ("The orchestrator consumes classifications; it does
  not fold dependencies, merge observations, dispatch history, or contract policies into its own
  readiness judgement"). Worse, the failure is *quiet* on the default path: an orchestrator that
  called `pce ready` without `--graph` and read only the `results` array would see a plausible
  empty or partial answer and never learn that its graph was skipped. I only found it because I
  passed `--graph` explicitly, which fails loudly, as documented.

### Round-index artifact naming collides with a prior session's on-disk artifacts

- Severity: `low`
- Phase: `step planning`
- Observation: `(m8-s1, step-critic)` had 6 prior `dispatch` records, so the protocol names the
  next artifact `review-7.json` — but `review-7.json` already existed on disk from the previous
  session, which dispatched its critics through raw `codex exec` and Claude subagents rather than
  through `pce dispatch`, and therefore recorded no `dispatch` records for them.
- Evidence: `milestone-8/` contained `review-1.json` … `review-7.json` while
  `pce status` reported `{"classification":"critique-producing","count":6,"node":"m8-s1","role":"step-critic"}`.
  The prior session's own dispatch evidence strings say "round 4" … "round 9", confirming its
  round numbering diverged from the log's.
- Inference: the index is derived from binary-owned issuance records, which is correct and
  drift-free *going forward*, but it has no relationship to filenames produced before issuance
  became binary-owned. Any resumed run that crosses that boundary can silently overwrite an
  audit artifact.
- Impact: one artifact would have been overwritten. I copied the whole `review-*.json` set into a
  `.superseded-5node/` directory before dispatching, so nothing was lost, but the protocol as
  written does not prompt that precaution.

## Recommendations

### Resolve `depends_on` ids against the union of approved graphs, not one graph

- Addresses: finding 1
- Change: when validating a graph for readiness, treat an id that matches `m<digits>-s<digits>`
  and belongs to a *different* milestone as an external reference to be resolved against the
  merge state of that node, rather than as a dangling id within this graph. A step whose external
  dependency has merged is `ready` with respect to that edge; one whose external dependency has
  not is `waiting`. An id matching no node in any approved graph remains a genuine dangling id.
- Location: the graph conformance check invoked by `pce ready` (both the `--graph` path and the
  newest-first candidate walk)
- Trade-off: readiness now depends on more than one artifact, so `--graph` no longer fully pins
  the inputs; the provenance check would need to cover each graph consulted. Also risks masking a
  true typo in a cross-milestone id, which the union lookup would now have to catch explicitly.
- Confidence: `high` that the current behaviour is wrong; `medium` on this specific resolution.

### Make a skipped candidate loud on the default path

- Addresses: finding 1
- Change: if the newest-first walk skips **every** approved candidate, exit non-zero with the
  accumulated reasons rather than returning a `results` array. A silent empty answer from a
  readiness authority is indistinguishable from "nothing is ready", which is a legitimate state.
- Location: `pce ready` default candidate selection
- Trade-off: a run whose graphs are all genuinely unparseable now halts instead of degrading;
  that is the intent, but it converts a quiet condition into a stop.
- Confidence: `high`

### Have the step critic check edge ids against the milestone graph it already receives

- Addresses: finding 1, at authoring time rather than consumption time
- Change: `SKILL.md` Phase 2 already gives the step critic `milestones.json`. Add an obligation
  that every `depends_on` id either names a node in this step graph or names a step of a
  milestone this milestone depends on — and that the critic report which, so the cross-milestone
  edges are visible as a deliberate set rather than discovered later by a validator.
- Location: `SKILL.md`, `## Phase 2 — Milestones to steps`
- Trade-off: one more obligation on an already long critic prompt.
- Confidence: `medium`

### Instruct a resuming orchestrator to preserve existing review artifacts before dispatching

- Addresses: finding 2
- Change: in the routing section, note that the derived index `n = prior_count + 1` names a
  filename that may already exist when resuming across a change in dispatch mechanism, and that
  existing `review-*` artifacts at that index must be preserved before the gate is issued.
- Location: `SKILL.md`, `## Routing, caps, and adaptation`
- Trade-off: none material.
- Confidence: `high`

## No-change decisions

- **The plan/critic cap of 3 was exceeded and that was correct.** The step-graph loop ran five
  planner/critic rounds. The short-circuit condition ("two consecutive verdicts with
  substantially identical blocking issue sets") never triggered, because each round found a
  strictly deeper defect: round 1 found four issues including a circular ownership assignment;
  rounds 2, 3 and 4 each found exactly one new constraint, each *earlier in the same call path*
  than the last (`Evidence.__init__` → `validate_prereg` → the entry point); round 5 closed the
  enumeration by proving no further constraint exists. The cap correctly forced an escalation at
  round 3, a human resolved it, and the resolution explicitly authorized continuation. **The
  existing mechanism handled this well and needs no change** — the cap plus stuck-detection
  distinguished convergence from thrashing exactly as designed. Worth recording as evidence
  *for* the current rule, since the raw round count looks alarming out of context.
- **`pce status` output size.** The snapshot is 188 KB and `SKILL.md` requires quoting it
  verbatim at three call points. I quoted the non-dispatch content and reported the dispatch
  array by count. This is a real tension between the verbatim rule and a long-running log, but it
  is a documentation ergonomics issue, not a correctness one, and I would not spend a change on
  it before the readiness finding.

## Suggested follow-up

- The dangling-id resolution touches `pce ready`'s provenance guarantees (`--graph` currently
  pins exactly one artifact by digest). Deciding what `--graph` means when readiness legitimately
  consults several graphs is a design question worth its own vision rather than a patch.
