# PCE workflow feedback: incidence core plan-stage cap exhaustion

- Date: `2026-08-11`
- Orchestrator: `PCE orchestrator using current binary behavior inspected at pce HEAD 2e4010c`
- Run: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core`, milestone 3
- Outcome: `blocked with two capped plan loops`

## Executive summary

Two replacement nodes exhausted both `step-plan-writer` and `step-plan-critic` at three validated productions while their executor budgets remained unused or nearly unused. The terminal `m3-s9` finding is not a domain or product defect: two source lines quoted under “reproduce exactly” are one formatting column beyond the repository's rustfmt configuration. The critic demonstrated that rustfmt-normalizing those two match arms makes every gate and all four required mutants pass, but no writer round remains to change the quotation and no critic round remains to approve it.

This is a workflow recovery defect. The cap correctly stops automatic work, but PCE offers no typed human-authorized continuation on the existing `(node, role)` series. Generic escalation closure cannot affect admission. Supersession only changes the accounting key and repeats the node without evidence that another three-round plan loop will converge.

## Evidence reviewed

- `planning/2026-08-10-incidence-core/events.jsonl`
- `planning/2026-08-10-incidence-core/milestone-3/step-9/plan.md`
- `planning/2026-08-10-incidence-core/milestone-3/step-9/review-1.json`
- `planning/2026-08-10-incidence-core/milestone-3/step-9/review-2.json`
- `planning/2026-08-10-incidence-core/milestone-3/step-9/review-3.json`
- `planning/2026-08-10-incidence-core/milestone-3/step-9/pr/review-1.json`
- Preserved incidence commit `1623d9b2499372ed41ca4bca78bf1d97ed6d289c`, worktree `incidence-worktrees/m3-s9`, and PR #17
- PCE `crates/core/src/run_state.rs:3640-3820`, `src/main.rs:2789-2872`, `tests/dispatch.rs:6612-6648`, and `skills/pce/SKILL.md:407-496`

## What worked

### Critics executed the specification rather than accepting prose

- Evidence: `step-9/review-3.json` appends the plan's quoted tests byte-for-byte and runs `cargo fmt --all --check`. It reports the exact rustfmt diffs and then verifies that, after formatting, every remaining debug/release gate and all four required mutants behave as prescribed.
- Effect: prevents approving a plan that contradicts its own first acceptance command.

### The cap is enforced atomically

- Evidence: `run_state.rs:3801-3802` refuses admission once the exact `(node, role)` count reaches three; `main.rs:2866-2871` exposes the refusal; `tests/dispatch.rs:6612-6648` proves no child or log mutation occurs on the forbidden fourth issuance.
- Effect: the orchestrator cannot silently exceed the configured automatic-work limit.

## Friction and failures

### Independent three-round plan loops did not converge

- Severity: `high`
- Phase: `step planning`
- Observation: `m3-s8` reached writer 3/3 and critic 3/3 with executor 0/3. `m3-s9` reached writer 3/3 and critic 3/3 with executor 1/3. Review blocker counts did not monotonically approach zero: `m3-s8` moved 4→2→4 and `m3-s9` moved 1→2→1 as executable scrutiny exposed deeper seams.
- Evidence: the dispatch ledger in `events.jsonl` and the per-round verdicts under `milestone-3/step-8/` and `step-9/`.
- Inference: the workflow assumes that a complex executable plan reaches a fixed point within three independent author and critic productions. This run refutes that assumption twice. Superseding to a fresh node resets counters but does not improve convergence.
- Impact: two valid pieces of milestone work cannot enter or resume execution, and downstream `m3-s5`/`m3-s6` remain blocked.

### The terminal blocker is formatter-governed source layout treated as an immutable authored literal

- Severity: `medium`
- Phase: `step planning criticism`
- Observation: two `panic!` match arms quoted under “reproduce exactly” exceed rustfmt's configured width. The plan simultaneously prohibits changing authored text and requires `cargo fmt --all --check` to pass first.
- Evidence: `step-9/review-3.json` records exit 1 and the two exact rustfmt rewrites. After applying only those rewrites, fmt, clippy, check, test, build, release gates, and four mutants all pass.
- Inference: the plan contract fails to distinguish semantic/golden literals from source layout governed by the repository formatter.
- Impact: one mechanical quotation correction consumes an otherwise unavailable author/critic cycle and strands mergeable PR #17.

### Human escalation has no typed continuation for a capped plan pair

- Severity: `high`
- Phase: `recovery`
- Observation: generic escalation open/close state does not participate in defect-round admission. Authorizing another round in prose therefore cannot enable it. Reopening only the writer would leave the changed plan unvalidated; reopening only the critic would leave the rejected plan unchanged.
- Evidence: `classify_dispatch_admission` reads exact round state and typed non-production holds but no generic escalation resolution. `SKILL.md:488-496` requires cap exhaustion to stop and forbids proceeding unconverged.
- Inference: plan recovery must be a coupled, typed transaction rather than a raw increase to one role's counter.
- Impact: the only supported workaround is node supersession, which launders the same artifact through a new accounting identity and repeats a full workflow.

## Recommendations

### Add a typed, single-use coupled retry for capped plan recovery

- Addresses: independent plan-loop exhaustion and missing human continuation.
- Change: introduce a typed cap-hold resolution that lets a human authorize exactly one additional `step-plan-writer` correction followed by exactly one `step-plan-critic` validation on the same canonical node. Derive and consume it atomically from append-only records. Preserve all prior issuance ordinals and round history. `re-plan` and `abandon` should remain terminal alternatives. A generic free-text escalation close must not reset counters.
- Location: `crates/core/src/event_log.rs`, `crates/core/src/run_state.rs`, `src/main.rs`, `tests/dispatch.rs`, status/schema rendering, and the cap/recovery clauses in `skills/pce/SKILL.md`.
- Trade-off: status must expose authorization scope and consumption, and the state model gains a coupled-role recovery transition. The default three-round automatic cap remains unchanged.
- Confidence: `high`.

### Format quoted source before plan approval

- Addresses: formatter-governed source layout blocker.
- Change: the plan critic should materialize quoted source and run the repository formatter before approving its first version. Source layout is formatter-normalized by default; only literals explicitly labelled as byte-sensitive remain verbatim. Executors may run the formatter on newly authored source before the check gate without violating “reproduce exactly.”
- Location: `step-plan-writer`, `step-plan-critic`, and `step-executor` role frames/prompts used by `skills/pce/SKILL.md`.
- Trade-off: plans cannot treat ordinary source formatting as byte-authoritative implicitly. Byte-sensitive source tests require an explicit label.
- Confidence: `high`.

### Do not use canonical-node supersession as cap reset

- Addresses: repeated replacement-node workaround.
- Change: reserve runtime delta/supersession for changed graph semantics or genuinely new work. When the proposed successor carries the same artifact and differs only by a repair that the capped role would normally perform, require typed cap recovery or stop pending workflow repair.
- Location: runtime-delta and supersession guidance in `skills/pce/SKILL.md`.
- Trade-off: runs pause until PCE supports recovery instead of continuing through duplicated nodes. Audit semantics become accurate and repeated full-node work is avoided.
- Confidence: `high`.

## No-change decisions

- Do not weaken the critic. The literal plan did fail its own first gate.
- Do not merge PR #17 while its revised plan is unconverged.
- Do not raise every role's automatic cap globally from three based only on this run. The required change is explicit human-controlled recovery after the stop.
- Do not let generic escalation text or direct event-log edits bypass admission.

## Suggested follow-up

- Use this report as concrete evidence for PCE issue #44.
- Specify the coupled writer/critic retry state transition and production CLI tests.
- After that mechanism lands, resume `m3-s9` at commit `1623d9b` and PR #17 rather than creating another replacement node.
