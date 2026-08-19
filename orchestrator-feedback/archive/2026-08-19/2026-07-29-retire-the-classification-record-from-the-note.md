# PCE workflow feedback: retire the classification record from the note

- Date: `2026-07-29`
- Orchestrator: `Claude Code, claude-opus-5, PCE-PR-C orchestrator skill, fresh ultracode session`
- Run: `grant-proposal-nik`, vision directory `planning/2026-07-29-retire-the-classification-record-from-the-note`
- Outcome: `completed; all four milestones merged to main and all nine vision acceptance criteria verified, after three human escalations`

## Executive summary

> **CORRECTION, filed after the original report.** The central finding below was
> diagnosed correctly as to mechanism and wrongly as to state. The unscoped
> milestone selector was real, but it was **already fixed in the repository's
> source** at commit `1a1596e fix: scope milestone merge identity to vision
> (#87)`. The installed binary at `~/.local/bin/pce` was a dangling symlink to a
> `target/release/pce` built before that commit. The orchestrator read CURRENT
> source while running a STALE binary, and attributed the disagreement to an
> unfixed defect rather than to a stale install. After
> `cargo build --release`, `MilestoneMergeSubject::derive` formats
> `pce/{slug}/milestone-{m}` at `crates/core/src/run_state.rs:424`, and
> `pce ready` returns `waiting` for the gated nodes with the already-dispatched
> roots correctly filtered out. No `dependency-inconclusive` result remains.
> The recommendation "Scope the milestone merge subject to the vision slug" is
> therefore **already implemented** and is retained below only as a record of
> what the run observed. The findings that survive unchanged are the stale-install
> exposure, the unpushed integration branches, the total-deadlock rule gap, and
> the classification-naming issue. See "Recommendations" for the one new
> recommendation this correction adds.

Phase 0 and Phase 1 completed and produced a milestone graph that survived two
adversarial critic rounds. Phase 2 could not dispatch, because
`pce ready` classified all four milestone nodes `dependency-inconclusive`,
including the two nodes whose `depends_on` is the empty list.

`pce` identified a milestone by the pull-request selector
`head=milestone-<m>, base=main`. That selector carried no vision scope, so it
matched the merged pull requests of every earlier PCE run in the same
repository. In this repository the selector matched four historical pull
requests for `milestone-1`, two for `milestone-2`, and two for `milestone-3`.
Multiple exact matches make the git authority unreachable by construction, and
the two authorities then disagree, which yields `MergeStatus::Inconclusive`.

The consequence was general. Step branches were vision-scoped
(`pce/<vision-slug>/m<m>-s<s>`), milestone integration branches were not
(`milestone-<m>`). Milestone-altitude readiness therefore degraded to
permanently inconclusive in any repository that had completed one PCE run and
merged a milestone pull request. This repository had completed at least three.

Since the fix was already committed, the operative finding is different: an
orchestrator has no way to detect that the `pce` on its PATH is older than the
`pce` source it reads, and that mismatch cost this run a human escalation, an
authorised deletion of three remote branches that was irrelevant to the cause,
and a full rebuild of the branch topology after the convention changed
mid-run.

A second finding is that `SKILL.md` instructs the orchestrator to create
integration branches but never to push them, while both `pce status` and
`pce ready` observe them by fetching from `origin`. The orchestrator had to
improvise the push.

A third finding is that the skill's `dependency-inconclusive` handling has no
rule for the case where no node is `ready`, which is a total deadlock rather
than the partial-progress case the rule anticipates.

## Evidence reviewed

- `planning/2026-07-29-retire-the-classification-record-from-the-note/events.jsonl`, the run's full event log, 17 records
- `planning/2026-07-29-retire-the-classification-record-from-the-note/milestones.json`, approved, sha256 `0a79f50ca9e6d71087c344fe3ffecf743f39df676ab9a2ec390c05041639b713`
- `planning/2026-07-29-retire-the-classification-record-from-the-note/review-1.md`, milestone-critic round 1, `REVISE`, two `major` blocking issues
- `planning/2026-07-29-retire-the-classification-record-from-the-note/review-2.md`, milestone-critic round 2, `APPROVE`
- `pce status --file <LOG> --vision-dir <VISION> --human` output at three points in the run
- `pce ready --file <LOG> --vision-dir <VISION> --graph <GRAPH> --policy grant-proposal-nik=NONE` output at five points in the run
- `gh pr list --head milestone-<m> --base main --state all --limit 1000 --json number,headRefName,baseRefName,state,mergeCommit` for m = 1, 2, 3, 4
- `pce` source at `src/main.rs` and `crates/core/src/run_state.rs`
- `git ls-remote --heads origin`, `git branch --merged main`, `git log --oneline main..<branch>`

