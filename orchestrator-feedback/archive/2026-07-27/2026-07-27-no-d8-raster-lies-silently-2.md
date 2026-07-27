# PCE workflow feedback: no-d8-raster-lies-silently (resumed session)

- Date: `2026-07-27`
- Orchestrator: Claude Code, Opus 5, single resumed session
- Run: `planning/2026-07-26-no-d8-raster-lies-silently` in `CooperBigFoot/pourpoint`
- Outcome: completed (all five milestones merged to `main` at `88b51f7`)

This is the second report for this vision. The first,
`2026-07-27-no-d8-raster-lies-silently.md`, covers the originating session
(M1–M2 merged, paused with M3 planned). This report covers the **resumption**,
which delivered M3, M4 and M5. Findings that merely repeat the first report are
not restated; where this run produced new evidence for one of its findings, that
is marked **[confirms report 1]** and cited.

## Executive summary

The resumed session merged M3 (`fb24e5a`), M4 (`945e732`) and M5 (`88b51f7`)
across PRs #93–#99, ending at 26 test binaries / 769 passed / 0 failed / 12
ignored on `main`. Twelve verdict artifacts were produced; eight gates returned
APPROVE, three returned REVISE, one returned BLOCK.

Four findings dominate.

**The single highest-value catch and the single worst near-miss share one root
cause: a check that could not fail.** M5's approved plan contained a
verification `git grep` whose pattern spanned a hard-wrapped line, so it could
never match — its "must produce no match" assertion was a tautology that would
have passed had the executor skipped the edit entirely
(`milestone-M5/step-S1/review-2.json`, B1). Independently, my own gate-ladder
command piped `cargo` through `tail`, so the `&&` chain tested `tail`'s exit
status rather than cargo's. Neither the skill nor the verdict schema asks
whether an evidence-producing command is capable of reporting failure.

**The vision itself was false, and the workflow only discovered it three
milestones in.** `milestone-M4/review-s-1.json` returned `verdict: BLOCK`,
`root_cause: vision`, five blocking issues: vision Scope-In item 6 and
acceptance criterion 8 described a defect that never existed at any ref the
vision ran against. Every fact needed to determine this was available at the
original planning ref. Nothing in Phase 0 or Phase 1 asks whether the vision's
claims are true at the ref.

**Prose is where the defects landed, and prose is what the gates do not see.**
M4/S1's production diff was correct on the first executor pass; its `CHANGELOG`
entry was not, and PR review rejected it
(`milestone-M4/step-S1/review-pr-1.json`, one `critical`). M5 was prose only.
The acceptance-gate ladder compiles and tests code; it is structurally blind to
the artifact class that carried this run's defects.

