# PCE workflow feedback: catalogue-origins

- Date: `2026-08-01`
- Orchestrator: Claude Code, Opus 5, session `1c3f11f1`
- Run: `planning/2026-08-01-catalogue-origins` in `RivRetrieve`; log
  `planning/2026-08-01-catalogue-origins/events.jsonl`
- Outcome: in progress — milestone 1 merged to `main` (`7a93774`), milestone 2 two-thirds executed

Scope note: `pce contract bootstrap` being cargo-bound is already filed as
`2026-08-01-bootstrap-is-rust-only.md`, with a source-level analysis wider than what I reported.
Not repeated here.

## Executive summary

The single highest-impact finding is that **a three-lens parallel critique approved a plan carrying a
provable internal contradiction, which the executor then refused in one shot**. The lenses were
self-sufficiency, write-set completeness, and gate reachability; none of them owned the question
"are this plan's own done criteria mutually satisfiable?" Adding a fourth lens closed the instance.
The general lesson is that review quality is governed by **partition coverage, not reviewer count** —
and `SKILL.md` currently specifies neither.

Second: the event log's `key-finding` kind behaves as compounding memory in a way the skill does not
advertise. Pre-loading accumulated findings into later briefs coincided with a large drop in
convergence cost — `m1-s1` (4 files) took five plan-writer rounds and an executor rejection;
`m1-s2` (51 files) took two rounds and none. I can evidence the round counts and the brief contents;
I cannot prove the causal link, and mark it as inference.

Third: several defects were **environment-shaped rather than code-shaped** — the plan forbidding the
commit the executor must make, a regression proof that silently skips, a criterion falsified by the
harness's own artifact, and a step requiring network the executor sandbox does not have. `SKILL.md`
treats the executor's environment as a given; four separate blocking defects arose from it.

## Evidence reviewed

- `planning/2026-08-01-catalogue-origins/events.jsonl` — 60+ records at time of writing
- `pce status --file … --vision-dir …` snapshots at each merge point
- `pce ready --graph …` outputs at each altitude
- Plan artifacts and review documents under `milestone-1/step-{1,2}/`, `milestone-2/step-{1,2}/`
- Executor verdicts at `<worktree>/.codex-result.json` for `m1-s1` (BLOCK, then APPROVE), `m1-s2`,
  `m2-s1`
- PRs #52, #53, #54, #55
- `~/.claude/skills/pce/SKILL.md`

## What worked

### The `PLAN_INFEASIBLE` executor protocol

- Evidence: `m1-s1` executor returned `verdict=BLOCK`, `root_cause=step_plan`,
  `summary` prefixed `PLAN_INFEASIBLE:` with a single blocking issue: the plan mandated a test
  containing `assert "country" not in parameters` while a done criterion mandated
  `git grep -n -w country` over a path list including that test file return no matches. It made no
  commit and restored the worktree clean (`git log BASE..HEAD` empty, `git status` showing only the
  harness artifact).
- Effect: caught a defect that three independent critics had approved. The explicit
  "DO NOT work around it and DO NOT open a PR" instruction is load-bearing — a compliant executor
  surfaces the contradiction; a non-compliant one would have silently weakened the assertion or the
  criterion.

### `dependency-inconclusive` as a classification distinct from `waiting`

- Evidence: after approving `milestone-2/steps.json`, `pce ready --graph` returned
  `dependency-inconclusive` for all three `m2` steps. `SKILL.md` forbids treating it as `waiting` or
  `ready`. Diagnosis via `pce status` showed
  `git fetch --no-tags origin refs/heads/pce/catalogue-origins/milestone-2 exited exit status: 128`
  and `branch.state: absent`. Creating and pushing the integration branch flipped the same command to
  `ready`, `ready`, `waiting`.
- Effect: prevented dispatching two step-plan writers against an unobservable dependency state. A
  two-valued ready/waiting model would have silently guessed.

### Status snapshots cross-confirm git and GitHub independently

- Evidence: pre-merge snapshot for `m1-s2` showed `github.cardinality: one-exact-match` with
  `status: not-merged`; post-merge showed `git.state: squash-commit-reachable`,
  `squash_commit_oid: faf33bc…` and `github.…status: merged` agreeing.
- Effect: merge state is confirmed by two independent observers rather than by the command's own exit
  code.

### `key-finding` as compounding run memory

- Evidence: thirteen `key-finding` records. Contents were transcribed verbatim into later
  plan-writer and critic briefs — e.g. the `uv build` sandbox hazard, `uv sync` being an exact sync
  that removes optional extras, `F841` being an unsafe ruff fix, `pl.Schema` order-sensitivity, the
  station-metadata versus provider-info-metadata deletion test.
