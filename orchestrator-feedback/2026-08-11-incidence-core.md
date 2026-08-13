# PCE workflow feedback: incidence core

- Date: `2026-08-11`
- Orchestrator: restarted PCE orchestrator session; current binary source inspected at `2e4010c`
- Run: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core`
- Outcome: `blocked during milestone 3 PR repair`

## Executive summary

The gates correctly rejected a materially sound `m3-s4` artifact whose test suite could not detect deletion of ordinate signed-zero normalization. The workflow then had no executable recovery after the human chose to retain the invariant: the exact `(m3-s4, step-executor)` series had three validated productions, admission unconditionally refuses a fourth, and a generic escalation close cannot alter that state. The only supported continuation is to create a semantically duplicate canonical node, repeat planning and all gates, and close the original mergeable PR as superseded for one test row.

The cap remains useful as a cost backstop, and the blocking gate must remain. The actionable defect is the missing typed post-cap recovery path. A related accounting issue makes the effective repair allowance smaller than the terminology suggests: every validated executor production increments the "defect-round" series, including initial or approving production, rather than only a defect repair.

## Evidence reviewed

- `planning/2026-08-10-incidence-core/events.jsonl`, including `m3-s4` executor issuances 327, 335, and 340 and PR-reviewer issuance 349.
- `planning/2026-08-10-incidence-core/milestone-3/step-4/pr/review-2.json`.
- `planning/2026-08-10-incidence-core/milestone-3/steps.json`.
- Incidence commit `90c5c62`, worktree `/Users/nicolaslazaro/Desktop/work/incidence-worktrees/m3-s4`, and PR #16.
- `crates/core/src/run_state.rs:3640-3820` and `src/main.rs:2789-2872` in PCE at `2e4010c`.
- `tests/dispatch.rs:6612-6648` and `skills/pce/SKILL.md:407-496` in PCE at `2e4010c`.
- PCE issue #44, “When does a gate loop stop, and what does an unresolved stop owe the human?”

## What worked

### PR falsification detected a real vacuity rather than accepting green gates

- Evidence: `milestone-3/step-4/pr/review-2.json` records a compiling mutant that replaces normalized ordinate bits with raw input bits. `cargo test --workspace` remained green because no fixture supplied a negative-zero ordinate. The reviewer separately observed different structural bits and JSON despite identical canonical bytes.
- Effect: prevented merging an artifact that violated the plan's signed-zero normalization and the correspondence between structural and canonical identity.

### Admission enforces the recorded cap atomically

- Evidence: `run_state.rs:3780-3820` derives admission from exact `(node, role)` state; `main.rs:2866-2871` refuses the dispatch; `tests/dispatch.rs:6612-6648` proves a fourth invocation neither spawns a child nor changes the log.
- Effect: the orchestrator cannot silently exceed a human-facing cost backstop or fabricate an extra round in scratch state.

## Friction and failures

### A human-resolved cap escalation cannot produce any permitted continuation on the same artifact

- Severity: `high`
- Phase: `PR review and recovery`
- Observation: `m3-s4` has a correct implementation, a clean one-commit artifact on the pinned base, a mergeable PR, and one blocking test-only change. Its three validated `step-executor` productions are exhausted. Generic `escalation-open`/`escalation-close` records do not participate in `classify_dispatch_admission`, so no human answer can make the required repair dispatch admissible.
- Evidence: `run_state.rs:3801-3802` returns `DefectRoundCapExhausted` unconditionally at count 3. The classification reads defect rounds and typed non-production holds, not generic escalation state. `SKILL.md:488-496` requires cap exhaustion to escalate and forbids proceeding unconverged, while runtime delta adaptation creates a new canonical node with a fresh series.
- Inference: escalation currently records that a decision happened but provides no state transition for the human decision “retain the requirement and authorize the smallest repair.” Supersession works by changing the accounting key rather than resolving the capped series.
- Impact: the supported recovery for one test row is a new graph node, new plan, new branch and combined commit, new falsification cycle, new PR review cycle, and closure of an otherwise mergeable PR. Milestone 3 has already used the same escape for `m3-s2`→`m3-s7` and `m3-s3`→`m3-s8`, so this is repeated workflow work rather than a hypothetical edge case.

### “Defect round” accounting charges every validated executor production

- Severity: `medium`
- Phase: `dispatch accounting`
- Observation: a validated required artifact increments the `(node, role)` round count without consulting verdict class or whether the production was the initial implementation, a repair, or an approval.
- Evidence: `run_state.rs:3693-3697` calls `increment_defect_round` for every `ArtifactOutcome::Validated`. `step-executor` is the role for both initial execution and all fixes. Thus a cap of three permits the initial production plus at most two later validated repairs, not three defect repairs.
- Inference: the implementation is internally consistent, but the name and workflow prose overstate the available repair budget and make empirical claims about “three rounds” ambiguous.
- Impact: a step can reach the cost backstop after two repair opportunities even when every dispatch is productive. Operators diagnose this as an insufficient three-repair cap when the actual policy is three total validated executor productions.

### Case-oriented plans repeatedly leave properties un-falsifiable

- Severity: `medium`
- Phase: `step planning`
- Observation: this milestone repeatedly used fixtures for which the required behavior coincided with a plausible wrong implementation: palindromic byte-order goldens, aligned identity/magnitude ordering, incomplete operand-position coverage, uncalled generated conversion entry points, absent rejection rows, and no negative-zero ordinate.
- Evidence: durable falsification and PR verdicts under `milestone-3/step-{2,3,4,7}/` and the surviving mutant in `step-4/pr/review-2.json`.
- Inference: sampled example lists do not force planners or critics to identify the property partition and a plausible mutant for each branch. Strong downstream gates find the omissions one seam at a time and consume the bounded repair series.
- Impact: repeated repair cycles and three replacement nodes within one milestone.


### Plan-stage caps can strand an executable artifact over a mechanical quotation defect

- Severity: `high`
- Phase: `step planning and PR repair`
- Observation: after the earlier report, both replacement nodes exhausted their independent `step-plan-writer` and `step-plan-critic` series before execution could continue. `m3-s8` reached writer 3/3 and critic 3/3 with executor 0/3. `m3-s9` reached writer 3/3 and critic 3/3 with executor 1/3. The final `m3-s9` critic finding identifies two quoted `panic!` match arms that rustfmt wraps because they exceed the configured width; after that mechanical wrapping, every gate and all four mandated mutants pass.
- Evidence: `planning/2026-08-10-incidence-core/milestone-3/step-9/review-3.json` records `REVISE`, `root_cause: step_plan`, and the exact two formatting diffs. Its replacement execution reports `cargo fmt --all --check` and all remaining gates green after requoting. The preserved artifact is commit `1623d9b`, tracked-clean apart from untracked `pr-body.md`; incidence PR #17 is open and mergeable. The run reports both plan roles at 3/3, so neither the plan writer nor critic may validate the corrected quotation on `m3-s9`.
- Inference: the three-round limit is not merely short for implementation repair. Applying the same hard limit independently to plan authorship and criticism assumes complex executable specifications converge within three validated productions. This run provides two counterexamples. It also shows that blocker count is not a sufficient convergence measure: `m3-s8` changed 4→2→4 and `m3-s9` changed 1→2→1 as deeper executable seams were exposed.
- Impact: the workflow's only currently admissible continuation is another canonical-node supersession even though the product artifact, revised design, gate set, and mutation evidence are known. Repeating the entire node does not reduce uncertainty proportionally and provides no reason the same three-round boundary will converge on the next identifier.

## Recommendations

### Add a typed, bounded human resolution for defect-cap exhaustion

- Addresses: `A human-resolved cap escalation cannot produce any permitted continuation on the same artifact`.
- Change: model cap exhaustion as a typed hold keyed by exact `(node, role)`. Permit a human `retry` resolution to authorize exactly one additional validated-production window on that same key; retain `re-plan`/`abandon` outcomes that do not reopen it. Admission must derive the authorization solely from append-only typed records and consume it atomically. Do not make free-text generic escalation closes reset the cap.
- Location: `crates/core/src/event_log.rs` typed event registry, `crates/core/src/run_state.rs` dispatch-outcome fold and `classify_dispatch_admission`, `src/main.rs` dispatch admission CLI, production tests in `tests/dispatch.rs`, and the cap/escalation clauses in `skills/pce/SKILL.md`.
- Trade-off: a human can authorize additional cost, so status output must display the original count, authorization, and consumed extension. The default cap remains unchanged and no automatic fourth round is introduced.
- Confidence: `high`.

### Either rename the counter or distinguish initial production from defect repair

- Addresses: `“Defect round” accounting charges every validated executor production`.
- Change: choose and document one policy. Minimal documentation-only option: rename it to `validated-production round` everywhere and state explicitly that initial execution consumes round 1. Semantic option: record/derive the initial execution separately and increment the defect counter only for validated repair dispatches following a blocking verdict.
- Location: `crates/core/src/run_state.rs`, dispatch status/schema rendering, `skills/pce/SKILL.md`, and `tests/dispatch.rs`.
- Trade-off: the semantic option needs an authoritative join from repair dispatches to prior verdicts and is therefore larger; the rename preserves behavior but does not increase repair capacity.
- Confidence: `high` for the terminology fix, `experimental` for changing accounting.

### Require a falsification matrix in step plans

- Addresses: `Case-oriented plans repeatedly leave properties un-falsifiable`.
- Change: require each planned invariant to name its input partition, at least one plausible wrong implementation or deletion mutant, and the exact assertion that would fail. Plan criticism rejects symmetric transformation paths, operand positions, error variants, or wire branches without either an exhaustive partition or an explicit equivalence argument.
- Location: the `step-plan-writer` and `step-plan-critic` frames/prompts used by `skills/pce/SKILL.md`.
- Trade-off: longer plans and more pre-execution criticism; fewer late repair and supersession cycles.
- Confidence: `high` for this run's recurring failure shape.


### Make post-cap recovery role-independent and resumable at the existing node

- Addresses: `Plan-stage caps can strand an executable artifact over a mechanical quotation defect`.
- Change: the typed post-cap human resolution must cover every round-bearing role, not only `step-executor`. A one-window `retry` should reopen the exact capped `(node, role)` while preserving its issuance history and artifact lineage. For a capped plan critic, the authorization must permit one writer correction followed by one fresh critic observation as one bounded recovery transaction; otherwise reopening only the critic cannot change the rejected plan, while reopening only the writer cannot validate it.
- Location: dispatch outcome/admission domain types in `crates/core/src/run_state.rs`, typed records in `crates/core/src/event_log.rs`, CLI admission in `src/main.rs`, and plan-loop recovery rules in `skills/pce/SKILL.md`.
- Trade-off: recovery authorization needs an explicit scope naming the coupled roles and a single-consumption rule. This is more state than a numeric cap extension but avoids laundering the same artifact through fresh node identities.
- Confidence: `high`.

### Treat formatter-normalized source quotations as equivalent where formatting is not the asserted property

- Addresses: the final mechanical `m3-s9` blocker, but not the general cap defect.
- Change: when a plan quotes source and the repository contract includes a formatter, distinguish semantic or golden-byte literals from formatter-governed source layout. Authorize the executor to run the repository formatter on newly added source before the check gate unless the plan explicitly labels layout bytes as acceptance data. The plan critic should apply the formatter before testing quoted source so its first review catches this class without another author/critic cycle.
- Location: `step-plan-writer`, `step-plan-critic`, and `step-executor` role frames/prompts referenced by `skills/pce/SKILL.md`.
- Trade-off: quoted source is no longer byte-prescriptive by default; plans that genuinely test source bytes must label that exception.
- Confidence: `high`.

## No-change decisions

- Do not weaken PR review or accept the `m3-s4` finding as documented debt. The surviving mutant demonstrates an observable structural/JSON mismatch and the one-row fix is known.
- Do not simply raise the global cap from three to four based on one milestone. The cap correctly stopped automatic spending. The missing mechanism is an explicit human-controlled continuation after the stop.
- Do not allow an escalation close to reset counters implicitly. A generic free-text record is insufficient authority for a cost-bearing dispatch.
- Until typed recovery exists, canonical-node supersession remains the only supported continuation and is preferable to bypassing binary admission or editing the event log.

## Suggested follow-up

- Attach this run as empirical evidence to PCE issue #44.
- Specify and test the typed post-cap `retry` transition independently from the incidence run.
- Evaluate the falsification-matrix prompt on `m3-s5` and `m3-s6` before changing the global round policy.
