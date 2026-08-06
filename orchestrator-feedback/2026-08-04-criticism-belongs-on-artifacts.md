# PCE workflow feedback: adversarial review saturates against plans and stays productive against artifacts

- Date: `2026-08-04`
- Orchestrator: `Claude Code (Opus 5), resumed session, ultracode`
- Run: `planning/2026-07-31-values-carry-the-descriptors-that-dimension-them` — cross-repo, milestone 8 of 8, step m8-s4 through m8-s5
- Outcome: `in progress` — m8-s4 merged after three prior rejections; m8-s5 in planning

## Executive summary

One finding dominates and it changed the outcome of the run.

**Adversarial review has a target-dependent yield curve. Against a plan it saturates and then goes
negative; against an artifact it stays productive.** m8-s4's plan was gated **sixteen times** as a
document and executed **zero** times. It grew from 1165 to 2891 lines, and **six of the last seven
findings were defects the previous repair had introduced.** Twenty-three plan-writer rounds and
sixteen plan-critic rounds produced no commit. The human redirected the run — "move the centre of
gravity of the critic to the work and not the planning" — and a committed, testable artifact
existed on the next dispatch. Four code reviews then took it from hollow to merged.

The skill currently has no saturation signal and no instruction to switch targets. It gates plans
by default and reviews artifacts once, at the end, after the expensive part is over.

Four supporting findings, each of which cost a dispatch or a defect:

- **All-or-nothing executor refusal produced nothing to evaluate for three consecutive dispatches.**
  The mandated `PLAN_INFEASIBLE` behaviour is correct for whole-plan incoherence and wrong for a
  single bad requirement.
- **Mutation testing proves non-vacuity, not contract conformance.** Sixty-three killed mutations
  failed to detect that the merged checker implemented a different contract than the specification,
  because the tests were authored alongside the implementation and encode *its* contract.
- **A step's own artifacts can encode constraints its successor must violate.** Three instances,
  each true when written, each caught by the consumer rather than by any reviewer of the author.
- **A gate asked for an exhaustive battery can exhaust its budget and write no verdict**, losing
  every result.

## Evidence reviewed

- `planning/.../events.jsonl` — 229+ dispatch records at time of writing; m8-s4 alone: 23
  `step-plan-writer`, 16 `step-plan-critic`, 9 `step-executor`, 6 `pr-reviewer`
- `milestone-8/step-4/review-3.json` … `review-24.json` — 22 verdicts across plan and code review
- metis PRs #39 and #42 and their diffs at named refs
- Direct verification at source for every executor `BLOCK` claim
- Independent gate re-runs and mutation batteries reproduced by the orchestrator

## What worked

### Moving criticism from the plan to the built artifact

- Evidence: sixteen plan gates, zero artifacts. After the redirect: one dispatch produced commit
  `02dc6cc`; four code reviews with mutation batteries produced merged `e8670f8`. The first code
  review alone found six defects including *zero of twenty production clauses implemented* — a fact
  invisible in any document.
- Effect: broke a six-round deadlock in which every repair created the next contradiction.

### Mutation batteries with a validated no-op control

- Evidence: reviewers ran 49 and 50-mutant batteries in throwaway clones, re-stamping the digest
  envelope each time. One reviewer's control run **caught a flaw in its own method** — a naive
  re-stamp desynchronised `expected_complete_input_sha256`, which would have made every "kill" an
  artifact — and it fixed the method before trusting a single result.
- Effect: the only evidence in the run that actually settled "does this compute?", and it settled
  it against three prior attempts that had passed document-level checks while computing nothing.

### Honest partial delivery

- Evidence: once permitted to build everything satisfiable, disclose the rest and still commit, the
  executor delivered a reviewable artifact with four self-declared gaps, two labelled critical
  unprompted, and described six unfinished subcommands as *failing closed rather than falsely
  succeeding*.
- Effect: converted three consecutive no-artifact refusals into a review loop that converged.

### The `--planning-act` reversibility binding

- Evidence: `irreversible` on m8-s5 produced a plan whose first act is a refusal check, and whose
  author refused outright when it found the merged checker could not support the seal.
- Effect: the refusal was correct and was verified at source. This is the mechanism working exactly
  as intended.

## Friction and failures

### Plan gating has no saturation signal and no exit

- Severity: `high`
- Phase: `step planning`
- Observation: sixteen consecutive plan-critic rounds on one step. Findings never repeated, so the
  skill's stuck-detector ("two consecutive verdicts with substantially identical blocking issue
  sets") never fired — yet the loop was not converging, because each repair enlarged the surface.
- Evidence: plan grew 1165 → 2891 lines; rounds 10–15 all fought one predicate (copied-body
  liveness), each round finding the same defect displaced one level down; the loop only ended when
  a human changed the target of criticism.
- Inference: the stuck-detector tests for *repetition*, which is the wrong invariant. A loop can be
  non-repeating and still non-converging when repairs generate new defects. Prose has no refutation
  procedure, so a plan critic can always find another reading.
