# PCE workflow feedback: the result is five columns and a receipt

- Date: `2026-08-07`
- Orchestrator: `Claude Code, Opus 5, ultracode session`
- Run: `RivRetrieve @ planning/2026-08-07-the-result-is-five-columns-and-a-receipt`
- Outcome: `completed — all 11 steps and all 7 milestones merged to main; main independently verified green (1622 passed, all five gates)`

## Executive summary

Phase 0 through Phase 2 completed: a seven-milestone zero-edge graph and eleven
steps, all approved. Two findings dominate.

The highest-impact finding is a **readiness result that is indistinguishable from
a stall**: `pce ready` returned `dependency-inconclusive` for all eleven step
nodes, including six whose `depends_on` is empty. The cause was not a dependency
at all — the milestone integration branch did not yet exist on the remote. Since
SKILL.md forbids dispatching an inconclusive node and forbids an
orchestrator-side readiness rule, a literal reading of the workflow halts the
entire run with zero dispatchable nodes and no stated remedy.

The second is a **systematic planner failure induced by the revision prompt**:
all five revised step graphs replaced their node `summary` with a changelog about
the planning act. One milestone never recovered and exhausted its cap on a
formatting defect rather than a disagreement about the work.

## Evidence reviewed

- `planning/2026-08-07-the-result-is-five-columns-and-a-receipt/events.jsonl`
- `planning/.../milestones.json`, `review-1.json`, `review-2.json`
- `planning/.../milestone-{1..7}/steps.json` and their `review-{1,2,3}.json`
- `pce status`, `pce ready`, `pce contract check` command output in the session transcript
- `.pce/repository-contract.json` at `adebfd317fdf8f4708466a79653fd43e86b24996`

## What worked

### Edge refutation as the graph critic's primary obligation

- Evidence: `planning/.../review-1.json` blocking issue `M7-ROLLUP-EDGES` deleted
  all six edges in the milestone graph, showing each reason named an
  acceptance-criterion relationship ("Required for zone acceptance") rather than
  a build-order one, and that m7 consumes no artifact of m1–m6.
- Effect: the graph went from a false serial tail to seven genuinely concurrent
  milestones. The same rule at step altitude produced 9 of 11 steps dispatchable
  at once. This is the single largest throughput effect in the run.

### Requiring the critic to read source at a named ref

- Evidence: `review-1.json` non-blocking notes cite
  `ObservationResult` declaring `raw: RawPayload | None = None`, `conversion.py:55,77`,
  `engine.py:223,231`, `driver.py:74,84`, and `aggregate_daily` having no hits in
  `src`. Each was used to *refuse* to add an ordering edge.
- Effect: missing-edge findings were argued from source rather than intuition,
  and the critic corrected the orchestrator's own brief — it noted that the brief
  pointed at `results.py` for the m2/m3 question but that at the pinned ref the
  relevant symbols live elsewhere.

### The appendable environment-hazard list

- Evidence: the tracked contract's hazard entry predicted the exact failure text
  `Failed to initialize cache` / `Operation not permitted (os error 1)` that
  `pce contract check` produced on its first invocation.
- Effect: the failure was recognized immediately instead of being diagnosed from
  scratch. See the friction below for where the entry's stated remedy was wrong.

### Step-critic catching self-sufficiency defects prose review would miss

- Evidence: `milestone-3/review-1.json` named six omitted test files, two of them
  milestone-exit contract assertions; `milestone-4/review-1.json` found that
  `br_ana` and `no_nve` ship no `catalogue/native.parquet` and expose no
  `--native` build mode, and that seven tests pin a SHA-256 over
  `products.parquet` content that the planned deletion changes.
- Effect: these are the defects that would have reddened the test gate during
  execution. They were found before a worktree existed.

## Friction and failures

### `dependency-inconclusive` is returned for a missing integration branch, and the workflow has no remedy for it

- Severity: `high`
- Phase: `step readiness / Phase 3 entry`
- Observation: with all seven step graphs approved, `pce ready --graph
  <milestone-m>/steps.json` classified every one of the eleven step nodes
  `dependency-inconclusive`, including `m1-s1`, `m3-s1`, `m4-s1`, `m5-s1`,
  `m5-s2`, `m5-s3` and `m7-s1`, whose `depends_on` arrays are empty. At milestone
  altitude the same verb had returned `ready` for all seven milestones minutes
  earlier. Creating `pce/the-result-is-five-columns-and-a-receipt/milestone-1`
  from `main` and pushing it flipped `m1-s1` to `ready` across exactly that one
  change, with no other edit to the log, the graph, or the repository.