- Effect (observation): `m1-s1` required 5 `step-plan-writer` rounds, 4 `step-plan-critic` rounds and
  1 executor rejection for a 4-file change. `m1-s2`, a 51-file change, required 2 and 2 with no
  rejection. `m2-s1` required 1 and 1.
- Inference, not proven: the briefs carrying prior findings caused the drop. Confounders include
  differing step complexity and my own improving brief-writing.

## Friction and failures

### F1 — Three parallel lenses approved a plan with an internal contradiction

- Severity: high
- Phase: step planning (plan critique)
- Observation: `milestone-1/step-1/review-3-self-sufficiency.md`, `review-3-write-set.md` and
  `review-3-gates.md` each returned `APPROVE` with zero blocking issues. The executor then refused
  the same plan as infeasible.
- Evidence: the three review documents; `<worktree>/.codex-result.json` issue
  `PLAN-CONTRADICTION-1`; `plan.md:244` (mandated assertion) against `plan.md:390` (mandated
  no-match grep over a path list containing that file).
- Inference: not laziness. Each lens was scoped to a real question and answered it correctly. No lens
  was assigned "are the plan's own stated done criteria mutually satisfiable, and consistent with the
  file contents the plan itself mandates?" I had assumed three lenses covered the space; they
  partitioned it with a hole.
- Impact: one wasted execution round, one wasted plan-writer round, and the defect reached the last
  gate before the product.

### F2 — The plan-critic cap counts dispatches, which penalises fan-out

- Severity: medium
- Phase: all critique phases
- Observation: `SKILL.md` says round counts derive from `dispatch` records, keyed by `(node, role)`,
  cap 3. Fanning a critique round into four parallel lenses would append four `step-plan-critic`
  records and exhaust the cap on the first critique.
- Evidence: `SKILL.md` "Routing, caps, and adaptation"; my improvised workaround visible in the log —
  one `dispatch` record per round whose `evidence` names all four subagents and their review paths.
- Inference: the cap intends to bound *convergence attempts*, not *reviewer count*. The two coincide
  only when a round is one agent.
- Impact: I had to invent an unstated convention. A different orchestrator would plausibly log four
  records and hit the cap, or log one and lose the fan-out from the audit trail.

### F3 — Plan-writer boundaries propagated into the plan as executor boundaries

- Severity: high
- Phase: step planning
- Observation: my plan-writer brief said "Do not run the gates. Do not commit. Do not open a pull
  request" — constraints on the *plan writer*. The writer transcribed them into `plan.md` as
  constraints on the *executor*, listing `commits` and `pull requests` under "Out of scope" and
  adding a done criterion requiring that no commit exist.
- Evidence: `milestone-1/step-1/review-1-self-sufficiency.md` §6; round-1 `plan.md:347,358`.
- Impact: a compliant zero-context executor would have finished with a clean, correct, **uncommitted**
  worktree, silently delivering nothing. Compounded by a criterion using `git diff --name-only`,
  which reports unstaged changes and is empty *after* the mandated commit — false precisely when the
  executor behaves correctly.

### F4 — The harness pollutes the worktree it asks the executor to keep clean

- Severity: medium
- Phase: execution
- Observation: `codex exec -o <worktree>/.codex-result.json` writes into the executor's own worktree,
  and no `.gitignore` entry covers it. `git status --short --untracked-files=all` therefore reports
  it, so any plan asserting an exact untracked-file set is unsatisfiable on a retry.
- Evidence: `milestone-1/step-1/review-4-satisfiability.md` issue `SAT-1`; observed
  `?? .codex-result.json` in both `m1-s1` and `m1-s2` worktrees.
- Impact: one blocking defect and one plan-writer round. The executor's two options were both wrong —
  declare the step not-done over an artifact it did not create, or delete the orchestrator's result
  channel.

### F5 — The executor sandbox has no network, and `SKILL.md` never says so

- Severity: high
- Phase: step planning and execution
- Observation: a step graph required generating an artefact "from the live provider API". Executors
  run `codex exec --sandbox workspace-write` with no `sandbox_workspace_write.network_access`
  override and `~/.codex/config.toml` has no such table, so the fetch is impossible.
- Evidence: `milestone-2/review-2.md` §6, which established this from the codex binary's own
  documentation strings, the absent config table, and every `step-executor` dispatch record in the
  log; `key-finding` at node `m2-s2`.
- Inference: `SKILL.md` specifies the executor's sandbox mode and writable roots but says nothing
  about network reachability, so a planner has no basis for knowing a network-touching step is
  unexecutable.
- Impact: one blocking defect and one step-planner round, on a milestone whose entire subject is a
  networked `refresh` versus a pure `build`. The honest resolution — the orchestrator performs the
  fetch outside the sandbox and the payload plus its retrieval instant become *inputs* to the step —
  is a pattern the skill does not describe.

