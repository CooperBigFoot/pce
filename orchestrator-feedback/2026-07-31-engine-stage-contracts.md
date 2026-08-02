# PCE workflow feedback: engine stage contracts

- Date: `2026-07-31`
- Orchestrator: Claude Code (Opus 5), single resumed session
- Run: `RivRetrieve`, `planning/2026-07-28-engine-stage-contracts`
- Outcome: `paused` — 5 of 7 milestones merged to `main`; milestone 3 in execution, milestone 7 waiting

## Executive summary

This was a resume of an interrupted run. It merged milestones 2, 5 and 6 to `main`
(joining 1 and 4 from the prior session), landed 13 step PRs and 5 milestone PRs,
and opened **zero** new escalations — the three in the log (`red-baseline-gates`,
`unpushed-orientation-ref`, `readiness-authority-unobservable`) were all opened and
closed before this session.

The highest-impact finding is that **a plan can contradict the baseline it quotes,
and no gate in the workflow is looking for it.** Milestone 3's two plans each assert
`uv run ruff check --fix` "succeeds" while quoting an enumerated baseline stating the
same command yields exactly one error. Two adversarial plan-critic rounds per plan
missed it, because critics evaluate gate *outcomes* against the baseline and never
compare the plan's *prose* to the baseline the plan itself quotes. It surfaced only
when a zero-context executor hit the contradiction and returned `PLAN_INFEASIBLE`.

The second finding is a recurrence: the `uv build` contract gate cannot execute
inside the executor sandbox. The 2026-07-29 report already recommended an
`environment` root cause and running preflight through the executor's own sandbox.
Neither exists yet, and this run paid for it four times, twice by an executor
withholding a commit on completed, green work.

The most valuable thing the workflow did was catch a defect that reading could not
find. `m3-s1`'s plan was approved by a critic, executed, and its PR review then
proved by mutation that discarding every conversion issue passed the entire test
suite. That routed to `root_cause: step_plan`, the plan was corrected, and the
re-executed step now kills the mutation. No amount of plan reading would have found
it; only executing a mutation against delivered code did.

## Evidence reviewed

- `planning/2026-07-28-engine-stage-contracts/events.jsonl` — 218 records: 129
  `dispatch`, 58 `key-finding`, 24 `planning-artifact-approved`, 3 `escalation-open`,
  3 `escalation-close`, 1 `repository-contract`
- Dispatch roles: `step-plan-writer` 33, `step-plan-critic` 30, `step-executor` 19,
  `pr-reviewer` 15, `step-critic` 14, `step-planner` 13, `milestone-planner` 2,
  `milestone-critic` 2, `repository-analyst` 1
- Review artifacts under `milestone-{1..6}/step-*/review-*.md`
- `milestone-3/step-1/review-2.md` and `review-4.md` (the B1 cycle)
- `milestone-3/step-2/review-1.md` (M8 survivor), `milestone-3/step-3/review-1.md` (B1 dedup survivor)
- `milestone-5/step-2/review-2.md` (harness self-correction), `milestone-5/step-4/review-1.md` (stale count)
- `.worktrees/engine-stage-contracts/*/.codex-result.json` for every executor
- `pce status` snapshots taken before each merge and each removal
- GitHub PRs 25–37; `git log --merges main`
- This session's transcript

## What worked

### Mutation-based PR review as a gate distinct from plan critique

- Evidence: `milestone-3/step-1/review-2.md` — mutating
  `assemble(converted.value, provenance, converted.issues, raw)` to
  `assemble(converted.value, provenance, (), raw)` left `1 passed` and `ty check src`
  at 3. The plan critic had approved the same plan at `review-1.md`.
- Effect: caught a driver that silently discards every conversion issue — the exact
  property ADR 0009 exists to guarantee — at the only stage boundary that step owned.
  A plan critic reading prescribed code cannot find this; the fixture was vacuous, so
  the property was unobservable rather than wrong.

### `root_cause: step_plan` routing away from the executor