- Evidence: `pce ready --file planning/.../events.jsonl --vision-dir planning/... --graph planning/.../milestone-1/steps.json`
  before and after `git push -u origin pce/the-result-is-five-columns-and-a-receipt/milestone-1`;
  the earlier `pce status` snapshot showing
  `"fetch":{"failure":"... couldn't find remote ref refs/heads/pce/the-result-is-five-columns-and-a-receipt/milestone-1","state":"unavailable"}`
  and `"merge_status":"inconclusive"`.
- Inference: step readiness folds a merge or branch observation for the step's
  integration branch, and an unavailable fetch degrades to
  `dependency-inconclusive`. The classification name attributes the condition to
  the graph's dependencies, which is where an orchestrator looks first and where
  nothing is wrong.
- Impact: SKILL.md says of `dependency-inconclusive` — do not dispatch, surface
  it to the user as an unresolved condition, continue dispatching any `ready`
  result — and separately forbids adding an orchestrator-side readiness rule.
  With no `ready` node anywhere, a workflow-literal orchestrator stops the entire
  run and escalates a condition that one `git push` resolves. Nothing in SKILL.md
  states that integration branches must exist before step readiness is computed,
  and Phase 3 introduces the milestone branch only implicitly, at isolate time,
  as the parent the step branch is cut from.

### The revision prompt induced every planner to write a changelog into `summary`

- Severity: `high`
- Phase: `step planning`
- Observation: five step graphs were revised after a REVISE verdict. All five
  replaced their node `summary` with a report about the planning act. Examples:
  "Corrected graph written to steps.json. It addresses every blocking change,
  preserves the single independently mergeable step and zero-edge structure"
  (m3-s1); "Overwrote and JSON-validated the milestone-4 steps.json with one
  independently mergeable repository step" (m4-s1). Four recovered after an
  explicit register correction; m4-s1 never did and exhausted its three-round cap
  on this defect alone.
- Evidence: `milestone-{1,2,3,4,7}/review-2.json`, whose blocking issues are
  `m1-s1-summary-is-still-a-meta-report-not-a-step`,
  `m3-s1-summary-is-a-changelog-not-a-step`,
  `m7-s1-summary-replaced-by-revision-changelog`, and m2's B1 and m4's C1.
