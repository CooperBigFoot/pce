# PCE workflow feedback: post-2019-deep-learning-directions

- Date: `2026-07-27`
- Orchestrator: `Claude Code, claude-opus-5, single session with one context compaction`
- Run: `planning/2026-07-25-post-2019-deep-learning-directions` in `grant-proposal-nik`; branches `milestone-1`, `milestone-2`, `milestone-3`; merged to `main` at `b6dc801`
- Outcome: `completed` (Effort ticket #5 landed; 3 milestones, 16 steps, PRs #44–#63)

## Executive summary

The workflow delivered the vision, and its adversarial gates caught a class of defect
that no mechanical verification could have found: a systematic bias toward the note's own
hypothesis appearing in **four distinct forms** across two drafting steps, every individual
citation faithful at every round. That is the workflow's strongest demonstrated result in
this run and it justifies the cost of the gate.

The dominant waste came from the opposite direction. In step `m2-s3` the shipped prose was
final at commit `555f548`; **five subsequent amendments touched only the verification
record**, four of them shipped false statements about that unchanged prose, and the
component was ultimately withdrawn by human decision (`state.json.escalations[E5]`). The
workflow has no concept for "this artifact component cannot be maintained accurately;
withdraw it," so the loop ran to cap exhaustion before a human supplied that move — the
second time in this run (`E1`–`E3` on `m1-s12` is the same shape).

Three findings from the immediately preceding report,
`2026-07-27-no-d8-raster-lies-silently.md`, **recurred here in stronger form**:
orchestrator-authored prompt content passing through no gate, partial fixes reading as
complete, and `gh pr merge --delete-branch` colliding with an active worktree. That report
also observed "a fifth round would have been needed had any round-3 critic found a real
defect." In this run a round-3 critic did, and the cap was exhausted.

The highest-impact recommendation is not a new gate. It is a rule about *how orchestrator
instructions are phrased*: three of my sweep instructions failed identically because I
scoped them to a **cause** rather than stating an **invariant**, and one inlined a claim
broader than the test that produced it.

## Evidence reviewed

- `planning/2026-07-25-post-2019-deep-learning-directions/state.json` — 16 step records, 6 deltas (`D1`–`D6`), 6 escalations (`E1`–`E6`), 10 notes, `counters` with per-artifact round histories
- 16 `plan.md` files, 6 `plan.round1.md` / `plan.replan-r1.md` supersessions, 36 `review-<n>.md` verdicts, 3 `steps.json` plus 2 `steps.round<n>.json`, `milestones.json` plus `milestones.round1.json`
- Git history on `main` (`e637750` → `b6dc801`) and the reachable `m2-s3` amendment chain `ff3c3c8 → 555f548 → 559ede6 → 296beae → 9dafc13 → 6450c8f → cfe27f9`
- `git diff --stat` across each amendment pair, and `git diff --quiet 555f548 cfe27f9 -- design_note/sections/state_of_research.tex` (exit 0)
- `/Users/nicolaslazaro/.claude/skills/pce/SKILL.md`, sections `## The delegation contract`, `## Phase 2`, `## Phase 3`, `## Routing, caps, and adaptation`, `## Conflict & recovery`
- Prior report `orchestrator-feedback/2026-07-27-no-d8-raster-lies-silently.md`
- This session's transcript for dispatch prompts and verdict routing

## What worked

### Adversarial PR review caught bias that per-claim verification cannot see

- Evidence: `milestone-2/step-1/review-1.md` (round 1 REVISE, 4 major); `milestone-2/step-2/review-1.md`, `review-2.md`, `review-3.md`; `state.json.notes[7]`. Four distinct forms: (i) a statistic detached from its recorded evaluation and repositioned after counter-evidence; (ii) half a recorded two-sided balance carried, the half cutting against the note dropped; (iii) comparison controls dropped at the two contests where a learned model beat a physical or operational benchmark; (iv) no-single-attribution controls present wherever they qualified counter-evidence (`C88`, `C94`, `C113`) and absent wherever they would qualify support (`C118`, `C123`, `C86`).
- Effect: form (iv) is visible only by comparing paragraphs. `m2-s1`'s own note records that 23 rows were spot-checked with no unsupported sentence while the argument still leaned. Had the step merged after round 1, the section would have contained every correct number and still favoured the thesis.

### Plan critics verifying the plan's own factual assertions

- Evidence: three catches, all of orchestrator-authored error. `milestone-1/step-5/review-1.md` (I asserted `xiang2022fullyDistributed` builds its graph over unit catchments; the PDF says nodes are land grid cells). `milestone-1/step-12` replan (I carried a `+4273` offset finding whose text existed only in discarded commits; would have forced a fabricated retraction). `milestone-3/step-1/review-1.md` finding `B1` (below).
- Effect: this is the only gate that reads *upstream* rather than downstream, and it caught every orchestrator factual error in the run.

### Derivation rules instead of closed enumerations

- Evidence: `state.json.counters.m2_step_plan_history` round 2 records that mandatory countervailing lists transposed from a record verdict row both over-included `EXCLUDED`/`OMIT` rows and omitted the findings ADR 0010 actually names. The round-3 fix replaced the list with a rule: every PASS row in `C78`–`C135` whose claim, headline, or column 19 records a countervailing outcome is mandatory.
- Effect: a critic independently surfaced five rows the enumeration had missed (`m2-s2` "Fifth-direction countervailing derivation" verdict row: `C80`, `C87`, `C88`, `C108`, `C109`, `C113`).

### `state.json` as the resume spine across context compaction

- Evidence: this session was compacted once mid-run. `state.json` carried `repo_contracts`, per-step `base_ref`/`head_ref`/`squash_commit`, round counters with prose histories, and `notes[0]` recording that `planning/` is gitignored and absent from every worktree.
- Effect: work resumed without redoing any merged step. The per-round prose histories in `counters` were more useful than the numeric counts.

### Cap exhaustion routed to a human produced a better outcome than more automation

- Evidence: `E4` (five counter-evidence passages: carry, leave, or split) and `E5` (withdraw the inventory component). Both were decided by the human; both decisions were ones the pipeline had no basis to make. `E5`'s remedy mirrors the `m1-s12` precedent recorded in `E1`–`E3`.
- Effect: `E4` resolved a conflict the vision does not adjudicate. `E5` ended a loop that had already consumed five passes.

### The executor refused to commit on a red gate

- Evidence: `state.json.deltas[D6]` — the `m2-s2` executor drafted the direction correctly (311→504 lines) then reported `BLOCK / PLAN_INFEASIBLE` because `tectonic` 0.16.9 exits 101 panicking on macOS network configuration inside the sandbox. Fixed by `sandbox_workspace_write.network_access=true`; verification then passed with no prose change.
- Effect: correct behaviour under an environment fault. The executor did not fabricate a green gate.

## Friction and failures

### The record was rewritten five times after the prose was final

- Severity: high
- Phase: execution / review (`m2-s3`)
- Observation: `state_of_research.tex` reached its final bytes at `555f548`. Five subsequent amendments changed only `docs/verification/0005-state-of-research-post-2019.md`, at 6, 2, 7, 6, and 7 changed lines respectively. Four of the five shipped verdict rows asserting things the unchanged prose contradicted. The component was withdrawn at `cfe27f9`.
- Evidence: `git diff --quiet 555f548 cfe27f9 -- design_note/sections/state_of_research.tex` exits 0. `git diff --stat` per pair as listed under Evidence reviewed. `milestone-2/step-3/review-2.md`, `review-3.md`, `review-4.md`. `state.json.escalations[E5]`.
- Inference: the failing rows asserted properties of a 522-line document — omission enumerations, completeness words, position words — each requiring whole-file verification. The workflow gates *whether an artifact satisfies its plan*; it has no notion of an artifact component whose accuracy cost exceeds its value.
- Impact: five execution passes and three PR-review rounds after the deliverable was correct, ending in deletion of the work product of those passes. This is the single largest measurable waste in the run.

### Orchestrator-authored prompt content passes through no gate — recurrence, and worse

- Severity: high
- Phase: step planning (`m3-s1`)
- Observation: I ran a scan for `' -- '` (spaced double-hyphen), got zero, and inlined into the plan the far broader claim that the movement contains "no double-hyphen". The movement contains 41 `--` occurrences, all legitimate LaTeX en-dashes (`1989--1999`, `rainfall--runoff`, `Kolmogorov--Smirnov`, `0.8368--0.9988`).
- Evidence: `milestone-3/step-1/review-1.md` finding `B1`, severity critical. `state.json.notes[9]`. Independently re-derived: U+2014 count 0, `---` count 0, `--` count 41.
- Inference: the plan's "already mechanically verified" list is the only place a zero-context executor is told to *skip* a real acceptance check, and it rests on orchestrator authority. The same plan also directs the executor to "correct every substantiated defect."
- Impact: had the critic not been pointed at my own assertions first, the executor would plausibly have rewritten 41 en-dashes across prose required to stay byte-identical. The prior report recorded this class as medium severity with one wasted planning round; here it reached critical.

### No rule distinguishes cause-scoped from invariant-scoped instructions

- Severity: high
- Phase: execution (`m2-s3`)
- Observation: three consecutive sweep dispatches failed identically. I wrote "correct rows made stale by *that resolution*", so the executor correctly ignored identical falsehoods produced by a *different* cause. Sweeps 1 and 2 under-corrected; sweep 3 then over-corrected, inflating accurate clauses (`C125`, `C87`, `C130`) into unsupportable ones.
- Evidence: `milestone-2/step-3/review-2.md` (`BI-2`), `review-3.md` (`R2-1`, `R2-2`, `R2-3`). `state.json.notes[8]`.
- Inference: cause-scoped instructions leak exactly as closed enumerations do — the defect reappears through whichever channel the instruction did not name. All three failed sweeps *reported success*, so a summary of what changed is not a sufficient completion signal.
- Impact: three of the five wasted passes above. Fixed only when the instruction stated the invariant ("every prose-facing clause must be true of the prose at HEAD") plus per-item justification.

### Cap exhaustion occurred exactly where the prior report predicted it

- Severity: medium
- Phase: review (`m2-s3`)
- Observation: `m2-s3` consumed all three PR-review rounds and terminated `REVISE at cap`. `state.json.steps["m2-s3"].pr_review_verdict` reads "REVISE at cap, resolved by human decision E5". Round counts across the run: `m1-s12` 4 PR rounds under a replanned contract, `m2-s2` 3, `m2-s3` 3, `m1` step graph 3, `m2` step graph 3.
- Evidence: `state.json.counters.m1_step_plan_history` and `m2_step_plan_history` (both 3 rounds); `milestone-2/step-3/review-4.md`. Prior report, section "Four artifacts consumed exactly the 3-round cap".
- Inference: critics surface defects in layers rather than exhaustively. The `m2-s3` sequence is the clearest instance: rounds 1 and 2 each cleared the prior round's findings and found new ones of the same class.
- Impact: one escalation. The cap functioned as designed — it stopped an unconverging loop — but the workflow gave no guidance on what to *propose* to the human at that point, which I had to invent.

### The escalation terminal state is undefined

- Severity: medium
- Phase: execution (`m2-s3`)
- Observation: the approved plan instructed the executor, on hitting a fidelity-versus-rebuttal tension, to record the tension and "not commit". Execution step 21 (write `pr-body.md`) was gated on the commit. I overrode this in the dispatch: leave the affected prose unchanged, complete every other correction, commit, and name escalated items atop `pr-body.md`.
- Evidence: `milestone-2/step-3/review-1.md` non-blocking note explicitly flags the contradiction ("the plan should state explicitly what the executor leaves behind"). The override text appears in the `m2-s3` executor dispatch. Result: `ff3c3c8` carried 6 corrections plus 5 recorded escalations.
- Inference: `## Routing, caps, and adaptation` defines escalation for the *orchestrator* (write to `state.json.escalations`, stop, report) but never for an *executor* that must escalate mid-step.
- Impact: the plan as approved would have stranded the entire step on one unresolved disposition. Caught by a critic's non-blocking note, then fixed by improvisation rather than by rule.

### No disposition exists for withdrawing an unmaintainable artifact component

- Severity: medium
- Phase: review / escalation (`m1-s12`, `m2-s3`)
- Observation: both step failures ended with the same remedy — delete the component, keep the verified corrections, add an honest disposition row — and in both cases a human supplied it. `E1`–`E3` record three consecutive `BLOCK`s on `m1-s12` for false clean coverage verdicts; `E5` records five failed passes on the per-item inventory.
- Evidence: `state.json.escalations[E1..E3]` and `[E5]`; `state.json.deltas[D5]` ("Root cause sits in the step contract I authored, not in the executor"); the `Narrative-balance disposition` row now in the record.
- Inference: verdict routing offers `APPROVE`, `REVISE`, `BLOCK`. There is no route for "the artifact is correct but this component of it is not sustainably verifiable." Both times the loop ran to exhaustion first.
- Impact: the second occurrence cost five passes. The precedent from the first was carried only in my prose notes, not in any workflow rule.

### Folding a single-node graph review into the plan critic dropped an acceptance criterion

- Severity: medium
- Phase: milestone planning (`milestone-3`)
- Observation: `milestone-3/steps.json` has one node. I judged a separate graph critique to have near-zero value and folded it into the plan critic's remit. Milestone 3's own summary claims to "complete the audit of criteria 1 through 7"; node 3.1 dropped criterion 1, and the plan never named the five directions, whose only source is `vision.md` — unreadable to the executor.
- Evidence: `milestone-3/step-1/review-1.md` finding `B2`, severity major. `milestone-3/steps.json` node 3.1 summary before the fix.
- Inference: a single-node graph still carries the coverage obligation the milestone summary asserts. Folding the review removed the only check that compares node coverage against the milestone's own claim.
- Impact: one plan round. Recovered because the plan critic prompt independently asked for milestone-fidelity verification.

### Subagent hard failure has no workflow handling

- Severity: medium
- Phase: review (`m3-s1`)
- Observation: two consecutive plan-critic dispatches terminated on `API Error: 529 Overloaded` without producing a verdict. A third succeeded after I narrowed its scope, handed it facts I had re-derived myself, and instructed it to finish "in well under 20 tool calls".
- Evidence: two `<task-notification>` entries with `status: failed`; `milestone-3/step-1/review-2.md`. The prior report records a related but distinct failure ("Subagent final-message delivery failed silently").
- Inference: a failed dispatch is neither a verdict nor a `BLOCK`, so verdict routing does not cover it. I had to decide unaided whether a retry counts against the cap. I chose that it does not, since no verdict was produced.
- Impact: two wasted dispatches. Also a correctness hazard: under repeated failure the tempting shortcut is to self-verify and proceed, which would silently drop the gate.

### `gh pr merge --delete-branch` collides with an active worktree — recurrence

- Severity: low
- Phase: merge
- Observation: `gh pr merge 59 --squash --delete-branch` and `gh pr merge 62 --squash --delete-branch` both reported `cannot delete branch … used by worktree`. The merges themselves succeeded.
- Evidence: transcript for PRs #59 and #62. Identical to the prior report's finding for PR #88.
- Impact: two confusing error lines. On #62 I had also removed the worktree in the same command sequence, which produced the error before the removal ran.

### PR mergeability races a force-push

- Severity: low
- Phase: merge (`m2-s3`)
- Observation: `gh pr merge 60 --squash --delete-branch` failed with `GraphQL: Pull Request is not mergeable`. An immediate `gh pr view 60 --json mergeable,mergeStateStatus` returned `MERGEABLE` / `CLEAN` at the same head SHA, and the merge then succeeded.
- Evidence: transcript for PR #60.
- Inference: GitHub had not finished recomputing mergeability after the force-push.
- Impact: one failed merge attempt. Risk is misdiagnosis: the error text invites looking for a conflict that does not exist.

## Recommendations

### Mark orchestrator-authored assertions and require the critic to verify them first

- Addresses: "Orchestrator-authored prompt content passes through no gate"
- Change: add a sixth item to the delegation contract: *any factual assertion the orchestrator inlines into a dispatch must be labelled as orchestrator-authored, and every critic prompt must list those assertions as its first verification target, with a false entry defined as CRITICAL.* Add the corollary: *never inline a claim broader than the exact test that produced it; state the test alongside the claim.*
- Location: `SKILL.md`, `## The delegation contract (every dispatch, no exceptions)`
- Trade-off: slightly longer critic prompts; critics spend effort re-deriving facts the orchestrator already checked.
- Confidence: high. This is a second-run recurrence, and it is what caught `B1` in `m3-s1`.

### Require sweep and audit instructions to state an invariant, not a cause

- Addresses: "No rule distinguishes cause-scoped from invariant-scoped instructions"
- Change: add to the delegation contract that a correction or audit dispatch covering more than one site must state the property that must hold at HEAD, never the cause of the defect, and must require per-item justification (quote the evidence for a positive item; search the whole artifact for a negative, completeness, or position item). A summary of what changed does not satisfy completion.
- Location: `SKILL.md`, `## The delegation contract`
- Trade-off: longer executor reports; per-item justification costs tokens proportional to artifact size.
- Confidence: high. Three identical failures, then convergence once the invariant was stated.

### Add a `WITHDRAW` disposition to verdict routing

- Addresses: "No disposition exists for withdrawing an unmaintainable artifact component", "The record was rewritten five times"
- Change: in `## Routing, caps, and adaptation`, add: *when two consecutive rounds fail on the same component of an otherwise-approved artifact, the orchestrator must present withdrawal of that component — delete it, keep verified corrections, add a disposition recording what was attempted, what failed, and what evidence the conclusion now rests on — as an explicit option alongside a further fix round.* Escalate the choice rather than deciding it.
- Location: `SKILL.md`, `## Routing, caps, and adaptation`
- Trade-off: risks withdrawing components that one more round would have fixed. Mitigated by requiring the human to choose.
- Confidence: high. Two occurrences in one run, same remedy both times, human-supplied both times.

### Define the executor-side escalation terminal state

- Addresses: "The escalation terminal state is undefined"
- Change: state in the executor policy that an executor escalation is non-terminal: leave the affected artifact region unchanged, complete every independent part of the step, commit, and list escalated items at the top of `pr-body.md` and in the final message. Stranding a step on one unresolved disposition is a defect.
- Location: `SKILL.md`, `## Phase 3 — Per step → PCE-PR-C`, executor policy
- Trade-off: a committed step contains known-open questions; the PR body must be read, not skimmed.
- Confidence: high. I improvised exactly this and it worked; a critic had flagged the gap.

### Do not fold graph critique into the plan critic, even for a single node

- Addresses: "Folding a single-node graph review dropped an acceptance criterion"
- Change: in `## Phase 2`, state that the step-graph critique is required regardless of node count, and that its minimum check is coverage: every obligation the milestone summary claims must be assigned to some node.
- Location: `SKILL.md`, `## Phase 2 — Per milestone (dependency order) → steps`
- Trade-off: one extra subagent call per single-node milestone. Cheap relative to the plan round it costs when skipped.
- Confidence: high.

### Handle failed dispatches explicitly in recovery

- Addresses: "Subagent hard failure has no workflow handling"
- Change: add to `## Conflict & recovery`: *a dispatch that terminates without a schema-valid verdict is not a verdict and does not consume a cap round. Retry once; on a second failure, reduce the dispatch's scope and hand it facts already verified, and record the reduction in `state.json`. Never substitute orchestrator self-verification for a gate that never ran.*
- Location: `SKILL.md`, `## Conflict & recovery`
- Trade-off: a narrowed retry is a weaker gate; the state entry makes that visible.
- Confidence: high on the cap rule and the prohibition; medium on scope reduction as the right remedy.

### Drop `--delete-branch` from the documented merge sequence

- Addresses: "`gh pr merge --delete-branch` collides with an active worktree — recurrence"
- Change: document the merge as `gh pr merge <n> --squash`, then `git worktree remove`, then `git branch -D`, in that order.
- Location: `SKILL.md`, `## Phase 3`, merge step
- Trade-off: none.
- Confidence: high. Second run reporting it; the prior report's recommendation was not applied.

### Re-check mergeability after a force-push before merging

- Addresses: "PR mergeability races a force-push"
- Change: after any force-push to a PR head, poll `gh pr view <n> --json mergeable,mergeStateStatus` until `MERGEABLE`/`CLEAN` before invoking merge, and treat a single `not mergeable` immediately after a force-push as a race rather than a conflict.
- Location: `SKILL.md`, `## Phase 3`, merge step
- Trade-off: one extra call per force-pushed PR.
- Confidence: medium. One observation, but the mechanism is well understood and the check is cheap.

### Experiment: record round histories as prose, not counts

- Addresses: general observability; supports "Cap exhaustion occurred exactly where the prior report predicted"
- Change: make `state.json.counters.<artifact>_history` an array of one-line round summaries the required form, rather than a bare integer.
- Location: `SKILL.md`, `## Conflict & recovery` (rehydration) and `## Artifacts`
- Trade-off: more orchestrator writing per round.
- Confidence: experimental, but I adopted it unprompted for both step graphs and it was the most useful part of `state.json` after compaction.

## No-change decisions

**The 3-round caps should stay as they are.** They fired correctly. `m2-s3` was genuinely
unconverging and the cap forced the escalation that ended it. The problem was not the cap
but the absence of a withdrawal route at the point it fired, which the `WITHDRAW`
recommendation addresses directly.

**The prohibition on the orchestrator authoring artifacts should stay.** It cost real
latency — the trailing-newline restoration and the signpost reflow each required a full
Codex dispatch for a whitespace change — but every orchestrator factual error in this run
(`m1-s5` premise, `m1-s12` stale offset, `m3-s1` double-hyphen) argues that my direct
writing needs gating, not less ceremony.

**Codex-authored `pr-body.md` files were useful here and should stay.** The prior report
questioned this as ceremony. In this run `m2-s3`'s PR body carried the five escalated claim
IDs under a heading that made them visible to the human, which was load-bearing for `E4`.

**The gitignored `planning/` directory and the resulting self-containment rule should
stay.** It forced every plan to inline what the executor needs and produced the run's most
valuable discipline. `state.json.notes[0]` records the constraint; no failure in this run
traces to it.

**I am not recommending a change for the verification record's growth to 333 verdict rows
against 135 claim rows.** It is arguably disproportionate, but the run provides no evidence
that the volume itself caused a defect, and the rows that did cause defects were removed by
the `WITHDRAW` remedy rather than by any volume limit.

## Suggested follow-up

- **Apply the prior report's `--delete-branch` recommendation.** It recurred unchanged, which suggests these reports are not currently feeding back into `SKILL.md`. Worth confirming that loop exists before writing a third report.
- **Separate experiment: whether critics can be asked to be exhaustive.** Both reports independently observe layered defect discovery consuming caps. Neither run tested whether a prompt demanding an exhaustive first pass changes the layering, or whether layering is intrinsic. This is the shared root of both caps findings and is worth one deliberate trial.
- **`E6` remains open for the human** and is not a workflow issue: `vision.md` Scope In item 1 asks the signpost to record a construction "the preceding decade did not have", while acceptance criterion 10 forbids absolute negative claims. The pipeline resolved it toward the criterion and flagged it. A workflow-level question worth considering separately is whether vision-internal contradictions should be detected at Phase 1 rather than discovered by an executor in the final step.