## What worked

### The two-round plan-and-critic loop on the milestone graph

- Evidence: `review-1.md` returned `REVISE` with blocking issues B1 and B2, both `major`, both against node `m3`. B1 observed that `m3` bundled vision Scope-In item 5 with item 6, that item 5's diff is five Markdown files none of which sit under `design_note`, and that the bundle therefore forced unconstrained work behind two milestones and collapsed the graph to a serial chain. B2 observed that item 5's content exists only as uncommitted modifications plus one untracked file in the primary worktree, and is therefore unreachable from a worktree cut from `main`. `review-2.md` confirmed both resolved.
- Effect: B1 recovered concurrency that would otherwise have been lost for the whole run. B2 caught a defect that would have surfaced only at execution time, as a zero-context executor re-deriving governance text the vision explicitly forbids re-deriving. Both were found before any branch was cut.

### Placing the burden of proof on the presence of an ordering edge

- Evidence: the round-1 planner inverted the vision's own "Decomposition hints", which propose moving the classification record first and accepting a temporarily broken build. The critic tested the inversion in both directions against source and admitted it: `m1` alone leaves `\label{app:classification}` and `\label{app:trustworthiness}` defined-but-unreferenced, which matches none of the three build-gate patterns, while `m2` alone leaves exactly three `\ref` sites undefined at `design_note/sections/state_of_research.tex:124`, `:460`, `:462`.
- Effect: the delivered graph never leaves the build red, where the vision's hinted order would have. The refutation discipline overrode a human hint on source evidence, which is the behaviour the rule is written to produce.

### Critic independence from the previous round's conclusions

- Evidence: the round-2 critic prompt required re-checking at the ref every fact the revised graph's edges cite rather than trusting round 1. It found a fourth compiled `\ref{app:classification}` at `appendix_classification.tex:477` that round 1 had not reported, determined it does not refute the edge because it sits inside the file carrying its own label, and separately reconciled a discrepancy between the vision's stated line 121 for edit 3.1 and the actual reference site at line 124.
- Effect: a fact round 1 missed was caught, and a vision-level line-number inaccuracy was recorded before any executor met it.

### The `deny_unknown_fields` repository-contract boundary

- Evidence: the twelve-field contract append at `events.jsonl` sequence 8 was accepted only with exactly the twelve documented fields.
- Effect: the measured contract could not silently accumulate orchestrator-invented fields. Related: the contract carried the measured fact that `tectonic` is the only installed LaTeX engine and that `latexmk`, `pdflatex`, `xelatex`, `lualatex`, `biber` and `bibtex` are absent, which prevents a planner from authoring a gate command that cannot run on this machine.

### Digest-verified artifact provenance

- Evidence: `pce status` reported `artifact 1: path="planning/.../milestones.json" approved-sha256="0a79f50c..." approval-node="m1-s1" approval-sequence=14 condition=digest-matches`.
- Effect: `--graph` selection was verifiable rather than assumed, which let the orchestrator eliminate "wrong artifact selected" as a hypothesis for the readiness failure in one command.

## Friction and failures

### Milestone merge subject is not vision-scoped, so milestone readiness breaks permanently after a repository's first run