### F6 — `dependency-inconclusive` reports the classification but not the cause

- Severity: medium
- Phase: readiness
- Observation: `pce ready` returned three `dependency-inconclusive` results whose JSON carried only
  `classification`, `node`, `repository`. Diagnosing it required a separate `pce status` call and
  reading a nested `git.failure` string.
- Evidence: the `pce ready` output; the `pce status` `git.failure` naming the failed
  `fetch --no-tags origin refs/heads/pce/catalogue-origins/milestone-2`.
- Impact: `SKILL.md` instructs surfacing the classification to the user as an unresolved condition. I
  instead diagnosed and fixed it in one step, because the cause was a missing precondition I owned.
  Following the instruction literally would have escalated a self-inflicted, self-fixable condition.

### F7 — Documentation-carried claims falsified by diffs, caught by no gate

- Severity: medium, recurring
- Phase: step planning and PR review
- Observation: three instances in one run.
  1. `README.md:26` called `map_stations(providers=…, country=…)` after the parameter was removed;
     `pyproject.toml:5` declares that file as packaged metadata.
  2. `architecture.md` declared itself "the current implementation contract" while mandating the
     `metadata` column and per-provider pydantic models that the run's binding ADRs delete.
  3. `AGENTS.md` §4 forbids generating `catalogue/*.parquet` from a `tests/test_data/` fixture — the
     exact path and provenance of an artefact a step was about to commit.
- Evidence: `milestone-1/review-1.md` B1; my own `git grep -c engine architecture.md` (1 of 668
  lines); `milestone-2/step-2/review-1-write-set.md` §4, including
  `git grep "AGENTS.md" -- tests src pyproject.toml` returning nothing.
- Impact: all three passed every repository gate. Ruff, ty and pytest cannot read prose. Each was
  caught only because a reviewer was explicitly tasked with enumerating gate-invisible consumers.

### F8 — `pce dispatch codex` cannot spawn `codex`

- Severity: low (workaround is trivial)
- Phase: all Codex dispatches
- Observation: exits 1 with ``failed to spawn `codex`: No such file or directory (os error 2)``
  while `codex-cli 0.146.0` resolves at `/opt/homebrew/bin/codex`, which is on `PATH`.
- Evidence: `key-finding` at node `m1-s1`; `which -a codex`; every subsequent dispatch used direct
  `codex exec` successfully.
- Impact: one phantom `milestone-planner` dispatch record, which inflated that role's round count by
  one against a cap of three. Possibly already addressed by the in-flight
  `2026-07-31-the-binary-owns-every-dispatch` vision.

## Recommendations

### R1 — Name the required critique partition, not a reviewer count

- Addresses: F1
- Change: state in `SKILL.md` that a plan critique must cover four questions, and that they may be
  answered by one agent or by several in parallel: (a) **self-sufficiency** — can a zero-context
  executor act on this alone; (b) **write-set completeness** — is any file the change affects
  missing; (c) **gate reachability** — does every stated gate pass afterward; (d) **done-criteria
  satisfiability** — is every stated criterion satisfiable *against the file contents the plan itself
  mandates*, and non-contradictory with the others. Add that (d) must treat any author-written
  self-audit as unverified.
- Location: `SKILL.md`, "Phase 3 — Per step PCE-PR-C", step 1, where the critic's minimum obligations
  are already listed.
- Trade-off: prescribes review structure the skill currently leaves open.
- Confidence: high. (d) is the one that caught the defect the other three missed, and its absence is
  what let it through.

### R2 — Make the round, not the agent, the explicit unit of the cap

- Addresses: F2
- Change: one sentence — "A critique round may be implemented as several parallel agents; append one
  `dispatch` record for the round, naming each agent and its artifact in `evidence`. Caps count
  rounds."
- Location: `SKILL.md`, "Routing, caps, and adaptation".
- Trade-off: none identified; it codifies what the cap already means.
- Confidence: high.

### R3 — Separate the plan writer's boundaries from the executor's mandate

- Addresses: F3
- Change: require every `step-plan-writer` dispatch to state the executor's mandate affirmatively
  (exactly one conventional commit; `pr-body.md` untracked at the worktree root; no tag, push,
  attribution footer, or version change under `NONE`), and to warn the writer not to propagate its
  own boundaries into the plan. Additionally require file-set done criteria to be expressed against
  the commit (`git show --format= --name-only HEAD`) rather than the unstaged diff.
- Location: `SKILL.md`, Phase 3 step 1.
- Trade-off: lengthens the plan-writer contract slightly.
- Confidence: high — this defect would have silently voided a step.

### R4 — Keep the result channel out of the executor's worktree, or ignore it