- Inference: partly the orchestrator's own prompt — it told the planner to "say
  so explicitly in the affected node's summary" if source refuted a review
  finding, which legitimizes meta-commentary in the one field the executor reads
  as scope. But the uniformity across five independent cold agents suggests the
  pull is structural: a revision dispatch frames the artifact as a *response to a
  review*, and the only prose field available absorbs that framing. SKILL.md's
  revision rule ("Every revision is a fresh invocation naming the artifact and
  `review-<n>.md`") does not say the artifact must not reference the review.
- Impact: two full planning rounds across five milestones — ten Codex dispatches
  and five gate dispatches — spent on a register defect, plus one escalation and
  one milestone entering execution with a scope-free summary.

### A cap exhausted on a formatting defect escalates identically to a substantive disagreement

- Severity: `medium`
- Phase: `step planning / escalation`
- Observation: m4-s1 hit the three-round cap. The blocking issue was entirely the
  `summary` register; the substantive findings had been understood and addressed
  in earlier rounds. The escalation the workflow requires is the same instrument
  used for a genuine BLOCK.
- Evidence: `escalation-open` at `m4-s1`, key `m4-s1-step-planner-cap-exhausted`,
  in `events.jsonl`; `milestone-4/review-{1,2,3}` showing the substantive B1/B2
  findings raised at round 1 and the register finding raised at rounds 2 and 3.
- Inference: the cap counts `(node, role)` dispatches without regard to whether
  successive blocking issue sets are about the same class of defect. SKILL.md's
  short-circuit rule fires on *substantially identical* issue sets, which pushes
  toward stopping earlier, not toward distinguishing a cheap defect from an
  expensive one.
- Impact: a human decision was required for what was, in the human's own framing,
  better resolved by moving criticism onto real code.

### The environment-hazard entry named the right symptom and the wrong remedy

- Severity: `medium`
- Phase: `orientation / repository contract`
- Observation: the hazard entry predicts the failure text exactly and prescribes
  "that config is missing or the temp cache was purged; recreate it and re-warm
  with `uv sync --reinstall` outside any sandbox." Running `uv sync --reinstall`
  succeeded and changed nothing: `pce contract check` failed again with the same
  EPERM on the same existing zero-byte file. The actual cause was location — the
  configured `cache-dir` was `/private/tmp/pce-uv-cache`, while the writable temp
  root the gate sandbox grants is the platform temporary directory
  (`/var/folders/.../T`). Moving the cache there and repointing `cache-dir` made
  all five gates green.
- Evidence: `~/.config/uv/uv.toml` before the fix; two `pce contract check`
  invocations bracketing `uv sync --reinstall`, both failing identically; a third
  after relocation exiting 0 with 1619 tests passed.
- Inference: the hazard text says the cache should live "under `$TMPDIR`", so the
  drift was in the machine config rather than in the entry's intent. The entry's
  *diagnosis* clause ("missing or purged") is what misdirects: EPERM on a file
  that demonstrably exists falsifies both.
- Impact: one wasted remedy cycle. Small in isolation, but the appendable list is
  precisely the mechanism meant to stop a later run from re-deriving this.

### The primary-repo `.pce/repository-contract.json` shape and the log payload shape differ, with no stated mapping

- Severity: `low`
- Phase: `repository contract`
- Observation: the tracked file nests gates under `stated.gates` and conventions
  under `stated.branches` / `stated.pull_requests`. The log payload requires flat
  `stated.format`, `stated.lint`, …, plus single-string `branch_convention` and
  `pull_request_convention`. SKILL.md authorizes the orchestrator to construct
  this payload by hand in exactly one case and says to "source every `stated`
  field … from that tracked file," but does not say how the nested branch and
  pull-request objects collapse into two strings.
- Evidence: the tracked file at `main`; the payload appended at `m1-s1`, whose
  `branch_convention` the orchestrator composed as
  `default=main; milestone=pce/<vision-slug>/milestone-<m>; step=pce/<vision-slug>/m<m>-s<s>`.
- Inference: the encoding is orchestrator-invented and will differ between runs
  and between orchestrators, which weakens it as a durable authority.
- Impact: none observed in this run; the fields were consumed by the orchestrator
  that wrote them.

### `pce contract check` pollutes the step worktree and can make an approved plan unexecutable

- Severity: `high`
- Phase: `execution / isolate`
- Observation: Phase 3's isolate step requires running `pce contract check` inside the
  step worktree. That runs the stated pytest gate, which creates an untracked
  `pytest-of-<user>/` directory at the worktree root. It is not in RivRetrieve's
  `.gitignore`, so it is still there when the executor starts. The m5-s2 executor
  returned `BLOCK`, `root_cause: step_plan`, summary prefixed `PLAN_INFEASIBLE:`,
  because its plan's acceptance criterion requires a final `git status` containing
  only untracked `pr-body.md`, and it correctly refused to delete data it was not
  authorized to remove. Three sibling executors — m1-s1, m3-s1, m5-s1 — silently
  tolerated the identical pollution and committed.
- Evidence: `git -C .worktrees/<slug>/m5-s2 status --short` reporting
  `?? pytest-of-nicolaslazaro/`; the m5-s2 `.codex-result.json` verdict; the same
  step re-dispatched after `rm -rf` of that directory returned `APPROVE` with one
  commit and no other change.
- Inference: the workflow mandates a step (worktree contract check) whose side
  effect can falsify a criterion that plans routinely state. The divergence between
  four executors given the same condition means the failure is nondeterministic
  across agents, so it will appear intermittently rather than reproducibly.
- Impact: one wasted executor round, and — worse — three commits produced by agents
  that were willing to proceed with a dirty tree, which is the disposition the
  criterion exists to prevent.

### Local integration refs go stale and a merge silently fast-forwards them onto main

- Severity: `high`
- Phase: `merge / recovery`
- Observation: after several step and milestone merges had landed on the remote, the
  orchestrator created a worktree from the LOCAL `pce/<slug>/milestone-3` branch and
  merged `origin/main` into it. The merge exited 0 with no conflicts. The result was
  wrong: the local branch was still at the vision's original base, so the merge
  fast-forwarded it onto main's tip, producing a worktree whose HEAD subject was
  main's last merge commit and which was `0` commits ahead of `origin/main` — the
  milestone's own step work was not in it. Resetting every local integration ref to
  its `origin/` counterpart and repeating produced the real conflicts (2 files for
  milestone-3, 4 for milestone-5).
- Evidence: `git -C .worktrees/<slug>/merge-m3 rev-list --count origin/main..HEAD`
  returning `0` and `git log -1 --pretty=%s` returning
  `Merge pull request #121 ... milestone-7`; after the reset, the same merge exited 1
  with `src/rivretrieve/_internal/registry.py` and `tests/test_internal_registry.py`
  unresolved.
- Inference: SKILL.md's cold-orchestrator rule 3 states exactly this ("After every
  step merge or milestone merge, refresh the local integration ref from the remote
  before branching from it"), and the orchestrator still violated it, because the
  rule reads as advice about *branching* while the failure occurred during
  *merging into* the stale ref. Nothing in the merge's own output signals the
  mistake — a clean exit 0 is indistinguishable from a correct empty merge.
- Impact: caught before any push, so no damage. Had it been pushed with `--force`
  it would have silently reverted a merged milestone. The near-miss is the finding.

### A foreground timeout on a dispatch consumes a round and produces no verdict

- Severity: `medium`
- Phase: `review`
- Observation: the m2-s1 `pr-reviewer` round-2 dispatch was killed by the
  orchestrator's own 10-minute command timeout before the child wrote its verdict.
  `pce dispatch` had already appended the issuance record, so the round was spent
  with nothing to show. The orchestrator appended a `delta` at `m2-s1` recording
  the cause as orchestration rather than the reviewed code, and re-dispatched as
  round 3 — the last round the cap allows.
- Evidence: `pce log read --file <log> --kind dispatch --node m2-s1` showing
  `pr-reviewer` at sequences 179 and 191 with only `review-pr-1.json` and
  `review-pr-3.json` on disk; the `delta` at `m2-s1`.
- Inference: issuance is appended before the child runs, which is correct for
  durability but means any orchestrator-side termination burns a round. Long gate
  children (this one exceeded 10 minutes) are normal, not exceptional.
- Impact: a step with two legitimate review rounds reached its cap on the third,
  with one round lost to infrastructure. Had round 3 returned REVISE, the step
  would have escalated for a reason unrelated to its code.

### A brief's prose refs and its `--ref` flag are independent and can disagree

- Severity: `medium`
- Phase: `review`
- Observation: the m2-s1 round-3 reviewer brief was produced by `sed`-substituting
  `__BASE__`/`__HEAD__` into a template that had ALREADY been rendered with concrete
  refs in an earlier round. The substitution matched nothing, so the prose carried
  the pre-amendment head `a83eebc` while the dispatch `--ref` carried the real head
  `40eb7c7c`. The reviewer detected the divergence itself, ran
  `gh pr view 116 --json headRefOid`, reviewed the real head, and stated the
  discrepancy in its summary.
- Evidence: `planning/.../milestone-2/step-1/review-pr-3.json` summary, which opens
  "APPROVE at the branch's real head 40eb7c7c (the brief's head a83eebc is one commit
  stale ...)".