- Evidence: same review; the reviewer wrote *"The driver source itself is correct; the
  defect is that the plan prescribed a fixture that cannot observe the property its own
  done criterion claims"* and set `root_cause: step_plan`.
- Effect: sent the work to the plan writer instead of asking the executor to patch code
  that was already right. Three later findings took the same route
  (`m3-s2` M8, `m3-s3` dedup, `m5-s4` stale count).

### Durable review artifacts absorbing agent-transport failures

- Evidence: several critics and reviewers signalled idle without their verdict JSON
  reaching the orchestrator (`pr-review-m5s1`, `critic-m3s1-r1`, `pr-review-m6s2`,
  others); `critic-m3s2-r1` and `critic-m3s3-r1` terminated outright on an account
  weekly rate limit. In every case the orchestrator read the verdict from
  `review-*.md` on disk, or confirmed no artifact existed and re-dispatched cleanly.
- Effect: zero re-runs caused by lost messages. Because critics are pure gates that
  write only their own artifact and are barred from touching tracked files, an abrupt
  kill left nothing to repair — verified by digesting both plans and checking `main`,
  branches and worktrees after the rate-limit kill.

### Requiring `depends_on` reasons to cite source, and critics to refute them

- Evidence: `milestone-3/review-1.md` checked both `→ m3-s1` edges at the depth their
  reasons cited and additionally closed an escape route neither reason argued —
  `validate_catalogue` already exists and is already tested at
  `catalogues/schemas.py:110`, so `m3-s2`'s deliverable is *placement* at a call site
  that does not exist, which is genuine unbuildability rather than convenience.
- Effect: an ordering edge kept for a proven reason, and the graph's concurrency
  claims trusted for the right reason.

### Pre-declared sibling conflicts

- Evidence: `m6-s2` and `m6-s3` were planned concurrently from one base. Both plans
  declared the overlap in `SIBLING CONFLICT RISK` with exact regions and the required
  resolution. `milestone-6/step-2/review-3.md` §14c then *executed* the union
  resolution in a scratch copy and confirmed it passes. When PR 32 went `CONFLICTING`
  after PR 31 merged, `git merge-tree` showed the conflict exactly where predicted.
- Effect: the conflict was a mechanical formality with a pre-verified target rather
  than a surprise. The composed suite went 1054 → 1070, additive, nothing lost.

### Preflight against an enumerated baseline in a red repository

- Evidence: the `red-baseline-gates` hold enumerates exact counts (3 named `br_ana`
  failures, 9 format, 1 B905, 3 `ty check src`, 132 whole-project). Every preflight
  this session matched exactly, and every executor was handed the enumeration verbatim.
- Effect: no step confused inherited red for red it caused, across 19 executor
  dispatches.

## Friction and failures

### A plan can contradict the baseline it quotes, and two executors read the contradiction differently

- Severity: `high`
- Phase: step planning / execution
- Observation: `milestone-3/step-2/plan.md` "Expected results" item 3 states
  `uv run ruff check --fix` **succeeds** and "may mechanically fix the inherited B905
  file". B905 has no safe fix, so the command always exits 1. The same plan quotes the
  enumerated baseline stating that command yields exactly 1 error. The `m3-s2` executor
  returned `verdict=BLOCK`, `root_cause=step_plan`, summary prefixed `PLAN_INFEASIBLE:`,
  id `GATE-LINT-UNSAFE-FIX`, and withheld the commit — correctly, per its instructions.
  **`milestone-3/step-3/plan.md` carries the identical wording at line 924, and its
  executor proceeded normally**, reporting "repository format baseline remained exactly
  9 files and lint baseline exactly 1 B905" and committing `2db93dc`.
- Evidence: `.worktrees/engine-stage-contracts/m3-s2/.codex-result.json` versus
  `.worktrees/engine-stage-contracts/m3-s3/.codex-result.json`;
  `uv run ruff check` in either worktree prints `Found 1 error.` and
  `No fixes available (1 hidden fix can be enabled with the --unsafe-fixes option)`;
  `grep -n 'ruff check --fix'` on both plans shows the same sentence.