- Severity: `high`
- Phase: `milestone planning, Phase 2 dispatch gate`
- Observation: `pce ready` classified all four nodes of a verified-conforming graph `dependency-inconclusive`, including `m1` and `m3`, whose `depends_on` is `[]`. No node classified `ready`, so no step planner could be dispatched and the run could not advance.
- Evidence:
  - `crates/core/src/run_state.rs:419`, `MilestoneMergeSubject::derive` builds the head branch as `HeadBranch(format!("milestone-{}", node.milestone().get()))` and the base as `IntegrationBranch("main".to_owned())`. The vision slug appears nowhere.
  - By contrast `MergeSubject::derive(&vision, node)` for step altitude does take the vision, and the step selector observed in `pce status` was `head="pce/retire-the-classification-record-from-the-note/m1-s1"`.
  - `src/main.rs:1314-1329`, `github_pull_request_list_args` queries `gh pr list --head <head> --base <base> --state all --limit 1000`. The `--state all` makes historical merged pull requests part of the observation.
  - Reproduced directly. `gh pr list --head milestone-1 --base main --state all` returns four `MERGED` pull requests, numbers 57, 42, 40 and 11. `milestone-2` returns two, numbers 61 and 14. `milestone-3` returns two, numbers 63 and 16. `milestone-4` returns `[]`.
  - `src/main.rs:1357-1374`, three or more exact matches produce `GitHubPullRequestObservation::MultipleExactMatches`, and `observe_git` then returns `unreachable_git("multiple exact GitHub pull requests prevent squash OID selection")` without consulting git at all.
  - `crates/core/src/run_state.rs:669-720`, the pair `(MultipleExactMatches, GitUnreachable)` maps to `MergeStatus::Inconclusive`.
  - `crates/core/src/run_state.rs:1357`, a candidate whose own merge status is `Inconclusive` maps to `DispatchabilityResult::DependencyInconclusive` regardless of its edges. This is why `m1` and `m3` classify inconclusive despite having no dependencies.
  - `crates/core/src/run_state.rs:1373`, a candidate whose own status is `NotMerged` but which has an inconclusive dependency also maps to `DependencyInconclusive`. This is why `m4` classifies inconclusive: its own selector matched zero pull requests and resolved `NotMerged` correctly, but its dependency `m2` is inconclusive.
- Inference: the observed facts above are sufficient to explain all four classifications without any further assumption. The inference, clearly separated, is about generality: because the collision is between a run-invariant branch name and `--state all` pull-request history, any repository that has completed one PCE run and merged a `milestone-<m>` pull request will reproduce this. The first run in a fresh repository would not, because no history exists yet. I did not test a fresh repository, so that last clause is reasoning from the code path rather than an observation.
- Impact: the run halted at the Phase 2 gate with a fully approved milestone graph and could not proceed on the tool's authority. Four hypotheses were tested and eliminated before the cause was found, costing one human escalation, one authorised deletion of three remote branches that turned out to be irrelevant to the cause, and roughly a dozen diagnostic commands. The deeper impact is that milestone-altitude readiness, which `SKILL.md` names "the sole readiness authority at milestone and step altitude", is unavailable in exactly the repositories that have used PCE most.

### `SKILL.md` never instructs the orchestrator to push integration branches, but both status verbs observe them through `origin`

- Severity: `medium`
- Phase: `orientation, Phase 2 and Phase 3 boundary`
- Observation: after the orchestrator deleted three stale local integration branches and recreated them from `main`, `pce status` reported `fetch unavailable: failure="command git -C <root> fetch --no-tags origin refs/heads/milestone-1 exited exit status: 128; stderr: fatal: couldn't find remote ref refs/heads/milestone-1"`, and step merge status degraded from `not-merged` to `inconclusive`.
- Evidence: `src/main.rs`, the `ready` wiring calls `observe_repository(..., &[selector.base().as_str().to_owned()], &selector)`, so the base branch is fetched from `origin`, and `observe_git` returns `unreachable_git` when the fetch result is `FetchResult::Unavailable`. `SKILL.md` Phase 3 step 2 says to "create `pce/<vision-slug>/m<m>-s<s>` from that repository's `milestone-<m>` head" and Phase 3 step 4 pushes only the step branch. No instruction covers pushing `milestone-<m>`.
- Inference: the integration branch must exist on `origin` for the observation to be reachable at all, and it must exist there anyway before `gh pr create --base milestone-<m>` can succeed. The requirement is real but implicit, and the orchestrator discovered it only by reading the failure text.
- Impact: an orchestrator that follows `SKILL.md` literally will produce unreachable fetch observations for every step whose integration branch has not yet been pushed, and will then have to interpret degraded merge status. In this run the orchestrator improvised `git push -u origin milestone-1 milestone-2 milestone-3 milestone-4`. That improvisation was correct but unwritten.

### The `dependency-inconclusive` rule has no total-deadlock case

- Severity: `medium`
- Phase: `Phase 2 dispatch gate`
- Observation: `SKILL.md` says for `dependency-inconclusive` to not dispatch that node, surface it to the user, not silently treat it as `waiting` or `ready`, "and continue to dispatch any other results classified `ready`". Every result was `dependency-inconclusive`, so there was nothing to continue with, and the instruction gave no next action.
- Evidence: the `pce ready` output at four separate points in the run, each returning four `dependency-inconclusive` results and zero `ready` results.
- Inference: the rule is written for the partial case, where some nodes proceed and one is surfaced. It does not state whether total inconclusiveness is an escalation, a hold, or a stop, and it does not say whether the orchestrator may proceed on independently verified dependency information once a human authorises it.
- Impact: the orchestrator had to decide the escalation shape itself. It appended `escalation-open`, stopped, and asked the human. That was a defensible reading, but a different orchestrator could equally have read "surface it and continue" as licence to proceed, which would have bypassed the readiness authority silently.

