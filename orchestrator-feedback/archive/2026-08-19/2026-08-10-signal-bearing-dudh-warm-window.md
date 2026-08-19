# PCE workflow feedback: signal-bearing Dudh warm window

- Date: `2026-08-10`
- Orchestrator: `Prime Agent / PCE session 019fe704-f4d5-7069-b7c5-1f9f448f0681`
- Run: `/Users/nicolaslazaro/Desktop/work/bluesmith/planning/2026-07-29-signal-bearing-dudh-warm-window`
- Outcome: `blocked at m3-s14 and m3-s4 pending an explicit external-gate execution accommodation`

## Executive summary

The workflow preserved the run history and stopped rather than accepting a red contract gate, but it could not execute a Stopwatch step whose stated test gate legitimately requires Docker. `pce contract check` always runs stated gates under a Seatbelt profile that denies the Docker socket, while the canonical Codex executor route also lacks Docker, network, and AWS credentials. The run spent two full workspace test invocations, including a 3,767-second serialized run, before a synthetic repository with no product code proved that the failure was imposed by PCE's gate sandbox.

A second orchestration incompatibility was present in the approved m3-s4 plan: the plan requires two commits so the first clean commit can be the measured model identity, while the executor contract requires exactly one step commit. The plan nevertheless received a schema-valid approval.

The smallest durable repair is to make gate capabilities explicit and validated before step planning or isolation. PCE must either provide a narrowly declared Docker-capable gate runner or fail before planning with a capability mismatch. PCE should also pass the exact temporary-directory value admitted by its Seatbelt profile into the gate child.

## Evidence reviewed

- Vision event log: `planning/2026-07-29-signal-bearing-dudh-warm-window/events.jsonl`, especially sequences 412–428.
- Approved m3-s4 plan: `planning/2026-07-29-signal-bearing-dudh-warm-window/milestone-3/step-4/plan.md`, SHA-256 `3bb70812a52755a6e69c07e6e2252bd02ab8236283dacc3d80425dc02bd31738`.
- Persisted critic verdict: `milestone-3/step-4/review-5.json`.
- Stopwatch contract at the m3-s4 paired worktree: `.pce/repository-contract.json`, whose exact test gate is `cargo test --workspace`.
- PCE composition root: `src/main.rs`, functions `measure_stated_contract_at_root`, `execute_sandboxed_gate_text`, `gate_child_environment`, and `render_seatbelt_profile`.
- PCE skill executor policy: `skills/pce/SKILL.md`, Phase 3 execution instructions requiring exactly one conventional step commit.
- Synthetic gate probe recorded in event sequence 427: no Stopwatch code, Rust, or tests; Docker and `/private/tmp` access both denied only inside `pce contract check`.

## What worked

### Red gates stopped execution

- Evidence: sequences 419, 424, and 425 stopped m3-s4 and created a delegated diagnostic/fix node rather than treating the failing gate as green.
- Effect: no m3-s4 implementation, commit, PR, S3 write, or EC2 action occurred under an unverified baseline.

### Append-only corrections preserved falsification history

- Evidence: sequence 424 refuted the concurrency hypothesis after the serialized gate failed; sequences 426–428 corrected the number and attribution of Docker-dependent tests without rewriting earlier records.
- Effect: the durable state distinguishes observations from superseded hypotheses and prevents a later session from repeating the 63-minute serialization experiment.

### A synthetic repository isolated the orchestration mechanism

- Evidence: sequence 427 records deterministic Docker-socket and `/private/tmp` denials in a repository containing no product code or tests, while the same socket and daemon remained usable outside the gate child.
- Effect: the probe separated a PCE sandbox capability mismatch from a Stopwatch defect.

## Friction and failures

### Stated gate requirements can be incompatible with the mandatory gate sandbox

- Severity: `high`
- Phase: `isolation and contract check`
- Observation: Stopwatch's exact stated test gate includes eight tests that legitimately invoke Docker. All eight fail inside `pce contract check` because its Seatbelt child cannot connect to the Docker socket. A synthetic repository reproduces the denial without Stopwatch code.
- Evidence: event sequences 426–428; `src/main.rs` unconditionally routes contract gates through `execute_sandboxed_gate_command`; `render_seatbelt_profile` has no declared Docker capability.
- Inference: the contract model records commands but not the capabilities required to execute them, so compatibility is discovered only after planning and worktree isolation.
- Impact: m3-s4 cannot satisfy Phase 3 through the canonical route. Two long gate runs and multiple human escalations were spent diagnosing an environment PCE constructs intentionally.

### The allowed temporary directory and the child environment disagree

- Severity: `high`
- Phase: `contract check`
- Observation: the synthetic gate child has an empty `TMPDIR`; `tempfile.mkdtemp(dir="/private/tmp")` is denied, while workspace-local temporary allocation succeeds.
- Evidence: sequence 427. In `src/main.rs`, `execute_sandboxed_gate_text` computes an allowed temporary directory from the parent, but `gate_child_environment()` forwards only `PATH`, `HOME`, `CARGO_HOME`, and `RUSTUP_HOME`.
- Inference: the child is not told to use the same temporary directory that PCE admitted in its Seatbelt profile.
- Impact: temporary-file behavior differs from the invoking environment and produces failures that resemble repository portability defects.

### Executor capability feasibility is checked after plan approval