- Addresses: F4
- Change: preferred — write the verdict artifact outside the worktree and pass its path, so the
  executor's tree is untouched. Minimal alternative — have `pce contract bootstrap`/`refresh` ensure
  `.codex-result.json` is git-ignored, since `git status --untracked-files=all` does not list ignored
  paths. Either removes the class.
- Location: `SKILL.md` Phase 3 step 3 (`-o` path), or the contract writer.
- Trade-off: the preferred option changes a documented invocation shape.
- Confidence: high.

### R5 — State the executor's network posture, and name the orchestrator-fetch pattern

- Addresses: F5
- Change: add to the executor description that `--sandbox workspace-write` does **not** grant network
  access unless explicitly configured, so no step may require the executor to make one. Add the
  resolution pattern: where a step needs live data, the orchestrator performs the fetch outside the
  sandbox and supplies the payload and its retrieval instant as inputs, recording URL, instant,
  canonicalization and digest as a `key-finding` so the claim is auditable.
- Location: `SKILL.md`, Phase 3 step 3, alongside the existing sandbox and writable-root text.
- Trade-off: adds an orchestrator responsibility that is currently implicit.
- Confidence: high. This is also the only shape that keeps a `retrieved_at` honest, and that field
  fed two downstream artefact fields in this run.

### R6 — Carry the cause into `dependency-inconclusive`

- Addresses: F6
- Change: include the underlying observation in the `pce ready` result — at minimum a reason string
  such as `integration-branch-absent` or the failed git command — so the orchestrator can tell a
  missing precondition it owns from a genuinely unresolvable dependency. Separately, note in
  `SKILL.md` that a milestone's integration branch must exist and be pushed before readiness is
  computed for its steps.
- Location: `pce ready` result schema; `SKILL.md` Phase 3 step 2.
- Trade-off: widens a stable output schema.
- Confidence: medium on the schema change, high on the `SKILL.md` precondition note.

### R7 — Make gate-invisible consumers a standing review obligation

- Addresses: F7
- Change: add to the write-set obligation an explicit clause: enumerate consumers no gate reads —
  files declared as packaged metadata in the build config, README and docs examples, module
  docstrings containing runnable snippets, and the repository's own instruction files
  (`AGENTS.md`/`CLAUDE.md`) where a rule may be falsified by the change.
- Location: `SKILL.md` Phase 3 step 1, in the critic's listed minimum obligations.
- Trade-off: slightly widens every write-set review.
- Confidence: high — three instances in one run, all gate-invisible, one of which would have shipped
  a `TypeError` on the packaged front-page example.

### R8 — Recommend mutation as the test-review instrument (experiment)

- Addresses: not a failure; a mechanism that outperformed inspection
- Change: suggest that a PR review lens covering tests ask "would this test fail if the code were
  wrong?" and answer it by mutating the source and re-running, rather than by reading.
- Location: `SKILL.md` Phase 3 step 5.
- Evidence: the PR #55 test lens proved two named properties were unasserted — collapsing a type
  union to one member left the suite green, and `eq=False` left it green at 16 passed. One of the
  tests was a tautology (a runtime-erased annotation compared against its own constructor calls)
  whose *name* promised the property the milestone's scope discipline rests on.
- Trade-off: costs a mutate-and-run loop per reviewed property; only worth it where a type's
  invariant is load-bearing.
- Confidence: experimental as a general rule, high for steps that define vocabulary later milestones
  build on.

## No-change decisions

- **The `PLAN_INFEASIBLE` protocol needs no change.** It caught the defect three critics missed and
  cost only a clean worktree. It is the correct last line of defense; F1's fix is upstream of it.
- **The two-tier merge needs no change.** Squash-into-milestone then merge-commit-into-default worked
  exactly as specified; PR #54 landed with two parents and both step commits reachable.
- **The `--graph` requirement on `pce ready` needs no change.** Passing the recorded path explicitly
  at both altitudes was unambiguous throughout, including with two approved graphs in play.
- **Verbatim status quoting at merge points needs no change.** It is ceremony that paid for itself
  once — the `dependency-inconclusive` diagnosis came directly from a `pce status` field.

## Suggested follow-up

- **The `retrieved_at` provenance pattern may deserve first-class support.** This run had the
  orchestrator perform a live fetch, verify content-identity against a committed fixture by canonical
  digest, and record URL, instant, canonicalization and digest so a downstream artefact field could
  honestly claim a retrieval time. That is currently prose in a `key-finding`. If other runs need
  maintainer-time network inputs, a typed event kind would make it machine-checkable rather than
  narrative.
- **Consider whether `SKILL.md` should require an integration-branch precondition check** as part of
  Phase 2's dispatch of `ready` milestones, rather than leaving it to be discovered at Phase 3.