### A zero-dependency node can be reported as inconclusive about its dependencies

- Severity: `medium`
- Phase: `Phase 2 dispatch gate`
- Observation: `m1` and `m3` have `depends_on: []` and were reported `dependency-inconclusive`.
- Evidence: `crates/core/src/run_state.rs:1353-1359`, the match on `status_for(candidate.node())` maps `MergeStatus::Inconclusive` to `DispatchabilityResult::DependencyInconclusive` before any edge is examined.
- Inference: the classification name describes the candidate's own merge observation, not its dependencies, in this branch of the match. The name is accurate for the `run_state.rs:1373` branch, where a dependency really is inconclusive, and misleading for the `run_state.rs:1357` branch, where the candidate's own identity is ambiguous.
- Impact: the label actively misdirected diagnosis. The orchestrator spent its first three hypotheses on dependency-side and git-state causes, because a node with no dependencies reporting a dependency problem reads as a graph or edge fault. The distinction between "I cannot tell whether this node is already merged" and "I cannot tell whether this node's dependency is merged" is the distinction that would have pointed at pull-request identity immediately.

### `pce ready --help` prints a usage error rather than help

- Severity: `low`
- Phase: `Phase 2`
- Observation: `pce ready --help` exited non-zero and printed the global `usage:` block prefixed with `Error:`.
- Evidence: the command output during diagnosis.
- Impact: minor. No per-verb documentation was available during a live diagnosis, so the orchestrator read the Rust source instead.

### A contract gate that cannot execute inside the executor sandbox halts every step that builds

- Severity: `high`
- Phase: `execution`
- Observation: the `m2-s2` executor halted twice with an identical blocking issue, which is the stuck condition. The contract build gate exited 101 because `tectonic` panicked before compiling.
- Evidence: the executor's `.codex-result.json` at both attempts reported `verdict=BLOCK`, `root_cause=step_plan`, summary prefixed `PLAN_INFEASIBLE:`. The captured panic is `thread 'reqwest-internal-sync-runtime' panicked at system-configuration-0.6.1/src/dynamic_store.rs:154: Attempted to create a NULL object`, followed by `reqwest-0.12.20/src/blocking/client.rs:1397: event loop thread panicked`. Isolated by running the same gate three ways: inside `codex exec --sandbox workspace-write` the default gate command and `tectonic -X compile --only-cached` BOTH exit 101; outside any codex sandbox the identical command in the identical worktree exits 0 with zero undefined citations and references. The Claude reviewer subagents ran the gate successfully throughout the run.
- Inference: macOS `SCDynamicStoreCreate` returns NULL because the sandbox blocks the SystemConfiguration daemon used for proxy autodetection, and tectonic constructs its network client before compiling regardless of flags. The observation is the exit codes and panic text; the attribution to proxy autodetection is inference from the panic site.
- Impact: two full execution rounds burned on the run's one irreversible step before the cause was isolated, then a human escalation to authorise `--sandbox danger-full-access` for that step. The failure is safe, because the gate refuses rather than passing wrongly, and the executor's halt rule correctly created no commit. Note that `m1-s1` and `m2-s1` passed this same gate earlier in the same session, and `m1-s1` explicitly reported recovering from this panic via a locally cached bundle, so the sandbox behaviour appears to have degraded mid-run.

### `root_cause` has no value for an environment fault

- Severity: `medium`
- Phase: `execution, routing`
- Observation: the executor correctly reported `root_cause: step_plan` for a failure the plan does not control and cannot fix.
- Evidence: the verdict schema's `root_cause` enum is `execution | step_plan | milestone_plan | vision`. The `m2-s2` failure was a sandboxed toolchain fault. The executor chose `step_plan` because the gate command is specified by the plan.
- Inference: `SKILL.md` routes `step_plan` to re-dispatching `step-plan-writer` with budget 2. Following that routing would have revised a correct plan twice and never addressed the cause. The orchestrator had to override the routing on its own judgement.
- Impact: the enum forces a misattribution, and the misattribution points the recovery path at the wrong artifact. A fifth value such as `environment`, routed to escalation rather than to replanning, would have surfaced the real condition on the first failure.

### A non-reproducible measurement nearly shipped as audit evidence