- Impact: roughly two thirds of the session's dispatches produced no artifact.

### `PLAN_INFEASIBLE` is all-or-nothing

- Severity: `high`
- Phase: `execution`
- Observation: the mandated executor prompt says, of an infeasible plan, "DO NOT work around it and
  DO NOT open a PR — report verdict=BLOCK". Three consecutive dispatches complied and produced no
  artifact.
- Evidence: two of the three `BLOCK`s were *correct* — verified at source, they identified genuine
  plan defects (a pre-registration demanding values only a future measurement can produce; a
  byte-identity requirement over a closure containing bindings with no counterpart). The third
  refused to build on the rejected attempts' commits. All three were right to refuse; none left
  anything to evaluate.
- Inference: the rule conflates "this plan is incoherent" with "one requirement in this plan is
  unsatisfiable". Only the former warrants producing nothing.
- Impact: three dispatches; the run had nothing to review until the rule was locally overridden.

### Mutation testing cannot detect contract divergence

- Severity: `high`
- Phase: `review`
- Observation: m8-s4 merged after 63 killed mutations across two batteries, and the very next step's
  planner found the merged checker could not support the seal at all.
- Evidence: `bind-seal` emitted a seven-key seal identity where the specification requires ten, and
  `verify-seal` `exact()`-rejected anything else; `verify-seal` required the seal commit's parent to
  equal a `base_commit` pinned to the pre-merge tip, so no seal that could ever exist would
  validate; `attest` hardcoded two closure counts as literal zeros.
- Inference: tests authored alongside an implementation encode *that implementation's* contract.
  Code and tests were mutually coherent and both diverged from the source. This is the
  internal-coherence trap the run already knew about, one level up.
- Impact: a merged step that could not be consumed; two corrective commits.

### A step's artifacts can forbid its successor's work

- Severity: `high`
- Phase: `execution`, `review`
- Observation: three separate instances in one milestone.
- Evidence: (1) `comparands.base_commit` pinned to the tip *before* the authoring step merged, so
  the step's own merge invalidated a pin inside its own artifact; (2) a seal-identity record whose
  key set the successor could not extend; (3) a test asserting that all of the successor's outputs
  do not exist, so the successor's own gate fails by construction.
- Inference: each constraint was **true when written**. No reviewer of the authoring step could
  falsify it, because it was not false yet. The failure surfaces only at the consumer.
- Impact: three corrective commits; one step-plan refusal.

### A gate can exhaust its budget and write no verdict

- Severity: `medium`
- Phase: `review`
- Observation: a reviewer attempted a 56-mutant battery at ~2 minutes of suite per mutant, completed
  20 with zero survivors, and ended its turn reporting progress instead of writing the verdict.
- Evidence: `pce dispatch` failed with `artifact output ... is missing`; `duration_ms 921934`. Every
  result was lost.
- Inference: mutation cost scales as suite-runtime × mutant-count, and the suite grew from 32 tests
  at 62s to 51 at 136s during the step. A battery affordable in one round is unaffordable two rounds
  later.
- Impact: one dispatch; recovered only because the re-dispatch carried the prior attempt's findings
  forward.

### Rejected attempts remain readable in the object store

- Severity: `medium`
- Phase: `execution`
- Observation: after a rejection resets the branch, the rejected commits persist by SHA. A
  re-dispatched executor found them, recognised them as prior work on the same step, and tried to
  adapt them.
- Evidence: commits `7ccddac` and `f56ed94`, both titled "test: seal production adjudication", each
  carrying a complete `check-production` tree. The executor correctly identified them as "the two
  rejected, self-agreeing contracts" and refused — costing a dispatch, because the prompt had never
  named them.
- Impact: one dispatch.

### A `stated.version_policy` override has nowhere to live

- Severity: `low`
- Phase: `execution`
- Observation: the human directed no further version bumps, overriding `SERIALIZE_DISPATCHES`. The
  skill forbids a run from editing any `stated` field, and the mandated executor prompt instructs
  the executor to read `stated.version_policy` and apply the bump it requires.
- Inference: an in-run override must be repeated in every subsequent dispatch prompt or the
  executor will bump. The event log records it, but nothing consumes that record.
- Impact: minor, but silent and recurring — one commit was authored with a bump that had to be
  stripped before merge.

## Recommendations

### Add a saturation rule that switches the target of criticism

- Addresses: plan gating has no exit
- Change: after N plan-critic rounds on one step (suggest 3) *or* on any round whose findings
  include a defect introduced by the previous repair, stop gating the plan and dispatch the executor
  with honest-partial-delivery permission. State explicitly that a reviewable artifact with
  disclosed gaps is preferred to an unbuilt plan with none.
- Location: `SKILL.md`, `## Phase 3 — Per step PCE-PR-C`, step 1; and `## Routing, caps, and adaptation`
- Trade-off: some genuinely bad plans reach an executor. That cost is bounded by one dispatch and
  the executor refuses; the current cost is unbounded.
- Confidence: `high` — this is the single change that would most have improved this run.