- Inference: the delegation contract's ground-truth field and the binary's `--ref`
  are two surfaces carrying the same fact, with nothing checking they agree.
- Impact: none here, because the reviewer treated refs as ground truth and verified
  them. A less rigorous gate would have reviewed stale code and approved it.

### GitHub's `mergeable` is stale immediately after a push, and only `UNKNOWN` is a safe "ask again"

- Severity: `medium`
- Phase: `merge`
- Observation: after pushing a resolved merge to milestone-6's branch, `gh pr view 123
  --json mergeable` returned `CONFLICTING`. That was stale: `git merge-base
  --is-ancestor origin/main origin/pce/<slug>/milestone-6` returned true, so main was
  already contained in the branch and no conflict was possible. A subsequent query
  returned `UNKNOWN`, and once GitHub finished recomputing it returned `MERGEABLE` and
  the merge succeeded. The orchestrator's poll loop treated only `UNKNOWN` as
  "not settled yet" and accepted `CONFLICTING` as final, so the first attempt reported
  a conflict that did not exist.
- Evidence: `gh pr view 123 --json headRefOid,mergeable,mergeStateStatus` returning
  `mergeable=UNKNOWN state=UNKNOWN` for the pushed head `22619552`, bracketed by an
  earlier `CONFLICTING` and a later `MERGEABLE` with no intervening change to either
  branch; `git merge-base --is-ancestor` proving containment throughout.
- Inference: GitHub serves the previous computation until the new one completes, and
  the transition is not always through `UNKNOWN` from the caller's point of view.
- Impact: one spurious "milestone 6 CONFLICTING — needs resolution" report. Had the
  orchestrator trusted it, it would have dispatched a conflict-resolution agent
  against a branch with nothing to resolve.

## Recommendations

### Verify a reported conflict against git before acting on it

- Addresses: stale `mergeable` after a push
- Change: add to the merge sections: "Before treating a `CONFLICTING` result as real,
  confirm it against local git — `git merge-base --is-ancestor origin/<base>
  origin/<head>` proving containment, or `git merge-tree` showing actual conflicted
  paths. GitHub serves the previous mergeability computation for some seconds after a
  push, and the stale value is not always `UNKNOWN`."
- Location: `skills/pce/SKILL.md`, Phase 3 step 6 and the milestone merge paragraph
- Trade-off: one extra local git command per merge; no network cost.
- Confidence: `high` — the containment check and the eventual `MERGEABLE` agree
  against the transient `CONFLICTING`.

### Remove gate residue from the worktree before dispatching the executor

- Addresses: `pce contract check` polluting the step worktree
- Change: add to Phase 3 step 2, after the worktree contract check: "The contract
  check runs the repository's test gate inside the worktree and may leave untracked
  residue. Before dispatching the executor, restore the worktree to a clean untracked
  state, or the executor may correctly refuse a plan whose criteria require one."
  Independently, add `pytest-of-*` to RivRetrieve's `.gitignore` as an ordinary
  tracked edit outside a run.
- Location: `skills/pce/SKILL.md`, Phase 3 step 2; `RivRetrieve/.gitignore`
- Trade-off: the orchestrator must distinguish gate residue from a real tracked
  change. Scoping the cleanup to known-untracked residue keeps that safe.
- Confidence: `high` — the same step failed with the directory present and succeeded
  with it absent, nothing else changed.

### Say that integration refs are refreshed before merging INTO them, not only before branching

- Addresses: the stale local integration ref
- Change: extend cold-orchestrator rule 3 to: "After every step merge or milestone
  merge, refresh the local integration ref from the remote before branching from it
  **or merging into it**. Verify the refresh: a merge of `origin/main` into a stale
  integration ref fast-forwards silently and exits 0, discarding that milestone's
  work from the local ref. Confirm `git rev-list --count origin/main..HEAD` is
  non-zero before trusting a conflict-free merge."
- Location: `skills/pce/SKILL.md`, "Cold-orchestrator falsification rules", rule 3
- Trade-off: none; it is an addition that preserves the existing constraint, and it
  supplies the measurement that distinguishes the two cases.
- Confidence: `high` — measured on both sides of the ref reset.

### Do not let orchestrator-side termination consume a round

- Addresses: the timeout-killed dispatch
- Change: state in "Routing, caps, and adaptation" that a dispatch whose child was
  terminated by the orchestrator, rather than by the child's own completion or a
  model-side error, is reconciled with a `delta` AND does not count toward the
  `(node, role)` cap. Deriving that requires distinguishing the two, which the
  `dispatch-completion` record's terminal reason may already support — if it does,
  say so; if it does not, that is the smallest change worth making.
- Location: `skills/pce/SKILL.md`, "Routing, caps, and adaptation"; possibly the
  `dispatch-completion` payload
- Trade-off: a cap that ignores some dispatches is a cap an orchestrator could game
  by mislabeling. Tying it to the recorded terminal reason rather than the
  orchestrator's say-so avoids that.
- Confidence: `medium`

### Have `pce dispatch` cross-check the brief against `--ref`

- Addresses: prose refs disagreeing with `--ref`
- Change: as an experiment, have `pce dispatch` scan the caller tail for 40-hex
  strings and warn (not fail) when it finds one that is a valid object in the repo
  but differs from `--ref`. Cheap, catches exactly this class, and stays advisory
  because a brief legitimately names a base ref as well as a head.
- Location: `src/main.rs` dispatch argument handling
- Trade-off: a warning on every legitimate two-ref brief unless the check exempts
  `--ref`'s own merge base; may be more noise than value.
- Confidence: `experimental`

- Addresses: `dependency-inconclusive` for a missing integration branch
- Change: add to Phase 2, after milestone graph approval: for each milestone
  classified `ready`, create and push its contract-named integration branch from
  the default branch before computing step readiness. Additionally, in the
  `dependency-inconclusive` paragraph, add: an inconclusive classification whose
  cause is an absent or unfetchable integration branch is not an unresolved
  dependency; create the branch and recompute.
- Location: `skills/pce/SKILL.md`, Phase 2 and the `pce ready` paragraph in Phase 3
- Trade-off: creates branches for milestones that might never be dispatched. They
  are cheap and deleted at milestone merge.
- Confidence: `high` — the before/after readiness measurement isolates the cause
  to exactly that push.

### Distinguish the two conditions in the classification itself

- Addresses: same finding
- Change: emit a distinct classification (for example `integration-branch-absent`)
  or attach a `reason` field to `dependency-inconclusive` results, so the
  orchestrator can tell a graph-dependency problem from a git-observation one
  without inferring it.
- Location: the `pce ready` result type and its run-snapshot/JSON contract
- Trade-off: a new classification value is a compatibility surface; a `reason`
  field on the existing value is additive and cheaper.
- Confidence: `medium` — the remedy is clear, the right shape is a design call.

### Forbid meta-commentary in graph node prose

- Addresses: the changelog-in-`summary` failure
- Change: add to both planner sections: "A node's `summary` describes the
  repository after the node lands. Its grammatical subject is the repository,
  never the graph file, the review, or the planning act. A revision's summary is
  indistinguishable from a first draft's. There is no field in which to answer a
  reviewer." Add the mirrored rule to both graph critics: "a `summary` that
  describes the planning act rather than the work is a blocking finding."
  Optionally add the same sentence to the `graph.schema.json` `summary`
  description, where a schema-following author will see it.
- Location: `skills/pce/SKILL.md` Phase 1 and Phase 2; `schemas/graph.schema.json`
- Trade-off: none identified; it forbids only text that is already useless to the
  executor.
- Confidence: `high` — five of five revised graphs failed this way, and four of
  four recovered once told explicitly.

### Let a planning cap distinguish register defects from substantive ones

- Addresses: cap exhausted on a formatting defect
- Change: mark this as an experiment rather than a required change. One option:
  allow one additional round when every blocking issue in the latest verdict is
  confined to a single field and no issue disputes the work's content, and
  require that the extra round's brief constrain only that field.
- Location: `skills/pce/SKILL.md`, "Routing, caps, and adaptation"
- Trade-off: introduces a judgement the orchestrator must make about the *kind*
  of a blocking issue, which is exactly the sort of orchestrator-side predicate
  the workflow otherwise removes. That is why this is an experiment.
- Confidence: `experimental`

### Correct the uv hazard entry's diagnosis clause

- Addresses: the wrong remedy in the environment-hazard entry
- Change: in the RivRetrieve tracked contract, extend the entry (do not replace
  it): "If `uv sync --reinstall` outside the sandbox does not clear the error,
  the cache is not cold — it is in the wrong place. EPERM on a file that exists
  means `cache-dir` is outside the gate sandbox's writable temp root. Verify
  `cache-dir` resolves under the platform temporary directory (`/var/folders/...`
  on macOS), not `/private/tmp`, which is a different directory."
- Location: `RivRetrieve/.pce/repository-contract.json`, `appendable.environment_hazards`
  — an ordinary tracked edit outside a run, per SKILL.md
- Trade-off: none; it is additive and preserves the existing text.
- Confidence: `high` — measured on both sides of the relocation.

### Specify the tracked-file to log-payload mapping

- Addresses: the two contract shapes
- Change: state, in the one authorized hand-append case, the exact composition
  for `branch_convention` and `pull_request_convention` from the tracked file's
  nested objects — or have `pce contract bootstrap`/`refresh` be the only writer
  and direct the orchestrator to `pce contract refresh` in this case too.
- Location: `skills/pce/SKILL.md`, Phase 0
- Trade-off: removing the hand-append path narrows a documented escape hatch.
- Confidence: `medium`

### `pce contract refresh` leaves the tracked contract file dirty

- Severity: `low`
- Phase: `merge`
- Observation: `pce contract refresh` rewrites the tracked
  `.pce/repository-contract.json` in the working tree, not only the event log. After
  the milestone merges it left the file modified with a whitespace-only change,
  adding a trailing newline the committed file lacked. No `stated` field was altered.
- Evidence: `git diff .pce/repository-contract.json` showing only
  `-}\ No newline at end of file` / `+}`.
- Inference: the refresh serializes and rewrites unconditionally rather than writing
  only on a semantic change.
- Impact: the run left a tracked file dirty with nothing accounting for it. An
  orchestrator that does not check will either commit the noise into an unrelated
  step or report a dirty tree it cannot explain.

### Nothing gated the fully merged result until the orchestrator chose to

- Severity: `medium`
- Phase: `merge`
- Observation: every step ran the five gates in its own worktree, and each of the four
  conflict resolutions ran them on its merge commit. But the workflow never requires a
  gate run on the final default branch after the last milestone lands. Seven
  milestones built concurrently from one base merged into a union that no gate had
  ever measured as a whole. The orchestrator ran one voluntarily: green, 1622 passed.
- Evidence: `pce contract check` on a detached worktree at `origin/main` after the
  final merge, exit 0, `1622 passed, 2 skipped`; the run's own step gate runs peaked
  at 1637 and 1619 tests on partial unions.
- Inference: two-tier merge plus conflict-resolution gates make this very likely to
  pass, which is why its absence is easy to miss — but "likely" is not "measured",
  and the whole point of the concurrency is that no earlier gate run saw this tree.
- Impact: none observed. The finding is that the vision could have been declared done
  on inference rather than measurement.

### Require a gate run on the default branch after the last milestone merges

- Addresses: nothing gating the fully merged result
- Change: add to the `## Done` section: "Before declaring the vision delivered, run
  `pce contract check` against a clean checkout of each repository's default branch at
  its post-merge head and report the observed result. Concurrent milestones merge into
  a union no earlier gate run measured."