- Severity: `medium`
- Phase: `step planning, review`
- Observation: a PDF byte size of `296158` propagated from a step-plan critic verdict into a merged PR body and into the `m4` step graph, which would have published it in a permanent verification document.
- Evidence: `milestone-2/step-2/review-2.md` reported 296158. The `m4` step critic measured 296156 twice and refuted it. Independent re-measurement produced 296156, 296156 and 296158 across three consecutive builds of the identical commit `1df6c63`; a further three builds all sized 296156 had distinct MD5 sums, 275 differing bytes under `cmp -l`, and a per-build trailer `/ID`. `design_note/main.tex:42` sets `\date{\today}`.
- Inference: every party reported a real observation of an unstable quantity. Nothing in the workflow distinguishes a reproducible measurement from a one-off one, so a figure measured once acquires the same standing as a digest verified three times.
- Impact: caught before publication, by a critic checking values it had not been asked to doubt. The near miss is the finding: digests and counts were re-verified at every stage, while an incidental byte size travelled four artifacts unchallenged.

## Recommendations

### Scope the milestone merge subject to the vision slug

- Addresses: "Milestone merge subject is not vision-scoped".
- Change: give `MilestoneMergeSubject::derive` the vision, as `MergeSubject::derive` already receives it, and build the head branch as `pce/<vision-slug>/milestone-<m>` instead of `milestone-<m>`. Update `SKILL.md` Phase 3 to create, push, PR against, and merge that branch name. Note as corroboration that `origin` in this repository already carries `pce/trustworthiness-movement-and-classification-tables/milestone-1` through `milestone-6`, which suggests a vision-scoped integration branch name existed at some point and that the bare `milestone-<m>` form is a regression rather than a novel proposal.
- Location: `crates/core/src/run_state.rs:418-425` (`MilestoneMergeSubject::derive`), the milestone-altitude arm of the `ready` wiring in `src/main.rs`, and the branch, worktree, PR and merge language in `SKILL.md` Phase 3.
- Trade-off: existing in-flight runs that already created bare `milestone-<m>` branches would need those branches renamed, and any run resumed across the change would observe a different selector than the one its earlier records imply. Longer branch names appear in the GitHub UI.
- Confidence: `high`. The collision is reproduced, the code path is read end to end, and the fix removes the collision at its source rather than compensating downstream.

### Separate "candidate identity ambiguous" from "dependency inconclusive"

- Addresses: "A zero-dependency node can be reported as inconclusive about its dependencies".
- Change: add a fourth classification, for example `candidate-inconclusive`, emitted by the `run_state.rs:1357` arm where the candidate's own merge status is `Inconclusive`, and keep `dependency-inconclusive` for the `run_state.rs:1373` arm. Both remain non-dispatchable, so no dispatch behaviour changes. Update the `SKILL.md` classification list and the run-snapshot documentation accordingly.
- Location: `crates/core/src/run_state.rs:1351-1389`, the rendering match at `src/main.rs:653-660`, and the "Phase 3 - Per step PCE-PR-C" classification paragraph in `SKILL.md`.
- Trade-off: one more classification for orchestrators to handle, and a compatibility break for any consumer that enumerates exactly three classifications.
- Confidence: `high` that the distinction is real and diagnostically valuable; `medium` on the specific name.

### State that integration branches are pushed at creation

- Addresses: "`SKILL.md` never instructs the orchestrator to push integration branches".
- Change: in `SKILL.md` Phase 3 step 2, state that the integration branch is created from `main` and pushed to `origin` before the first step branch is cut, and give the reason, which is that both `pce status` and `pce ready` observe the base branch by fetching it from `origin` and that `gh pr create --base <branch>` requires it to exist there.
- Location: `SKILL.md`, "Phase 3 - Per step PCE-PR-C", step 2 "Isolate and preflight".
- Trade-off: none identified. The push is already required for the PR step to function.
- Confidence: `high`.

### Give the total-deadlock case an explicit rule

- Addresses: "The `dependency-inconclusive` rule has no total-deadlock case".
- Change: in the paragraph governing `dependency-inconclusive`, add that when no result at an altitude is classified `ready`, the orchestrator appends `escalation-open` naming every inconclusive node and stops, rather than proceeding on its own dependency judgement.
- Location: `SKILL.md`, "Phase 3 - Per step PCE-PR-C", the paragraph beginning "For `dependency-inconclusive`".
- Trade-off: an orchestrator that could have made correct progress will sometimes stop and ask. Given that the readiness verb is named the sole readiness authority, stopping is the behaviour consistent with that claim.
- Confidence: `high`.