- Severity: `high`
- Phase: `execution handoff`
- Observation: the approved m3-s4 plan begins with S3 downloads, requires AWS profile `work`, and includes Docker-bearing workspace gates. The bounded canonical-route probe later measured `network=no`, `aws_work_profile=no`, `s3_read=no`, and `docker=no`.
- Evidence: sequences 415, 421–423; m3-s4 plan measurement and gate sections.
- Inference: plan review checks textual self-sufficiency but does not compare required operations with the actual executor envelope.
- Impact: five critic rounds converged on a plan that the designated executor cannot perform.

### Plan approval missed the one-commit contradiction

- Severity: `high`
- Phase: `step planning and execution handoff`
- Observation: the approved plan requires a first clean code commit as the measured `model_git_sha`, followed by a second evidence/pointer commit. The PCE executor policy requires exactly one conventional step commit.
- Evidence: m3-s4 plan sections `Commits and pull request` and final gate instructions; `skills/pce/SKILL.md` Phase 3 executor policy; approval sequence 415.
- Inference: the critic frame does not mechanically reconcile plan commit topology with executor policy.
- Impact: changing sandbox capabilities alone would still leave the approved plan infeasible under the canonical executor contract.

### Verdict persistence failure consumed a human-authorized retry

- Severity: `medium`
- Phase: `step-plan review`
- Observation: critic round 4 substantively emitted approval but did not write `review-4.json` and used schema-invalid `root_cause: "none"` in stdout.
- Evidence: sequences 410–414 and `review-4.md`.
- Inference: the route correctly rejected the missing product, but the child did not reliably follow the output-artifact contract.
- Impact: an additional 658-second critic dispatch and human authorization were needed solely to persist a conforming verdict.

## Recommendations

### Add declared gate capabilities and reject incompatible contracts before planning

- Addresses: `Stated gate requirements can be incompatible with the mandatory gate sandbox`.
- Change: extend each stated gate, or the repository contract as a whole, with a closed capability set such as `docker`, `network`, and `host_temp`. During contract bootstrap/check and Phase 3 preflight, compare it with the selected gate runner. Either construct the narrow approved capability profile or stop with a capability-mismatch error before step planning.
- Location: repository-contract schema and parsing in `crates/core`; contract execution and Phase 3 instructions in `src/main.rs` and `skills/pce/SKILL.md`.
- Trade-off: contract authors must declare capabilities and PCE must maintain capability-specific runners.
- Confidence: `high`.

### Propagate the admitted canonical temporary directory

- Addresses: `The allowed temporary directory and the child environment disagree`.
- Change: insert the exact canonical temporary path admitted by `render_seatbelt_profile` into the child environment as `TMPDIR`. Add a synthetic contract test proving `tempfile.mkdtemp()` succeeds there and a forbidden directory remains denied.
- Location: `src/main.rs`, `execute_sandboxed_gate_text` and `gate_child_environment`; contract sandbox integration tests.
- Trade-off: one additional deterministic environment entry.
- Confidence: `high`.

### Run an executor-capability preflight before the first plan writer

- Addresses: `Executor capability feasibility is checked after plan approval`.
- Change: derive required external capabilities from the proposed step or require them as structured planner input, then probe the exact anchored executor route before spending planning rounds. A mismatch must route to an operator-owned action, a different runner, or an early escalation.
- Location: `skills/pce/SKILL.md`, Phase 3 before `Plan (Codex)`; optionally a structured capability field in step graph/plan schemas.
- Trade-off: one bounded preflight per distinct execution envelope.
- Confidence: `high`.

### Validate commit topology against executor policy

- Addresses: `Plan approval missed the one-commit contradiction`.
- Change: add a critic obligation and preferably a binary check that rejects plans requiring more commits than the executor policy permits. If measurements require an intermediate clean commit identity, model that as an explicit multi-stage/operator execution type rather than relying on prose exceptions.
- Location: `skills/pce/SKILL.md` planning critic frame and execution state model.
- Trade-off: plans with post-execution evidence may require an additional explicit stage type.
- Confidence: `high`.

### Provide a typed external-gate attestation instead of ad hoc human substitution

- Addresses: Docker-bearing repositories that cannot use the current Seatbelt runner.
- Change: as an interim or permanent operator route, accept a structured attestation containing repository SHA, clean-state observation, exact stated commands, exit statuses, and output digests. The binary should validate completeness and record that the authority was external rather than reporting `pce contract check` as green.
- Location: contract command/state model in `crates/core`, CLI composition in `src/main.rs`, and Phase 3 isolation instructions.
- Trade-off: trusted operator authority must be explicit and auditable; this is weaker than a fully PCE-owned capable sandbox.
- Confidence: `experimental` pending threat-model review.

## No-change decisions

- Keep the rule that a red or malformed gate cannot advance. It prevented execution against an unverified baseline; the defect is capability matching, not fail-loud behavior.
- Keep append-only event history. Superseding records made the two incorrect causal hypotheses visible without converting them into false facts.
- Do not weaken or conditionally skip the Docker-dependent Stopwatch tests. The probe shows they exercise a real repository requirement that the current runner cannot provide.

## Suggested follow-up

- PCE issue: repository-contract gates declare and receive narrow execution capabilities, starting with Docker.
- PCE bug: propagate the canonical allowed temporary directory as `TMPDIR` in gate children.
- PCE issue: capability preflight occurs before step planning and critic rounds.
- PCE issue: represent committed measurement identity followed by evidence publication without contradicting the one-commit executor policy.