**The improvised filesystem verdict channel from session 1 was needed again
immediately and is now twice-confirmed.** `pr-reviewer-93` emitted two idle
notifications with no verdict before I re-dispatched it as `pr-reviewer-93b`
with a write-to-disk deliverable, which succeeded
(`milestone-M3/step-S1/review-pr-1.json`). **[confirms report 1: "Subagent
final-message delivery failed silently"]**

## Evidence reviewed

- `planning/2026-07-26-no-d8-raster-lies-silently/state.json` — `resume_instructions`, `counters`, `deltas` D3–D4, `escalations` E1, `key_findings`, `final_verification`
- `vision.md` — `## Amendments` A1 and A1.1 (both authored during this session)
- `milestone-M3/step-S1/review-pr-1.json` — APPROVE, 0 blocking
- `milestone-M4/steps-r1.json`, `steps-r2.json`, `steps.json`
- `milestone-M4/review-s-1.json` — BLOCK / `root_cause: vision` / `self_sufficiency: FAIL` / 5 blocking
- `milestone-M4/review-s-2.json` — REVISE / PASS / 1 major
- `milestone-M4/review-s-3.json` — APPROVE / PASS / 0 blocking
- `milestone-M4/step-S1/review-1.json` — APPROVE / PASS / 0 blocking
- `milestone-M4/step-S1/review-pr-1.json` — REVISE / 1 critical (B1, CHANGELOG)
- `milestone-M4/step-S1/review-pr-2.json` — APPROVE / 0 blocking
- `milestone-M4/step-S2/review-1.json`, `review-pr-1.json` — both APPROVE / 0 blocking
- `milestone-M5/review-s-1.json` — REVISE / FAIL / 1 critical (section trailer)
- `milestone-M5/step-S1/review-1.json` — APPROVE / PASS / 0 blocking
- `milestone-M5/step-S1/review-2.json` — REVISE / PASS / 1 major (vacuous grep)
- `milestone-M5/step-S1/review-pr-1.json` — APPROVE / 0 blocking
- Merge commits `fb24e5a`, `945e732`, `88b51f7`; step squashes `ab1d71d`→`766640d`, `ee10087`→`8f4e71e`, `e0ee77a`→`9303a91`, `910e06e`→`ae1fc10`
- `codex exec` stdout token counts as printed by the tool for individual dispatches
- `CONTEXT.md` at `945e732` (binding glossary), `docs/decisions/` (four ADRs)

## What worked

### Cold resume from `state.json` alone

- Evidence: the session began by reading `state.json` (`phase: "3-M3-S1-ready-to-execute"`) plus `resume_instructions.next_action` and `exact_steps`, and resumed at Phase 3 step 2 for M3/S1 without re-planning. `planning_ref_by_milestone` supplied M3's correct base ref (`95603fa`) rather than the stale original planning ref. No merged work was redone.
- Effect: a full milestone's approved planning survived a session boundary intact. **[confirms report 1: "`state.json` as resume spine"]** — that finding was recorded from a session that wrote the spine; this is the first evidence of a session successfully *consuming* it.

### `root_cause: vision` routing to a hard stop

- Evidence: `milestone-M4/review-s-1.json` returned `root_cause: vision`; the skill's routing rule ("`vision` → **always escalate**") produced a full stop, an `escalations` entry E1 in `state.json` with the verified facts, and a human decision. No M4 work was attempted on the false premise.
- Effect: prevented an executor being handed a mandatory red phase it could not produce without fabricating one. The critic stated this explicitly: "S1 as written would ship a commit whose stated red phase the executor cannot produce."

### Verifying critic citations at the ref before acting on them

- Evidence: before escalating, I re-read `session.rs:715-717`, `d8_handle` (`session.rs:840-876`), `d8_aux_accessor.rs:390-425`, and `git log -S` for the short-circuit's introducing commit (`58a414f`, #53). All five of the critic's load-bearing claims held. Later, the same discipline applied to `engine.rs:741-749` and `session.rs:679-681` showed that **my own** amendment A1 was partly unachievable, producing A1.1.
- Effect: the escalation to the human carried verified facts rather than a relayed hypothesis; and an orchestrator-authored error was caught one round after it was introduced rather than propagating into M5's register retirement.

### PR review catching a defect the gate ladder cannot see

- Evidence: `milestone-M4/step-S1/review-pr-1.json` B1 — the `CHANGELOG` bullet used "terminal refinement", which `CONTEXT.md:86-91` defines as **engine** behavior, as the subject of an ordering claim covering the missing-declaration leg, which can only hold at the strategy boundary. All five acceptance gates were green at that commit.
- Effect: prevented a user-facing false claim shipping in a changelog. The production diff was correct; only the prose was wrong.

### Adversarial critics that execute rather than read

- Evidence: `milestone-M4/step-S2/review-1.json` recomputed the two-declaration extents from `cog.rs` tiepoint/pixel-scale arithmetic and traced a four-byte truncated header through `read_local_extent` before approving 28 matrix rows. `milestone-M4/review-s-2.json` read `d8_crs` and `EpsgCode` rather than assuming the `EPSG:3857` → `UnsupportedD8Crs` mapping.
- Effect: **[confirms report 1: "Adversarial critics that execute rather than reason"]** with two further instances.

## Friction and failures

### A verification command that cannot fail is accepted as evidence

- Severity: **high**
- Phase: step planning, execution, and orchestrator self-verification
- Observation: two independent instances in one session of a check that was structurally incapable of reporting failure. (1) M5's approved plan §8 contained `git grep -n 'These unresolved production risks are owned by milestone M5' -- crates/core/tests/fixtures/parity/README.md` followed by "both `git grep` commands must produce no match"; the sentence is hard-wrapped across `README.md:233-234` and `git grep` is line-based, so the pattern exits 1 before and after the edit. (2) My own gate ladder ran `cargo … 2>&1 | tail -40` inside an `&&` chain, so each link tested `tail`'s exit status, not cargo's.
- Evidence: `milestone-M5/step-S1/review-2.json` B1 for (1), reproduced by me at the ref (`git grep … 945e732 -- …` exits 1; `'These unresolved production risks'` matches `:233`). For (2), the first M3/S1 gate run reported `EXIT_0` for all five gates through a `tail` pipe; re-running with per-command exit capture was required to establish the result honestly.
- Inference: the workflow treats a command's *output* as evidence without requiring that the command be shown able to produce a failing output. Both instances share this shape; neither is a domain mistake.
- Impact: (1) was caught only because I had asked that critic to test the patterns; the plan had already been APPROVED once (`review-1.json`, 0 blocking) with the vacuous grep present. (2) produced one wasted gate run and, had I not noticed, would have let a red test suite read as green.

### The vision's own claims are never grounded at the ref

- Severity: **high**
- Phase: orientation / milestone planning
- Observation: vision Scope-In item 6 and acceptance criterion 8 described a defect that did not exist. `select_d8_raster_for_terminal` short-circuits an empty terminal at `session.rs:715-717` before any `d8_extent` read; `d8_handle` performs no I/O; and `d8_aux_accessor.rs:390-425` already asserted the correct outcome. All three facts were present at `9ec267cb`, the original planning ref, and the short-circuit dates to `58a414f` (#53), predating the vision.
- Evidence: `milestone-M4/review-s-1.json`, blocking issue `S1-RED-PHASE-ALREADY-GREEN`; `state.json.escalations[0].orchestrator_verification` records my independent confirmation of each fact.
- Inference: Phase 0 produces a repo contract (commands, conventions, version policy) and Phase 1 decomposes the vision, but neither step asks whether the vision's factual claims hold at the planning ref. The falsity surfaced only when a step-graph critic was forced to check constructibility of a red phase.
- Impact: three milestones merged before the defect surfaced. The cost was contained — M1–M3 were independently sound — but the discovery point is late by design, not by accident.

### An orchestrator-authored vision amendment is not gated

- Severity: **medium**
- Phase: escalation recovery
- Observation: after the human ruled on E1, I authored amendment A1 into `vision.md`. A1 stated that the missing-declaration leg reports `DegenerateTerminalPolygon` in `RequireD8`. That is unachievable: `engine.rs:741-749` short-circuits on `!has_d8_aux()` before the strategy is constructed, so that leg reports `MissingRequiredD8Aux` before and after the fix. A1.1 was required to correct A1.
- Evidence: `vision.md` `## Amendments` A1 and A1.1; `milestone-M4/review-s-2.json` B1 (`S1-MISSING-D8-LEG-UNOBSERVABLE-AT-MODE-LEVEL`); `state.json.escalations[0].follow_up_A1_1`.
- Inference: the skill gates Codex-authored artifacts and Claude critic verdicts, and permits the orchestrator to author `state.json` deltas and (by extension here) vision amendments. Those orchestrator-authored texts enter the planner prompt as authoritative but pass through no gate.
- Impact: one extra planner round (M4 graph round 3). Had the next critic not been pointed at the amendment, A1's false clause would have reached M5's register retirement — the exact artifact class this vision existed to keep truthful. **[confirms and extends report 1: "Orchestrator-authored state is not gated, and my error propagated into an artifact"]** — that finding concerned `state.json`; this extends the same failure to `vision.md`.

### Prose artifacts are gated only if the orchestrator improvises the instruction

- Severity: **medium**
- Phase: execution and review
- Observation: this run's two rejected PRs were both prose. M4/S1's code was accepted unchanged while its `CHANGELOG` bullet was `critical`-rejected; M5 touched no code at all. In both cases the defect was a claim whose scope exceeded what the code supports, detectable only against `CONTEXT.md`. The PR reviewers found them because I wrote glossary-checking instructions into those prompts by hand; nothing in the skill requires it.
- Evidence: `milestone-M4/step-S1/review-pr-1.json` B1 cites `CONTEXT.md:86-91` as binding; `milestone-M5/review-s-1.json` B1 caught a section trailer ("These remain current issues") that would have survived directly above its own retirement.
- Inference: the acceptance-gate ladder is a compile-and-test ladder. For a docs-only step it is nearly vacuous — M5's five gates were green before and after the edit, at 769 passed both times.
- Impact: two defects caught, but by improvised prompt content rather than by a workflow rule. A future orchestrator omitting that instruction would ship both.

### The 3-round cap continues to bind at the natural convergence point

- Severity: **medium**
- Phase: planning
- Observation: M4's step graph consumed exactly 3 rounds (BLOCK → REVISE → APPROVE) and converged on the last permitted attempt. `state.json.counters.step_graph_rounds` for this vision now reads `{M1: 3, M2: 2, M3: 3, M4: 3, M5: 2}`.
- Evidence: `milestone-M4/review-s-{1,2,3}.json`; `counters` in `state.json`.
- Inference: three of five step graphs used the full cap. The first report recorded four artifacts at the cap; this session adds a fifth and a sixth (M4 graph; M5/S1 plan also took three authoring rounds). **[confirms report 1: "Four artifacts consumed exactly the 3-round cap"]**
- Impact: no cap exhaustion occurred, so no work was lost. The risk is that the margin between "converged" and "escalated to the human" is zero for hard artifacts.

### Milestone PR bodies remain expensive for single-step milestones

- Severity: **low**
- Phase: merge
- Observation: M5 merged one squashed commit touching two documentation files. Its milestone PR body was authored by a dedicated `codex exec` dispatch reporting 41,122 tokens. M4's reported 57,792; M3's 44,774.
- Evidence: `codex exec` stdout token lines for the three milestone-PR-body dispatches; `milestone-M5/milestone-pr-body.md`.
- Inference: for a milestone whose content is one step PR already carrying a reviewed body, the milestone body is largely a restatement.
- Impact: token cost with no observed defect caught at this stage in this session. **[confirms report 1: "Ceremony with little observed return: Codex-authored milestone PR bodies"]**

### `state.json` counters drifted from reality

- Severity: **low**
- Phase: state persistence
- Observation: `counters.plan_rounds` records `M3.S1: 3`, `M4.S1: 1`, `M4.S2: 1` but has no entry for `M5.S1`, which took three authoring rounds (initial → D4 write-set expansion → vacuous-grep fix) and two critic reviews.
- Evidence: `counters` in `state.json` versus `milestone-M5/step-S1/{plan.md, review-1.json, review-2.json}`.
- Inference: counters are updated by the orchestrator by hand at points of its choosing; there is no rule tying a counter increment to a dispatch.
- Impact: none in this run — the cap was tracked from the artifacts themselves. The risk is that a resumed session reading `counters` alone would under-count rounds already spent and could exceed a cap unknowingly.

## Recommendations

### Require evidence-producing commands to be shown falsifiable

- Addresses: "A verification command that cannot fail is accepted as evidence"
- Change: add one clause to the plan-critic `self_sufficiency` definition and to the executor policies: *any command whose output is offered as proof that a change landed must be demonstrated to produce a failing result against the base ref before the change; a check that cannot fail is not evidence.* For shell pipelines specifically, require exit status to be captured from the command rather than from the last element of a pipe.
- Location: `pce/SKILL.md`, the `self_sufficiency` operational definition (items (a)–(c), add (d)); and the "Execute (Codex)" executor-policies list.
- Trade-off: adds one verification step per evidence command; slightly longer plans.
- Confidence: **high** — two independent instances in one session, one of which passed an APPROVE gate.

### Add a vision-grounding pass before milestone decomposition

- Addresses: "The vision's own claims are never grounded at the ref"
- Change: require the Phase 1 milestone-planner (or its critic) to check each vision acceptance criterion against the planning ref and classify it as *holds*, *already satisfied*, or *not reproducible at this ref*, emitting that classification as an artifact. Any criterion in the third class escalates before decomposition.
- Location: `pce/SKILL.md`, "Phase 1 — Vision → milestones", planner prompt and critic dispatch.
- Trade-off: one additional read-heavy pass at the most expensive point to add one; may produce false alarms for criteria that are genuinely forward-looking.
- Confidence: **medium** — it would have caught this run's `root_cause: vision` defect at Phase 1 instead of M4, but a single instance does not establish the general rate.

### Gate orchestrator-authored vision amendments like artifacts

- Addresses: "An orchestrator-authored vision amendment is not gated"
- Change: extend the existing rule that orchestrator-authored `state.json` facts should be gated so that it explicitly covers any orchestrator-authored text that will be inlined into a planner or executor prompt — including `vision.md` amendments recorded after an escalation. The next critic dispatch after such an amendment must be told the amendment is orchestrator-authored and unverified, and asked to check it at the ref.
- Location: `pce/SKILL.md`, "Active adaptation" under "Routing, caps, and adaptation".
- Trade-off: one explicit instruction per amendment; no additional dispatch.
- Confidence: **high** — A1 was false and reached a planner prompt.

### Require a glossary check when a step touches user-facing prose

- Addresses: "Prose artifacts are gated only if the orchestrator improvises the instruction"
- Change: add to the PR-reviewer dispatch contract: *when the diff touches a changelog, release document, README, or other user-facing prose, check every domain term used in a claim against the repository's glossary (`CONTEXT.md` where present) and reject a claim whose scope exceeds what the code supports.*
- Location: `pce/SKILL.md`, "Phase 3 → 5. Review".
- Trade-off: lengthens reviewer prompts for code-only steps unless made conditional.
- Confidence: **high** — this exact check produced the only `critical` finding against an otherwise-correct commit in this session.

### Tie counter increments to dispatches

- Addresses: "`state.json` counters drifted from reality"
- Change: state that the round counter for an artifact must be incremented in the same `state.json` write that records the dispatch, not retrospectively.
- Location: `pce/SKILL.md`, "Runtime expectation" (persistence bullet).
- Trade-off: none material.
- Confidence: **medium** — no failure occurred here; the finding is a latent resume hazard.

### Experiment: per-artifact-class round caps

- Addresses: "The 3-round cap continues to bind at the natural convergence point"
- Change: try a cap of 4 for graph artifacts (milestone and step graphs) while holding plans and PR-review loops at 3, and record where convergence actually falls.
- Location: `pce/SKILL.md`, "Caps".
- Trade-off: raises the ceiling on wasted rounds before human escalation.
- Confidence: **experimental** — six artifacts across two sessions have landed at the cap without exceeding it, which is equally consistent with a well-calibrated cap and a barely-sufficient one.

## No-change decisions

- **Cold re-dispatch on every REVISE (never `resume`).** Used for all planner revisions this session with no observed problem; report 1 already records it as working.
- **Worktree isolation and the writable-`.git` sandbox root.** Four step executions, no sandbox failures, no cross-step interference.
- **Two-tier merge (squash to milestone, merge-commit to `main`).** Verified two-parent merges at `fb24e5a`, `945e732`, `88b51f7`; step squashes and their history remained reachable.
- **The delegation contract's five elements.** No subagent in this session reported missing context, and no verdict was invalidated by a checkout/ref mismatch.
- **Repo-contract override of skill defaults** (no tags, `--exclude pourpoint-python`). Held for the whole session; the forbidden-command list prevented any bare `cargo test --workspace`.
- **The `docs/adr/` vs `docs/decisions/` path mismatch** encountered while landing the ticket is a `/land-ticket` issue, not a PCE one, and is out of scope for this report.

## Suggested follow-up

- **The PCE → `/land-ticket` handoff discards PCE's richest output.** `state.json` accumulated `key_findings`, `deltas` and `escalations` containing ref-verified facts (for example that known issue 3 never existed). `/land-ticket` forbids reading PCE state and requires reconstruction from GitHub, committed source, and human answers, so I re-derived several of those facts from source during the landing. Worth deciding deliberately whether that isolation is intended; it is defensible as a trust boundary, but it is currently an implicit consequence rather than a stated one.
- **The two undefined risk tokens** `risk-degenerate-path-corner` and `risk-dead-integrity-arm` appear in `crates/core/tests/fixtures/parity/README.md` with no definition anywhere in the repository, and survived a retirement pass in M5. Not a workflow finding; recorded so it is not lost.