- Inference: the wording is not strictly infeasible — it is **ambiguous**, and the two
  executors resolved it differently. One read "succeeds" as exit-zero and halted; the
  other read it against the quoted baseline and continued. Both readings are defensible
  from the text. Separately, both plan critics ran the gates and compared counts to the
  baseline, which passed; neither compared the plan's own expectation prose to the
  baseline the plan quotes. `SKILL.md`'s plan-critic obligations name write-set
  completeness, verbatim authored data, and updated assertions — nothing about internal
  consistency between a plan's gate expectations and its quoted baseline.
- Impact: one executor dispatch wasted, a third plan-writer dispatch consumed at the
  cap, and — more seriously — **non-deterministic executor behaviour on a
  safety-critical gate reading**. The same ambiguous sentence halted one step and not
  its sibling, which means the defect is invisible whenever the charitable reading
  happens to win.

### The `uv build` contract gate cannot execute in the executor sandbox — recurrence

- Severity: `high`
- Phase: execution
- Observation: four executors (`m6-s3`, `m5-s2`, `m3-s1`, and by inheritance the
  pattern was pre-empted for later ones) reported `uv build` failing with either a
  denial reading `~/.cache/uv/sdists-v9/.git` ("Operation not permitted") or a uv 0.9.0
  `system-configuration` panic when given a writable cache. `m6-s3` and `m5-s2`
  returned spurious `REVISE`; `m3-s1` returned `BLOCK` and **withheld the commit** on
  complete, green work, stating "No commit was created because gates must pass before
  committing."
- Evidence: `.codex-result.json` for `m6-s3` (`BUILD_FRONTEND_SANDBOX`), `m5-s2`
  (`gate-build-frontend`), `m3-s1` (`build-environment`); the orchestrator reran
  `uv build` outside the sandbox for each and it produced
  `rivretrieve-0.1.49.tar.gz` and the wheel every time.