### Replace the stuck-detector's repetition test with a regression test

- Addresses: plan gating has no saturation signal
- Change: also short-circuit when a round's blocking issues include one *caused by the previous
  round's repair*. Non-repeating findings are currently read as progress; they are equally
  consistent with a plan generating defects as fast as it sheds them.
- Location: `SKILL.md`, `## Routing, caps, and adaptation`
- Trade-off: requires the critic to attribute a finding to a prior repair, which it can usually do
  since it is given the prior verdict.
- Confidence: `medium`

### Split `PLAN_INFEASIBLE` into whole-plan and single-requirement cases

- Addresses: all-or-nothing refusal
- Change: amend the mandated executor prompt so `BLOCK` is reserved for whole-plan incoherence or a
  boundary violation. For a single unsatisfiable requirement: implement everything else, implement
  the closest correct thing, disclose the limitation prominently in `pr-body.md`, still create the
  commit, and report `REVISE`. Keep "fabricated completeness is the unrecoverable error" explicit —
  a disclosed gap is the requested behaviour.
- Location: `SKILL.md`, `## Phase 3`, step 3, the verbatim executor prompt
- Trade-off: partial commits enter review. In practice this is what made review possible at all.
- Confidence: `high`

### Give `pr-reviewer` a contract-diff obligation distinct from mutation

- Addresses: mutation cannot detect contract divergence
- Change: require the reviewer to enumerate every key, count, closure and comparand the
  specification names and check each against what the code emits and accepts — independently of any
  mutation result. Probes worth naming: `exact()`-style key-set assertions that make the
  implementation authoritative; spec-mandated values appearing as hardcoded literals; **pins the
  step's own merge invalidates**; and pairs of artifacts required to be equal that cannot be.
- Location: `SKILL.md`, `## Phase 3`, step 5
- Trade-off: adds review cost on top of mutation. It caught three merged defects here that 63
  mutations did not.
- Confidence: `high`

### Require every step to ask whether its successor must violate what it authors

- Addresses: a step's artifacts forbidding its successor's work
- Change: add to the plan-critic and pr-reviewer obligations: for every pin, key set, assertion or
  test the step authors, ask whether the next step is required to violate it. Pay particular
  attention to anything pinned to the current tip, since the step's own merge moves it.
- Location: `SKILL.md`, `## Phase 3`, steps 1 and 5
- Trade-off: none material; it is a question, not a mechanism.
- Confidence: `high` — three instances in one milestone.

### Bound gate batteries and require the verdict be written

- Addresses: gate budget exhaustion
- Change: instruct gates to choose a bounded decisive subset, state which checks they ran and which
  they did not so the human knows the coverage being accepted, and **write the verdict as soon as it
  is justified**, updating it if more is learned. When re-dispatching a gate, carry the prior
  attempt's findings forward.
- Location: `SKILL.md`, `## Routing, caps, and adaptation`
- Trade-off: less exhaustive single rounds; far fewer lost ones.
- Confidence: `high`

### Name rejected commits as forbidden in every re-dispatch

- Addresses: rejected attempts remain readable
- Change: when re-dispatching after a rejection, name the rejected commit SHAs, state what each
  faked, and forbid reading or reusing them — paired with an explicit "author everything fresh; there
  is no prior draft to salvage", since a large authoring task otherwise reads as an adaptation task.
- Location: `SKILL.md`, `## Routing, caps, and adaptation`
- Trade-off: none.
- Confidence: `high`

### Give in-run policy overrides a consumed record

- Addresses: `stated.version_policy` override
- Change: either let `pce log` record a typed policy override that `pce ready` and the dispatch
  envelope consume, or state in `SKILL.md` that an override must be repeated verbatim in every
  subsequent dispatch prompt because the mandated executor text reads the contract directly.
- Location: `SKILL.md`, `## Phase 0`, version and tag behaviour
- Trade-off: a typed override widens the contract surface; the documentation-only fix is free.
- Confidence: `medium`

## No-change decisions

- **The cap of 3 with human escalation worked.** It fired correctly twice, a human resolved both,
  and the escalation records carry the reasoning forward. The problem is not the cap but the absence
  of a *target switch* when the cap is reached for the third time on one predicate.
- **`--planning-act irreversible` needs no change.** It produced exactly the conservative behaviour
  intended, including an outright refusal that was correct.
- **Verbatim status quoting.** The snapshot reached 188 KB with 229 dispatch records; quoting it
  whole is impractical and I quoted the non-dispatch projections instead. Worth revisiting
  eventually, but it is ergonomics, not correctness.

## Suggested follow-up

- The `pce ready` step-graph defect filed separately today
  (`2026-08-04-values-carry-the-descriptors-that-dimension-them.md`) remains open and independent.
- Consider whether the skill should ship a standing **code-review obligations** artifact analogous
  to its verdict schema. This run produced one by hand
  (`milestone-8/CODE-REVIEW-OBLIGATIONS.md`) and it materially improved every review after it
  existed — its contract-diff clause found three merged defects on its first use.
