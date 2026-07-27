# PCE workflow feedback: no-d8-raster-lies-silently

- Date: `2026-07-27`
- Orchestrator: Claude Code, Opus 5, single session (run spanned 2026-07-26 21:40 → 2026-07-27 00:35 local)
- Run: `planning/2026-07-26-no-d8-raster-lies-silently` in `CooperBigFoot/pourpoint`
- Outcome: paused (2 of 5 milestones merged; M3 plan approved and ready to execute; stopped on orchestrator context budget, then resumed once on explicit user instruction, then paused again)

## Executive summary

The workflow delivered M1 (`03ecd1c`, PRs #88/#89) and M2 (`95603fa`, PRs #90/#91/#92) and left M3 with an approved step graph and an approved plan. Seventeen defects were caught by gates before merge; none escaped into a merged artifact that later needed reverting.

Three findings dominate.

**The gates that worked are the ones that executed code, not the ones that read text.** Every high-severity catch in this run came from a critic running a command, rebuilding a tree, or evaluating arithmetic from source — not from reading a plan carefully. The workflow does not currently distinguish these two modes of review.

**Subagent final-message delivery failed completely and silently.** Both Phase 0 Explore agents went idle without delivering their reports (`orient-gates`, `orient-release`). I improvised a filesystem delivery channel and used it for all fourteen subsequent subagent dispatches. This is the single largest undocumented adaptation in the run and it affected every gate.

**The workflow has no model for cross-milestone invalidation.** A later milestone can invalidate an earlier merged proof. This happened twice (M2/S2 broke an M1 test; M3 would break another) and both times it was caught only because I added an ad-hoc "what earlier proofs does this invalidate?" instruction to the critic prompt after the first occurrence. Nothing in the skill requires that question.

## Evidence reviewed

- `planning/2026-07-26-no-d8-raster-lies-silently/state.json` — final state, `deltas` D1–D2, four `key_findings` entries, `resume_instructions`
- `milestones.json`, `milestones-r1.json`, `review-m-1.json`, `review-m-2.json`
- `milestone-M1/steps.json`, `steps-r1.json`, `steps-r2.json`, `review-s-{1,2,3}.json`
- `milestone-M1/step-S1/plan.md`, `plan-r1.md`, `review-{1,2}.json`, `review-pr-1.json`
- `milestone-M2/steps.json`, `steps-r1.json`, `review-s-{1,2}.json`
- `milestone-M2/step-S1/plan.md`, `review-1.json`, `review-pr-1.json`
- `milestone-M2/step-S2/plan.md`, `plan-r1.md`, `plan-r2.md`, `review-{1,2}.json`, `review-pr-1.json`
- `milestone-M3/steps.json`, `steps-r1.json`, `steps-r2.json`, `review-s-{1,2}.json`
- `milestone-M3/step-S1/plan.md`, `plan-r1.md`, `plan-r2.md`, `review-{1,2,3}.json`
- Git history: `9ec267c` (base) → `03ecd1c` → `95603fa` → `37a9bc8`; step commits `75dd09a`, `491a3cc`, `1652d56`; squash commits `cd5aec9`, `594089f`, `55a41c8`
- PRs #88–#92
- Session transcript for dispatch prompts, subagent notifications, and gate output

## What worked

### Adversarial critics that execute rather than reason

- Evidence:
  - `milestone-M2/step-S2/review-2.json` — the critic ran the plan's `gdal_edit.py -a_nodata -1` recipe against the committed fixture. It exits 255 (COG layout protection) and leaves the file byte-identical (reported same sha256 `fece90b5…`, still `NoData Value=-128`). It then verified `gdal_translate -a_nodata -1 -of COG -co COMPRESS=DEFLATE` as a working replacement and reported the resulting metadata.
  - `milestone-M2/step-S1/review-pr-1.json` — the PR reviewer rebuilt the pre-fix source with `git archive` of the base ref, added only the test seam, and reproduced the panic at `trace.rs:50:28` with index `18446744073709551615`. It also reordered the two assertions in its scratch copy to verify the geometric half separately, because pre-fix the `cell_count` assertion fires before `unsigned_area` and a naive red run never reaches the area assertion.
  - `milestone-M1/review-s-3.json` — reimplemented `trace_upstream` and `decode` against the committed fixture bytes and measured GRASS = 375 / ESRI = 1 unmasked, rather than accepting the planner's figures.
  - `milestone-M3/step-S1/review-{2,3}.json` — evaluated Equal Earth closed-form from `crates/core/src/algo/projection.rs` to establish that the projected fixture ring occupies rows `[0.4984, 0.5016]`, which is what proved the round-2 fix incomplete and the round-3 fix correct.
- Effect: four defects that plan-reading alone would not have surfaced. The `gdal_edit.py` case is the clearest: the command is the obvious tool for the job, reads as correct, and fails only because of this specific fixture's COG layout — while leaving a valid readable `.tif` behind, so the downstream failure would have been maximally confusing.

### The `self_sufficiency` assertion-blast-radius check (item (c))

- Evidence: `milestone-M1/step-S1/review-1.json`, finding `S1-DONUT-GRASS-BREAKS-HOLE-ASSERTION` (critical). `DonutRasterSource` (`d8_aux_accessor.rs:734`) hard-codes ESRI over bytes `[0,16,16, 4,0,64, 1,1,64]`, but its only caller builds its manifest via `write_projected_manifest`, which declares `"flow_dir_encoding": "grass"`. Under declaration authority the double receives GRASS, `from_grass` rejects 16 and 64, the trace collapses, and `assert_eq!(interiors.len(), 1)` fails — while the plan simultaneously instructed "Do not alter the byte arrays, geotransforms, request capture, or assertions."
- Effect: prevented a red gate with no in-plan remedy — the worst executor failure shape, because the plan explicitly forbade the only available fix. Item (c) is the check that found it; items (a) and (b) would not have.

### Cold re-dispatch on every REVISE (never `resume`)

- Evidence: every revision round produced a diff confined to the requested change. Verified mechanically several times: `milestone-M1/steps.json` round 3 vs round 2 (`files_touched` byte-identical, only two summary sentences changed); `milestone-M2/steps.json` round 2 (`files_touched` identical on both nodes, only two summary strings); `milestone-M3/steps.json` round 3 (two partition sentences).
- Effect: no revision round introduced collateral drift. The "surgical revision" framing in the re-dispatch prompt plus cold context appears sufficient to prevent planners from "improving" approved material.

### `state.json` as resume spine, and the delta mechanism

- Evidence: `deltas` D1 records my own Phase 0 error (I asserted acceptance criterion 4's grep should return zero non-test `FlowDirEncoding::Esri` hits; `flow_dir.rs:169`/`:180` must survive). D2 carried a verified correction — that the nodata-1 tile cannot produce a differential because it always panics — from milestone planning into the M2 step planner's prompt across roughly an hour of intervening work.
- Effect: D2 in particular prevented the M2 planner from re-deriving a construction that had already been proven impossible.

### Repo contract overriding skill defaults

- Evidence: `state.json.repo_contracts.pourpoint.version_bump.orchestrator_ruling`. `RELEASING.md:14-15` forbids per-commit version bumps and agent tagging. The skill's executor prompt mandates the opposite ("apply the plan's version bump and create exactly ONE conventional commit with the bump folded in"; merge step: "tag `v<version>` yourself"). Every executor dispatch carried an explicit override clause.
- Effect: no version file was touched and no tag was cut across three step commits — verified per commit (`git diff --name-only` filtered for `Cargo.toml|pyproject.toml|uv.lock`; `git tag --points-at HEAD`).

## Friction and failures

### Subagent final-message delivery failed silently; filesystem delivery was improvised

- Severity: high
- Phase: orientation, and thereafter every gate
- Observation: `orient-gates` and `orient-release` both emitted `idle_notification` with `idleReason: "available"` and no result message. A `SendMessage` asking each to re-send its JSON also produced no result. Only after I instructed both to *write the JSON to a file path* and reply `DONE` did the contracts arrive (`orient-gates.json`, `orient-release.json`). I then used filesystem delivery for all fourteen subsequent subagent dispatches.
- Evidence: transcript — three `idle_notification` messages from `orient-release` and two from `orient-gates` before the disk-write instruction; `ls` of the vision directory showing `orient-gates.json` appearing only after that instruction.
- Inference: the failure is in the agent→orchestrator result channel, not in the agents (they had completed their work and could write files). I cannot determine from the run whether this is environment-specific.
- Impact: roughly six wasted round-trips at the start of the run, and a workflow-wide adaptation invented mid-run. Every critic prompt in this run carries a bespoke "write your verdict as a single JSON object to `<path>`" block that the skill does not specify. If a future orchestrator does not improvise this, Phase 0 stalls.

### Cross-milestone invalidation has no place in the workflow model

- Severity: high
- Phase: step planning (M2), step planning (M3)
- Observation: M2/S2's new rejection rule broke `projected_grass_declaration_drives_gdal_and_changes_geometry`, a test merged in M1 — because the fixture M1 chose declares Int8 nodata `-128`, which reinterprets to byte 128, and `from_esri(128) = Ok(Some(Northeast))`, a legal direction. Separately, M3 would break `selected_d8_read_failure_hard_errors_under_best_effort_and_require_d8`, whose *name* encodes the contract M3 inverts.
- Evidence: `milestone-M2/step-S2/review-1.json` finding `S2-B1` (critical); `milestone-M3/review-s-1.json` finding `B1` (major); `state.json.key_findings.M1_test_broken_by_M2_S2`.
- Inference: the workflow's review scope is per-artifact and per-step. Nothing directs any gate to ask what *already-merged* evidence a new rule invalidates. Both catches happened because I added that question to the critic prompt by hand — for M2 as "THE PRIMARY QUESTION — does S2 destroy S1's proof?" and for M3 as "THE END-STATE QUESTION — a prior milestone was defective on exactly this."
- Impact: without the ad-hoc instruction, M2 would have merged with its bounds fix unprovable (see next finding) and M3 would have handed an executor a red test whose name argued the executor was wrong.

### A milestone can end with its own evidence unconstructible, and no gate asks

- Severity: high
- Phase: step planning (M2)
- Observation: M2's first step graph split bounds-preservation (S1) from sentinel-rejection (S2). Because `decode` (`flow_direction_tile.rs:181-187`) compares against the hard-coded `const NODATA: u8 = 255` and never reads `self.inner.nodata()`, and `FlowDirectionTile::new` hard-codes 255, `from_raw` is the only route to a directional-nodata tile. S2 rejects exactly that. After S2, sentinel-returning and absence-returning `get_checked` are observationally identical through the entire public API — so S1's differential was not merely deleted but *unconstructible*.
- Evidence: `milestone-M2/review-s-1.json` finding `M2-BOUNDS-PROOF-EVAPORATES` (critical). The critic also evaluated and rejected both obvious remedies: reversing the steps is strictly worse (the panic could never be demonstrated), and merging them does not help (the loss is a property of the milestone's end state, not the commit boundary). Resolution was a `#[cfg(test)] pub(crate) fn from_raw_unchecked` seam.
- Inference: each step was individually correct and individually green. The defect exists only in the composition.
- Impact: had this merged, a future refactor could have reverted `get_checked` to sentinel-returning with the full suite green.

### The milestone graph goes stale as milestones land, and the workflow treats it as fixed

- Severity: medium
- Phase: step planning (M3)
- Observation: `milestones.json` node M3 was authored during Phase 1, before M2 existed. It therefore does not mention `FlowDirectionTileError::DirectionalNodata` — an error M3 must classify, because vision Scope-In item 4 requires "every malformed-input rejection this vision introduces" to become a typed skip reason. I had to instruct the M3 planner in prose: "**Its summary is STALE in one specific way** … ENUMERATE THE ACTUAL ERROR SURFACE AT 95603fa yourself."
- Evidence: `milestones.json` node M3 summary (no mention of directional nodata); my M3 step-planner prompt; `state.json.resume_instructions.M3_watch_items[0]`.
- Inference: Phase 1 produces the milestone graph once and Phase 2 consumes it as spec. There is no re-validation step when a milestone's ground truth advances past its authoring ref.
- Impact: a planner that trusted its own node summary would have under-enumerated the error surface and failed acceptance criterion 6, which is absolute ("**No** condition on the D8 refinement path returns `Err` … under `RefinementMode::BestEffort`").

### The planning ref is singular in the skill but advances per milestone in practice

- Severity: medium
- Phase: all phases after M1 merged
- Observation: the skill's dispatch templates use one `<planning-ref>`. After M1 merged, reading M2's inputs at the original ref would have been actively wrong — M1 changed the `RasterSource` trait signature and every reader. I tracked `state.json.planning_ref_by_milestone` and put "DO NOT read at an earlier ref" plus a summary of what changed into every subsequent planner and critic prompt.
- Evidence: `state.json.planning_ref_by_milestone` (M1 `9ec267c`, M2 `03ecd1c`, M3 `95603fa`); the "Do NOT read at an earlier ref" clause in the M2, M3 planner and all M2/M3 critic prompts.
- Impact: no incorrect-ref failure occurred, but the correction was manual and repeated in roughly ten prompts.

### Orchestrator-authored state is not gated, and my error propagated into an artifact

- Severity: medium
- Phase: orientation → milestone planning
- Observation: I recorded in `state.json.key_layout_facts` that acceptance criterion 4's grep "should return NO workspace hits outside tests," reasoning from the fact that `FlowDirEncoding` is an external `hfx` type. That is wrong: `flow_dir.rs:169`/`:180` are the `from_encoded`/`to_encoded` dispatch arms and must survive. The error reached the milestone-planner prompt and then M1's evidence clause.
- Evidence: `review-m-1.json` finding `M1-GREP-UNSATISFIABLE`; `state.json.deltas[0]` (D1), which records "The error was mine and had propagated into the milestone-planner prompt."
- Inference: the workflow gates every Codex artifact and every Claude verdict, but `state.json` — which the orchestrator authors and every downstream prompt consumes — passes through no gate.
- Impact: one wasted planning round. Caught only because the milestone critic read the source rather than the state file.

### A critic's own verified number was internally inconsistent and was consumed downstream

- Severity: medium
- Phase: step planning (M1)
- Observation: `milestone-M1/review-s-1.json` reported GRASS = 375 cells from a hand-reimplementation of the trace, and in the same note observed that "375 matches the golden's 374 masked cells." Those two statements are inconsistent. The planner consumed 375 and pinned it as a test assertion. The next round's critic measured the real value by running the existing `projected_grass_capture_child` test, got `374.0000000000013`, and blocked it.
- Evidence: `milestone-M1/review-s-1.json` non-blocking notes; `milestone-M1/review-s-2.json` finding `S1-PINNED-CELL-COUNT-IS-WRONG`; the golden's own `carve_measurement.derived_carved_cell_count = 374`.
- Inference: the difference is masked vs unmasked trace — `refine_terminal_from_source` applies `flow_dir.apply_mask` before tracing. The round-1 critic's simulation omitted the mask and it noticed the discrepancy without resolving it.
- Impact: one wasted round. More importantly, it shows non-blocking notes are consumed as authoritative by the next planner while being held to a lower evidentiary bar than blocking issues.

### Partial fixes read as complete

- Severity: medium
- Phase: step planning (M3)
- Observation: `milestone-M3/step-S1/review-1.json` finding `S1-2` (critical) said two engine-level tests assert unreachable reasons because the projected fixture sits at lon/lat ≈ (10,10) while the committed terminal is at lon 0..5 / lat −5..0. The round-2 plan supplied new tiepoint values that genuinely fixed *selection*. The round-2 critic verified that half, then continued and found the test still unreachable for a second reason: rasterization is pixel-center based, and the double's lone scanline at 0.5 sits below the projected terminal's row range `[0.5064, 0.5096]`, so the run returns `EmptyRasterMask` rather than `InverseProjection`.
- Evidence: `milestone-M3/step-S1/review-1.json` and `review-2.json`; `state.json.key_findings.M3_S1_2_partial_fix_trap`.
- Inference: the confirmed-correct half made the whole repair look complete. Nothing in the workflow directs a confirmation critic to keep going past the first verified fix.
- Impact: one additional round, consuming the last slot in the 3-round cap. Had the round-2 critic stopped at the confirmed half, a test named `inverse_projection_skips_best_effort_and_stays_fatal_when_required` would have merged while proving availability routing instead.

### Four artifacts consumed exactly the 3-round cap

- Severity: medium
- Phase: milestone planning, step planning, plan authoring
- Observation: `milestone-M1/steps.json` converged at round 3; `milestone-M2/step-S2/plan.md` at round 3; `milestone-M3/steps.json` at round 3; `milestone-M3/step-S1/plan.md` at round 3. None exhausted the cap, but four of nine gated artifacts finished in the final permitted round.
- Evidence: `state.json.counters.step_graph_rounds` (`M1: 3`, `M2: 2`, `M3: 3`); `counters.plan_rounds.M3.S1 = 3`; the `review-3.json` files for M1 steps and M3/S1.
- Inference: critics generally surfaced defects one or two at a time rather than exhaustively, so each round revealed a new layer. `milestone-M3/step-S1/review-1.json` is the exception (four findings at once). I cannot determine from the run whether prompting style or genuine defect-layering drives this.
- Impact: no escalation occurred, but the margin was thin on four artifacts. A fifth round would have been needed had any round-3 critic found a real defect.

### The red phase is required by the vision and by user policy, but not by the workflow

- Severity: medium
- Phase: execution
- Observation: vision acceptance criterion 2 requires "A regression test proves the panic on current code before the fix and passes after it." The user's global instructions require the same for all bug fixes. The PCE skill's executor prompt does not mention a red phase. I added it by hand to the M2/S1 executor dispatch ("The plan requires a RED-PHASE demonstration… record what you observed in pr-body.md") and the M3/S1 plan carries it as §4.1.
- Evidence: my M2/S1 executor dispatch text; `milestone-M2/step-S1/pr-body.md`, which records "RED: the nodata-1 column-zero regression panicked in the real trace at `trace.rs:50` after converting the negative column to `18446744073709551615`"; `milestone-M3/step-S1/plan.md` §4.1.
- Impact: without the manual addition, the PR reviewer would have had no way to distinguish a test that genuinely failed pre-fix from one that never could.

### `gh pr merge --delete-branch` conflicts with an active worktree

- Severity: low
- Phase: merge
- Observation: `gh pr merge 88 --squash --delete-branch` reported `failed to delete local branch pce/no-d8-raster-lies-silently/m1-s1: … cannot delete branch … used by worktree`. The merge itself succeeded.
- Evidence: transcript of the PR #88 merge. For #90 and #91 I dropped `--delete-branch` and removed the worktree first.
- Impact: one confusing error line. The skill's step-6 ordering ("squash-merge … Remove the worktree, delete the branch") is correct if followed literally; the `--delete-branch` convenience flag inverts it.

### Ceremony with little observed return: Codex-authored milestone PR bodies

- Severity: low
- Phase: merge
- Observation: the prime directive requires Codex to author all PR bodies. For M1 — a single-step milestone — the milestone PR body restated the step PR body. It cost one `codex exec` dispatch.
- Evidence: `milestone-M1/pr-body.md` vs `milestone-M1/step-S1/pr-body.md`.
- Inference: for multi-step milestones (M2) the milestone body did add synthesis across steps. The redundancy appears specific to single-step milestones.
- Impact: one avoidable dispatch per single-step milestone.

## Recommendations

### Specify a durable subagent result channel

- Addresses: "Subagent final-message delivery failed silently"
- Change: state in the skill that every Claude critic/reviewer dispatch must instruct the agent to write its verdict JSON to a named path under `VISION_DIR` and reply only with a short acknowledgement, and that the orchestrator reads the file rather than the agent's final message. Add the verdict path to the Artifacts tree (e.g. `review-<n>.json` alongside `review-<n>.md`).
- Location: PCE skill, "The delegation contract" (output-format clause) and "Artifacts"
- Trade-off: one extra file write per gate; verdicts become durable audit artifacts, which this run relied on when re-reading earlier rounds.
- Confidence: high — the run could not have completed Phase 0 without this adaptation.

### Add an end-state question to every step-graph and milestone-graph critic

- Addresses: "Cross-milestone invalidation", "A milestone can end with its own evidence unconstructible"
- Change: add a required review item: *"After every node in this graph merges, does each permanent proof from an earlier merged milestone still hold and still compile? Does any rule this graph introduces make an earlier proof unconstructible rather than merely failing?"* Require the critic to name each prior proof it checked.
- Location: PCE skill, Phase 2 (step-graph critic duties) and Phase 1 (milestone-graph critic duties)
- Trade-off: lengthens critic prompts and adds work proportional to the number of merged milestones.
- Confidence: high — this exact question, added by hand, produced the two highest-severity findings in the run.

### Re-ground each milestone's node summary at its own base ref before step planning

- Addresses: "The milestone graph goes stale as milestones land"
- Change: in Phase 2, direct the step-planner prompt to treat the milestone node summary as intent and the base ref as fact, with an explicit instruction to enumerate the current surface at that ref where the two could differ. Record the per-milestone base ref in `state.json` as a first-class field.
- Location: PCE skill, Phase 2 dispatch template; repo-contract/state shape
- Trade-off: none identified; this is what I did manually and it cost only prompt text.
- Confidence: high

### Make the planning ref per-milestone in the skill's own templates

- Addresses: "The planning ref is singular in the skill but advances per milestone"
- Change: replace the single `<planning-ref>` placeholder with `<base-ref for this milestone>` in the Phase 2 and Phase 3 dispatch templates, and add a state field `planning_ref_by_milestone`.
- Location: PCE skill, Phase 2 and Phase 3 templates; "Startup" state initialization
- Trade-off: none identified.
- Confidence: high

### Gate orchestrator-authored `state.json` facts the same way artifacts are gated

- Addresses: "Orchestrator-authored state is not gated"
- Change: require that any orchestrator-authored *claim about the code* placed in `state.json` (as opposed to recorded commands, refs, or verdict history) cite the ref and the command that established it, and mark it as unverified until a critic confirms it. Alternatively, restrict `state.json` to contract/command/history data and move code claims into the artifacts that critics already gate.
- Location: PCE skill, "Phase 0 — Orientation → repo contract" and "Active adaptation"
- Trade-off: slightly more bookkeeping in Phase 0; the run shows the alternative is orchestrator errors propagating into planner prompts.
- Confidence: medium — one occurrence, but it reached a merged artifact's evidence clause before being caught.

### Require confirmation critics to continue past the first verified fix

- Addresses: "Partial fixes read as complete"
- Change: add to the narrow-confirmation review pattern: *"Confirming one part of a fix does not end the review. Verify the finding is resolved end-to-end, and state explicitly which parts you verified and which you did not."*
- Location: PCE skill, "Routing, caps, and adaptation" (REVISE handling)
- Trade-off: confirmation rounds get longer.
- Confidence: medium — one clear occurrence, with a well-evidenced near-miss.

### Hold non-blocking notes to the same evidentiary standard as blocking issues, or mark them unverified

- Addresses: "A critic's own verified number was internally inconsistent and was consumed downstream"
- Change: instruct critics that any *number, constant, or code fact* stated in `non_blocking_notes` must carry the command or derivation that produced it, and that internally inconsistent notes must be resolved before the verdict is written. Instruct re-dispatched planners to treat non-blocking notes as leads to confirm, not as facts.
- Location: PCE skill, "Verdict schema" semantics
- Trade-off: minor; critics already do this for blocking issues.
- Confidence: medium

### Add the red phase to the executor policy when the plan asserts a regression proof

- Addresses: "The red phase is required by the vision and by user policy, but not by the workflow"
- Change: add to the executor's binding policies: if the plan asserts that a new test proves a pre-existing defect, the executor must demonstrate the failure before implementing and record the observed failure in `pr-body.md`. Add a corresponding PR-reviewer check that the recorded failure is genuine.
- Location: PCE skill, Phase 3 step 3 (executor policies) and step 5 (PR reviewer inputs)
- Trade-off: one extra test run per regression-proving step.
- Confidence: high — the PR reviewer for #90 independently reproduced the red phase and confirmed it, which is only possible because the executor recorded it.

### Drop `--delete-branch` from the documented merge command

- Addresses: "`gh pr merge --delete-branch` conflicts with an active worktree"
- Change: document the merge as `gh pr merge <n> --squash`, then remove the worktree, then delete the branch — matching the skill's existing step-6 prose.
- Location: PCE skill, Phase 3 step 6
- Trade-off: none.
- Confidence: high

### Experiment: allow the milestone PR body to reuse the step PR body for single-step milestones

- Addresses: "Ceremony with little observed return"
- Change: permit the orchestrator to reuse a single step's `pr-body.md` as the milestone PR body when the milestone has exactly one step, instead of dispatching Codex again.
- Location: PCE skill, Phase 3 (milestone PR)
- Trade-off: loses a synthesis pass; the run shows synthesis only added value for the multi-step milestone.
- Confidence: experimental

## No-change decisions

- **The 3-round cap.** Four artifacts converged in round 3, which is thin margin, but none exhausted the cap and no escalation was needed. The evidence does not show the cap caused a failure. Raising it may simply extend rounds that better critic prompting would collapse. Keep as is and re-examine if a future run escalates on cap exhaustion.
- **One atomic step for large trait changes.** M1/S1 was a single 14-file step and M3/S1 is a single 10-file step. Both were challenged by critics and both were upheld as required for a green tree (`milestone-M1/review-s-1.json`, `milestone-M3/review-s-1.json`). The "prefer fewest steps" guidance worked.
- **Codex authors all artifacts; Claude only gates.** No occasion arose where this cost anything measurable, and it kept authorship and review genuinely independent. The one place I authored content — `state.json` facts — is exactly where an ungated error occurred, which supports the rule rather than undermining it.
- **Orchestrator owns all git and network operations.** No sandbox denial or push failure occurred. The parent-`.git`-writable-root configuration worked for all three step commits without incident.
- **zsh `<ref>:crates/...` path mangling.** This cost me two failed verification attempts and produced one false positive (`diff` of two empty files reporting "BYTE-IDENTICAL"), which I caught and corrected using `git cat-file -p <ref>:'<path>'`. It is an environment quirk rather than a workflow defect. Recording it here for future orchestrators, but it does not justify a workflow rule.

## Suggested follow-up

- **M4's conformance matrix inherits M3's category labels.** `BestEffortSkipCategory { Availability, MisDeclaration, DataGeometryIntegrity }` becomes the matrix row labels. Two assignments were wrong across two review rounds (`GeographicKm2Unsupported` and `TileConstruction`, see `milestone-M3/review-s-{1,2}.json`). M4's planner and critic should re-derive each label from the merged code rather than copying M3's summary — this is a specific instance of the staleness finding above and is already recorded in `state.json.resume_instructions.after_M3`.
- **Baseline test-count discrepancy.** Phase 0 recorded "740 passed / 11 ignored"; my own count at the same tree was 741/12 after M1 (+1 test), and no `#[ignore]` was added by any commit (verified by attribute count diff and by grepping the diff for added `#[ignore]`). The ignored-count delta is unexplained and is most likely an orientation miscount. Worth confirming so the contract's `baseline` field is trustworthy as a regression signal.
- **Consider whether "execute, don't just read" should be an explicit critic instruction.** Every high-severity catch in this run came from a critic running something. This report does not recommend it as a rule because it may be an artifact of how I wrote the prompts rather than a property of the workflow — but it is the strongest correlation in the evidence and would be worth testing deliberately across a future run.