- Location: `skills/pce/SKILL.md`, `## Done`
- Trade-off: one full gate run per repository at the end of a run. For this repository
  that is about six minutes, against a vision that took hours.
- Confidence: `high` — cheap, and it converts the run's final claim from inference
  into measurement.

### Make `pce contract refresh` write only on a semantic change

- Addresses: the dirty tracked contract file
- Change: have refresh compare the serialized result against the file's current bytes
  and skip the write when only formatting differs; or state in Phase 0/Phase 3 that
  the orchestrator verifies `git diff .pce/repository-contract.json` after every
  refresh and restores the file when the change is cosmetic.
- Location: `pce contract refresh` implementation, or `skills/pce/SKILL.md` Phase 0
- Trade-off: none of consequence.
- Confidence: `high`

## No-change decisions

- **Two full milestone-graph rounds before approval.** Round 1 deleted every edge
  in the graph. That is the gate working, not friction; a one-round cap would
  have shipped a falsely serial graph.
- **Seven concurrent step-planner dispatches and seven concurrent critics.** No
  contention, no conflict, no repeated work observed. The fan-out cost is real
  but bought a correct answer per milestone.
- **The prohibition on the orchestrator restating status in prose.** It forced
  every claim in this report back to a command or artifact, which is why the
  readiness finding could be stated as a measurement rather than a guess.

## Suggested follow-up

- Confirm whether `pce ready`'s step-altitude classification consults the
  integration branch deliberately or as a side effect of the merge observation.
  The remedy differs: if deliberate, it is a documentation gap; if incidental, an
  absent branch is being conflated with an unfetchable one.
- Consider whether a milestone whose graph carries zero edges should skip the
  `--graph` round-trip at step altitude entirely, since every one of its steps'
  classifications is then determined by git observation alone.
