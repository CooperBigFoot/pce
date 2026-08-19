# PCE workflow feedback: escalation resolution cannot authorize a capped retry

- Date: `2026-08-12`
- Orchestrator: `PCE orchestrator; exact model/session not recorded in the event log`
- Run: `RivRetrieve/planning/2026-08-11-the-store-is-the-only-copy`, node `m1-s2`
- Outcome: `blocked; recovery redirected to a runtime delta node`

## Executive summary

The binary permanently refuses an exact `(node, role)` after three validated-production completions. The documented escalation cycle can record an operator's resolution, but generic `escalation-close` records never participate in dispatch admission. Consequently, the operator authorized one narrowly scoped additional `m1-s2` executor attempt, the orchestrator recorded that authorization, and PCE still refused the dispatch without an executable sanctioned recovery on the same node.

The released binary's only auditable workaround is a new delta node with a fresh round series. That workaround changes decomposition and PR topology solely because the admission model cannot represent a one-use operator-policy exception. Role substitution, hand editing, and an ambient cap override are not acceptable substitutes.

A separate uncommitted change currently present in `../pce/crates/core/src/run_state.rs` adds `PCE_DEFECT_ROUND_CAP`. It must not be shipped or used: it changes replay of the same log through hidden global process state, affects every series, is not single-use, is not recorded or consumed, and silently ignores malformed values.

## Evidence reviewed

- `RivRetrieve/planning/2026-08-11-the-store-is-the-only-copy/events.jsonl`, sequences 100–115.
- Sequence 114, `escalation-open` for key `m1-s2-executor-cap-exhausted-block-a`.
- Sequence 115, `escalation-close` recording authorization for exactly one additional executor attempt.
- Repeated CLI diagnostic: `dispatch admission refused: defect-round cap 3 is exhausted for node m1-s2 and role step-executor`.
- `RivRetrieve/planning/2026-08-11-the-store-is-the-only-copy/milestone-1/step-2/pr-review/review-2.json`.
- `pce/crates/core/src/run_state.rs`, `derive_dispatch_outcome_state` and `classify_dispatch_admission`.
- `pce/src/main.rs`, `admit_and_append_dispatch`.
- `pce/tests/dispatch.rs`, `productive_rounds_and_productless_retries_are_separate_through_production_cli`.
- `pce/skills/pce/SKILL.md`, cap/escalation and runtime graph-adaptation rules.
- Uncommitted diff in `pce/crates/core/src/run_state.rs` introducing `PCE_DEFECT_ROUND_CAP`.

## What worked

### The cap prevented an unrecorded fourth attempt

- Evidence: admission refused before appending another dispatch or spawning a child after the third validated-production completion.
- Effect: the cost boundary remained effective and the event log stayed unchanged on refusal.

### The executor refused an invalidated plan rather than improvising

- Evidence: the second charged executor round returned `PLAN_INFEASIBLE` with `root_cause=step_plan` after the worktree rebase changed the baseline from which final-history and write-set criteria were measured.
- Effect: the executor exposed upstream premise drift instead of producing an artifact that could not satisfy its approved plan.

### PR review found a real contract contradiction

- Evidence: `review-2.json` demonstrates that exact equality against the fixture's eight-column schema rejects both a minimal conforming five-column store and a conforming store with another provider-native column. The normative document and approved plan permit additional native columns.
- Effect: the gate prevented a provider-specific fixture schema from becoming the provider-neutral store contract.

### Dispatch admission already has the correct atomic boundary

- Evidence: `src/main.rs::admit_and_append_dispatch` reads, folds, classifies, and appends while holding the event-log file lock.
- Effect: a future one-use authorization can be consumed atomically at issuance without a mutable side store.

## Friction and failures

### A closed escalation cannot affect exhausted-cap admission

- Severity: `high`
- Phase: `recovery`
- Observation: sequence 115 closes the exact escalation with a resolution authorizing one additional attempt, but an identical dispatch remains refused by the cap.
- Evidence: `derive_dispatch_outcome_state` does not fold generic `EscalationOpen` or `EscalationClose` into dispatch outcome state. `classify_dispatch_admission` returns `DefectRoundCapExhausted` whenever the count is at least three.
- Inference: generic escalation payloads were designed as display-oriented free text and no typed authority was added when cap admission became binary-owned.
- Impact: the documented stop → resolve → continue lifecycle terminates after “resolve.” A human decision can be recorded but cannot be enacted.

### The only released-binary workaround changes graph identity

- Severity: `high`
- Phase: `recovery`
- Observation: a runtime delta node receives a fresh `(node, role)` series and is admitted, while the original authorized node remains permanently closed.
- Evidence: defect rounds are keyed only by exact `(node, role)`; `SKILL.md` permits runtime graph adaptation through a new canonical stub.
- Inference: delta adaptation is acting as an accounting escape rather than as a response to newly discovered product decomposition.
- Impact: one logical step becomes two nodes and normally two PRs. To keep the known defect off `main`, the first PR must exist temporarily only on the private milestone integration branch before the repair node can branch from it.

### Correct upstream-cause refusal consumed executor capacity