- Inference: the repository contract's `build` command is measured on the orchestrator's
  host but executed in the executor's sandbox, and nothing reconciles the two. The
  2026-07-29 report already raised both halves of this ("A contract gate that cannot
  execute inside the executor sandbox halts every step that builds"; "`root_cause` has
  no value for an environment fault").
- Impact: three verdicts that had to be manually reclassified, one withheld commit
  requiring an extra dispatch, and an orchestrator-side rule invented mid-run
  (pre-declaring the gate satisfied in later executor prompts) to stop it recurring.

### No rule for "executor produced correct work but withheld the commit"

- Severity: `medium`
- Phase: execution / recovery
- Observation: `m3-s1`'s re-execution left `driver.py`, `tests/test_internal_driver.py`
  and `pr-body.md` in the worktree, all green, with no commit. `SKILL.md` routes
  `BLOCK` to escalation and `REVISE` to a fix loop, but neither fits: the plan was
  fine, the code was fine, and the only missing act was `git commit`.
- Evidence: `.worktrees/engine-stage-contracts/m3-s1/.codex-result.json` (`verdict=BLOCK`,
  non-blocking notes "Implementation and focused tests are green", "No commit, tag,
  push, version change, or staging occurred"); the orchestrator then dispatched a
  hand-authored commit-completion prompt.
- Inference: the executor prompt's "gates must pass before committing" is unconditional,
  so any unsatisfiable gate — including an environmental one — strands finished work.
- Impact: an improvised dispatch class with no name in the workflow, and ambiguity about
  whether it consumes the fix cap. The orchestrator had to record a log note asserting
  it does not.

### Round-count derivation conflates defect rounds with mechanical re-dispatches

- Severity: `medium`
- Phase: all
- Observation: `SKILL.md` says "Derive rounds from dispatch records" and caps
  plan/critic and PR/fix loops at 3. This run produced at least four dispatch classes
  that are not defect rounds: (a) two dispatches at sequences 127–128 interrupted by
  session end that produced **no artifact**; (b) two critic dispatches killed by a rate
  limit that produced no verdict; (c) a conflict-resolution rebase dispatch for `m6-s2`;
  (d) the commit-completion dispatch above. All are indistinguishable from defect
  rounds in the dispatch record.
- Evidence: sequence 127/128 dispatch records with no corresponding `plan.md` on disk;
  the orchestrator appended explicit `key-finding` notes at `m6-s2`, `m6-s3`, `m3-s1`
  and `m3-s2` solely to keep cap accounting honest.
- Inference: the dispatch record has a `role` but no outcome or class, so "round" is
  inferred from a record that cannot distinguish "this attempt produced a critiqued
  artifact" from "this attempt produced nothing".
- Impact: four defensive log entries written purely to prevent future
  mis-derivation, and a real risk that a resumed run mis-counts a cap and escalates
  early or late.

### Every milestone's first step reports `dependency-inconclusive`

- Severity: `medium`
- Phase: step planning → execution handoff
- Observation: immediately after approving `milestone-3/steps.json`, `pce ready
  --graph milestone-3/steps.json` classified **all three** nodes
  `dependency-inconclusive`. Cause: the integration branch
  `pce/engine-stage-contracts/milestone-3` did not exist yet. Creating and pushing it,
  then re-running the identical command, returned `ready m3-s1, waiting m3-s2, waiting
  m3-s3` — exactly the approved graph.
- Evidence: both `pce ready` invocations recorded in the `m3-s1` key-finding; the
  `unpushed-orientation-ref` hold records the same mechanism for `m4-s1`/`m4-s2`.
- Inference: `SKILL.md` places integration-branch creation in Phase 3 step 2 (isolate),
  but step-altitude `pce ready` is consulted in Phase 3 step 0, before any isolate has
  run. So the first readiness query of every milestone necessarily precedes the branch
  it needs.
- Impact: a `dependency-inconclusive` that the skill says to surface as an unresolved
  condition is in fact routine and self-clearing. Treating it as the skill instructs
  would stall every milestone at its first step.

### Deleting integration branches after a milestone merge destroys step observability

- Severity: `low`
- Phase: merge / cleanup
- Observation: after milestones 2 and 6 merged to `main` and their integration branches
  were deleted as ordinary cleanup, all five of their steps flipped from `merged` with
  reachable squash commits to `merge-status=inconclusive`, permanently.
- Evidence: `pce status` before cleanup showed `m2-s1`, `m2-s2`, `m6-s1`, `m6-s2`,
  `m6-s3` merged with squash OIDs; after cleanup all five read `inconclusive`. Every
  step commit remains an ancestor of `main` (`git merge-base --is-ancestor`).
- Inference: status derives the integration branch name by convention and fetches it;
  an absent branch is an absent-branch observation. This is the same mechanism the
  `unpushed-orientation-ref` hold recorded for the non-prefixed `milestone-1`/
  `milestone-4` branches.
- Impact: none to correctness here, because those milestones were already on `main` and
  no readiness decision depended on them. But the workflow offers no guidance on the
  trade, and a run that deletes a branch before its milestone lands would lose
  observability it still needs.

### Mutation harnesses can silently reuse stale bytecode

- Severity: `medium`
- Phase: review
- Observation: the `m5-s2` round-2 critic initially reported mutation `M5f` as killed by
  the wrong test. It diagnosed the cause itself: `continue` → `break` shrinks the file
  by exactly 3 bytes, so two distinct mutants produced **identical-size** files written
  within the same mtime second, and CPython's `(mtime, size)` `.pyc` invalidation reused
  the first mutant's bytecode. It confirmed the mutants were genuinely different with
  `difflib`, re-ran the entire sweep with `PYTHONDONTWRITEBYTECODE=1` and
  `-p no:cacheprovider`, and reported only corrected figures.
- Evidence: `milestone-5/step-2/review-2.md`, "Method" and the M5e/M5f table.
- Inference: any mutation-based gate in this workflow is exposed to this. It was caught
  here only because that particular critic was suspicious of its own result.
- Impact: none this run, but a false "killed" is the most dangerous possible output of
  an adversarial gate — it reports a property as pinned when it is not. The
  orchestrator subsequently added the two flags to every mutation-running prompt by
  hand.

### Executor prompt construction is unspecified and was hand-rolled per dispatch

- Severity: `low`
- Phase: execution
- Observation: `SKILL.md` fixes the executor's *core* prompt text verbatim but says
  nothing about the surrounding context each dispatch needs (base ref, sibling
  boundaries, scope prohibitions, known-environmental gates). The orchestrator built
  each one by copying and `sed`-substituting the previous step's prompt. One derivation
  inverted a sentence about which sibling adds which re-export; it was caught by manual
  inspection before dispatch.
- Evidence: the `m6-s2` prompt derivation and the subsequent corrective edit, visible in
  the transcript; the `m6-s2` dispatch record notes the prompt "was DERIVED from the
  m6-s3 prompt by substitution and then MANUALLY VERIFIED for inversion errors".
- Inference: repeated ad-hoc derivation of a safety-critical prompt is an obvious place
  for an undetected error, since a wrong sibling boundary would surface only as a
  confusing merge.
- Impact: none realised, but the near-miss was caught by vigilance rather than by any
  rule.

## Recommendations

### Require plan critics to check gate expectations against the quoted baseline

- Addresses: "A plan can contradict the baseline it quotes"
- Change: add to the plan-critic obligations: *"Compare every stated gate expectation
  against the enumerated baseline the plan itself quotes. A plan that says a command
  'succeeds' while its own baseline says that command reports N errors is internally
  contradictory and is a blocking finding, even if the measured counts are correct.
  State each gate expectation as an exact expected exit status and an exact expected
  count, never as 'succeeds'."*
- Location: `SKILL.md`, Phase 3 step 1, the paragraph listing what the plan critic
  requires at minimum (write-set completeness, verbatim authored data, affected
  assertions).
- Trade-off: one more mechanical check per plan review; negligible cost.
- Confidence: `high` — the exit-status half matters most, since this run showed the
  ambiguity resolving differently in two executors reading identical text.

### Add an `environment` root cause routed to the orchestrator, not to a fix loop

- Addresses: "`uv build` cannot execute in the executor sandbox" (recurrence)
- Change: extend the verdict schema's `root_cause` enum with `environment`, and state in
  `SKILL.md` that an `environment` verdict is never a defect round, never consumes a
  cap, and is resolved by the orchestrator rechecking outside the sandbox.
- Location: `~/.claude/skills/pce/schemas/verdict.schema.json`; `SKILL.md`, "Routing,
  caps, and adaptation".
- Trade-off: a schema change; existing verdicts remain valid.
- Confidence: `high` — this is the second report to ask for it.

### Instruct the executor to proceed when the only failing gate is environmental

- Addresses: the withheld commit
- Change: amend the binding executor prompt: *"If a gate cannot execute in your sandbox
  for environmental reasons and every other gate is green, record it in your result and
  PROCEED to commit. Do not withhold the commit."* Pair with the `environment` root
  cause above.
- Location: `SKILL.md`, Phase 3 step 3, the exact executor prompt text.
- Trade-off: an executor could misclassify a real failure as environmental; the
  orchestrator already re-derives every gate independently before opening a PR, so the
  check is not lost.
- Confidence: `high`

### Give the dispatch record an outcome class

- Addresses: "Round-count derivation conflates defect rounds with mechanical re-dispatches"
- Change: add an optional `class` field to the `dispatch` payload with values
  `round` (default), `resumed` (prior attempt produced no artifact), `recovery`
  (conflict resolution, commit completion), and state that only `round` counts toward a
  cap.
- Location: `SKILL.md`, "Event log contract", `dispatch` payload; `pce log` validation.
- Trade-off: widens a `deny_unknown_fields` boundary, so it is a versioned change.
- Confidence: `medium` — the need is demonstrated; the exact vocabulary is a guess.

### State that a milestone's integration branch is created before its first readiness query

- Addresses: "Every milestone's first step reports `dependency-inconclusive`"
- Change: move integration-branch creation from Phase 3 step 2 into the transition into
  Phase 3, and add: *"Create and push `pce/<vision-slug>/milestone-<m>` from the
  milestone's base before the first step-altitude `pce ready` query. A
  `dependency-inconclusive` result whose cause is the absent integration branch is an
  absent-branch observation; create the branch and re-query before surfacing it."*
- Location: `SKILL.md`, Phase 3 preamble and step 2.
- Trade-off: none — it codifies what both this run and the `unpushed-orientation-ref`
  hold already did.
- Confidence: `high`

### Mandate deterministic bytecode settings in any mutation-running prompt

- Addresses: "Mutation harnesses can silently reuse stale bytecode"
- Change: add to the critic and PR-reviewer dispatch requirements: *"Run every mutation
  with `PYTHONDONTWRITEBYTECODE=1` and `-p no:cacheprovider`. Same-size edits written
  within one mtime second collide under CPython's `(mtime, size)` `.pyc` invalidation
  and will silently reuse stale bytecode, producing false 'killed' results."*
- Location: `SKILL.md`, Phase 3 step 1 and step 5 dispatch requirements. Generalise to
  "the language's compilation cache" if the workflow is not Python-only.
- Trade-off: Python-specific text in a language-neutral skill; the failure mode is
  severe enough to justify naming it.
- Confidence: `high`

### Defer integration-branch deletion until after the vision completes

- Addresses: "Deleting integration branches destroys step observability"
- Change: state that integration branches are removed only after the whole vision is
  done, or that the orchestrator must record each step's squash OID in a
  `key-finding` before deleting the branch that makes it observable.
- Location: `SKILL.md`, the milestone-merge and removal instructions.
- Trade-off: branches linger for the run's duration.
- Confidence: `medium`

### Provide an executor prompt template

- Addresses: "Executor prompt construction is unspecified"
- Change: add a template beside the fixed executor text with named slots — base ref,
  write-set, sibling boundaries, scope prohibitions, known-environmental gates — so
  each dispatch is filled in rather than derived from a sibling by substitution.
- Location: `SKILL.md`, Phase 3 step 3.
- Trade-off: more text in the skill.
- Confidence: `medium`

## No-change decisions

- **`pce ready` disagreeing with reality for milestones 1 and 4.** Their integration
  branches were cut without the `pce/<slug>/` prefix under a human-ratified hold, so
  their steps are permanently `inconclusive` and `m3` read as `waiting` despite both
  dependencies being merged. The orchestrator used the standing
  `readiness-authority-unobservable` authorisation, verified the digest and the merge
  evidence, and recorded the decision. This is a consequence of a one-off human
  decision recorded in a hold, not a workflow defect, and the existing hold mechanism
  handled it correctly.
- **The three-round cap.** No loop this run needed a fourth round. Every `REVISE`
  converged in one revision, including two that were re-dispatched after
  infrastructure failures.
- **Codex-authors / Claude-critiques separation.** No case appeared where the
  orchestrator would have done better by writing plan or product content itself. The
  one place the orchestrator authored text — executor prompts — is the one place a
  near-miss occurred, which argues for the separation rather than against it.

## Suggested follow-up

- **Milestone 7 carries a known-false instruction.** `milestone-5/step-4/review-3.md`
  established that the `m5-s4` step summary's flag — that `issue_codes.py` becomes
  orphaned once the legacy pipeline is deleted — is factually wrong, because
  `UsgsNwisObservationIssueCodes` remains referenced. This is inherited from the
  approved step summary, so it will reach milestone 7's planner as an approved
  artifact. Worth a mechanism for correcting a forward-looking claim in an already
  approved graph without re-approving the graph.
- **Experiment: give the plan critic the executor's actual sandbox.** Both the
  `uv build` recurrence and the gate-wording contradiction would have been caught if
  the plan critic had run the plan's gates in the same environment the executor uses,
  rather than in its own scratch copy on the orchestrator's host.