### Add a regression test for repeated runs in one repository

- Addresses: "Milestone merge subject is not vision-scoped".
- Change: add a test that fixtures a repository with a merged pull request from an earlier vision at the same milestone index and asserts that a fresh vision's root milestone node classifies `ready`. The existing tests at `crates/core/src/run_state.rs:3659`, `:3769` and `:3885` construct `DependencyInconclusive` expectations directly and so cannot catch a selector-collision defect.
- Location: `crates/core/src/run_state.rs` test module, plus an integration test if pull-request observation is exercised at that level.
- Trade-off: the test needs a GitHub observation fixture rather than a pure in-memory state fixture.
- Confidence: `high` on the value; `medium` on placement, since I did not survey how the existing suite fixtures GitHub observations.

### Add an `environment` root cause, routed to escalation

- Addresses: "`root_cause` has no value for an environment fault".
- Change: add `environment` to the `root_cause` enum in `schemas/verdict.schema.json`, and in `SKILL.md`'s routing paragraph route it to `escalation-open` rather than to replanning. Instruct executors to use it when a gate command fails for a reason outside the plan's control, such as a toolchain crash.
- Location: `schemas/verdict.schema.json`, and the "Routing, caps, and adaptation" section of `SKILL.md`.
- Trade-off: one more enum value, and a compatibility break for consumers enumerating exactly four.
- Confidence: `high`.

### Require preflight to run the gates through the executor's own sandbox

- Addresses: "A contract gate that cannot execute inside the executor sandbox".
- Change: in `SKILL.md` Phase 3 step 2, run the contract `preflight` via a `codex exec` invocation with the same sandbox the executor will use, rather than from the orchestrator's shell. A gate that cannot execute in the executor's environment then fails before a plan is written instead of after two execution rounds.
- Location: `SKILL.md`, "Phase 3 - Per step PCE-PR-C", step 2 "Isolate and preflight".
- Trade-off: preflight costs one subprocess spawn per step instead of a direct shell call.
- Confidence: `high`. In this run it would have caught the blocker before any `m2-s2` planning.

### Require evidence in a verification artifact to be reproducible

- Addresses: "A non-reproducible measurement nearly shipped as audit evidence".
- Change: state in `SKILL.md` that a quantity reported as evidence must be measured at least twice with identical results, or else be reported with its variability. Cite build artifact sizes and timestamps as the standard examples.
- Location: `SKILL.md`, the "Evidence" guidance carried into plan and PR-review prompts.
- Trade-off: a second measurement for figures that go into durable records.
- Confidence: `medium`. The rule is cheap, though this run shows a determined critic catches such values anyway.

## No-change decisions

- **`--state all` in the pull-request query.** It looked like a candidate cause, and narrowing it to open pull requests would mask this defect. It would also break the legitimate case the status model depends on, which is recognising an already-merged step or milestone after the fact. The defect is the unscoped selector, not the state filter. No change recommended.
- **The three-valued `MergeStatus` and its two-authority disagreement model.** The pairing table at `crates/core/src/run_state.rs:641-760` behaved exactly as documented at every point in this run. Once the selector collision is fixed, the observations it receives become unambiguous. Preserve it.
- **The stale local and remote `milestone-1`, `milestone-2` and `milestone-3` branches.** These were investigated as a cause and eliminated: deleting the local branches changed nothing, and deleting the remote branches with human authorisation and then pushing all four integration branches fresh from `main` also changed nothing. Their existence was untidy rather than causal. No workflow rule is warranted, though the recommendation to scope integration branch names to the vision would prevent the untidiness as a side effect.
- **The orchestrator's `escalation-open` then stop behaviour at the gate.** It cost one human round trip. Given the skill's silence on total deadlock, stopping was correct, and the recommendation above makes it explicit rather than changing it.

## Suggested follow-up

- Audit whether any other merge subject, selector, or convention derived inside `pce` is run-invariant where it should be vision-scoped. The step subject is correct and the milestone subject is not, so the invariant is not enforced anywhere and a third case may exist.
- Decide whether the historical bare `milestone-<m>` pull requests in repositories that have already run PCE need any migration, or whether scoping new runs is sufficient to leave the history inert.
- Consider whether `pce status` and `pce ready` should report the underlying authority observations, such as the exact pull-request match count, alongside the derived classification. In this run the derived classification was reached in one command and the cause took four eliminated hypotheses and a source read; the match count alone would have pointed at it immediately.