- Severity: `medium`
- Phase: `execution`
- Observation: the executor's `PLAN_INFEASIBLE` result was counted as one of its three validated-production rounds even though it named `root_cause=step_plan` and correctly performed no workaround.
- Evidence: every completion with `ArtifactOutcome::Validated` increments the series for the dispatch issuance's `(node, role)`; verdict root cause is not an input to the fold.
- Inference: “validated production round” and “defect attributable to this role” are currently conflated.
- Impact: upstream re-planning churn can exhaust executor capacity even when the executor behaves correctly. This run lost one of three executor rounds that way.

### The ambient environment override is not an auditable recovery

- Severity: `high`
- Phase: `recovery`
- Observation: an uncommitted patch reads `PCE_DEFECT_ROUND_CAP`, accepts any parseable value above three, and changes admission globally for the process.
- Evidence: local diff in `crates/core/src/run_state.rs`; the value is absent from the event log and dispatch key.
- Inference: this was an attempted local unblock rather than a designed authority model.
- Impact: identical logs replay differently under different environments; the invoking process can grant itself arbitrary additional rounds; the grant is neither scoped nor consumed; malformed input silently falls back. Existing classifier tests also become environment-sensitive.

### The refusal diagnostic names no executable recovery

- Severity: `medium`
- Phase: `recovery`
- Observation: the diagnostic states only that the cap is exhausted.
- Evidence: the byte-exact assertion in `tests/dispatch.rs` and the emitted `src/main.rs` branch.
- Inference: admission implemented the stopping condition before a recovery protocol existed.
- Impact: the orchestrator must inspect code and infer delta adaptation; role abuse and manual edits remain technically tempting.

## Recommendations

### Add typed, append-only, one-use cap authorization

- Addresses: closed escalations cannot affect admission.
- Change: add a typed authorization field or dedicated typed event identifying the exact `(node, role)`. It must be valid only after that series is exhausted, grant exactly one later matching dispatch issuance, and be consumed by the first matching issuance while the existing event-log lock is held. It must not reset or decrement historical rounds. Generic free-text escalation records remain inert.
- Location: `crates/core/src/event_log.rs`, `crates/core/src/run_state.rs`, `src/main.rs`, run-snapshot schema/rendering, `tests/dispatch.rs`, and `skills/pce/SKILL.md`.
- Trade-off: expands the append-only event contract and status schema. Legacy close records must remain readable.
- Confidence: `high`.

Required regression coverage:

1. Three validated productions refuse a fourth without spawning a child or changing the log.
2. One typed authorization admits exactly one issuance for its exact `(node, role)`.
3. A second issuance and every other node or role remain refused.
4. Two concurrent contenders can spend the grant only once.
5. A grant written before cap exhaustion cannot be banked.
6. Generic close text, malformed payloads, `re-plan`, and `abandon` do not authorize retry.
7. Restart and replay derive identical unused and consumed grant state independent of environment.
8. Status exposes the original count, authorization scope, and consumed issuance sequence.

### Remove the ambient cap override

- Addresses: replay-unstable and self-authorized recovery.
- Change: revert the uncommitted `PCE_DEFECT_ROUND_CAP` implementation and add a test proving relevant environment values cannot change admission.
- Location: `crates/core/src/run_state.rs` and its classifier tests.
- Trade-off: the current run must use the delta workaround until typed recovery ships.
- Confidence: `high`.

### Separate round production from role-attributable failure

- Addresses: correct upstream-cause refusal consumed executor capacity.
- Change: investigate a typed accounting projection that does not charge an executor convergence cap for a validated result whose structured root cause belongs to the step or milestone plan. Do not infer this from free text. Treat this separately from one-use recovery because it changes what the cap measures.
- Location: dispatch completion/verdict accounting in `crates/core/src/run_state.rs`, associated schemas, and cap policy in `skills/pce/SKILL.md`.
- Trade-off: requires a durable binding between the validated verdict and completion accounting; careless attribution could make caps avoidable.
- Confidence: `experimental` pending a precise authority design.

### Make the refusal actionable

- Addresses: dead-end diagnostic.
- Change: after typed authorization exists, name its exact sanctioned command or event. Until then, state that same-node admission is permanently closed and name runtime delta adaptation as the only sanctioned recovery.
- Location: `src/main.rs` diagnostic and `skills/pce/SKILL.md` recovery section.
- Trade-off: byte-exact CLI tests and documentation must change together.
- Confidence: `high`.

## No-change decisions

- Do not add `--force` or role substitution. Either would bypass the append-only authority and corrupt role-keyed accounting.
- Do not reinterpret the current free-text escalation key or resolution. Naming conventions and prose are not typed authorization and cannot safely determine cost admission.
- Do not reset the immutable round count. Historical production remains true after a retry is authorized.
- Do not apply the RivRetrieve repair manually outside orchestration. The released binary's delta route is inefficient but remains auditable.
- Do not merge the known BLOCK-A contradiction to `main`. PR #150 may exist only as an intermediate on the private milestone integration branch before its delta repair lands.

## Suggested follow-up

1. Open a contained PCE effort for typed exhausted-cap recovery, including status/schema compatibility and concurrent single-consumption proof.
2. Separately define whether caps measure all validated production rounds or only role-attributable defect rounds.
3. For the blocked RivRetrieve run, append `m1-s3` as a delta depending on `m1-s2`, merge PR #150 only into `milestone-1`, branch the repair from that integration head, and prohibit milestone promotion until the repair passes review.
