# WP3 bounded recovery evidence

## Executed end-to-end scenario

`tests/package_recovery.rs::failing_branch_climbs_recovery_ladder_while_sibling_completes`
was executed through the compiled `pce package driver-run` binary with an isolated target directory.
The graph contains two initially ready packages. Package `A` reports `done` on every worker attempt,
but its independently executed criterion always prints `actual-failure-output` and exits 7. Package
`X` reports `done` and its criterion passes.

Observed package sequence:

1. The full initial antichain dispatched `A` issuance 1 and `X` issuance 2. `X` completed.
2. A's first criterion failure selected `retry`. Issuance 3 received the same graph-authored brief
   and a fresh worker process.
3. A's second criterion failure selected `local-patch`. Issuance 4 received the same base brief plus
   the criterion name `deterministic`, exact command `printf actual-failure-output; exit 7`, status
   `exited 7`, stdout `actual-failure-output`, and captured stderr.
4. A's third criterion failure selected `replan`. A was parked for plan version `n+1`; X remained
   complete. The graph outcome was `blocked`, not an execution error.

## Recorded artifacts

The integration test reads and checks every artifact before its temporary run directory is removed:

| Artifact | Observation |
|---|---|
| `graph.json` | Two independent packages and immutable plan version 1 |
| `driver.jsonl` `recovery-configured` | Retry limit 1 and local-patch limit 1 |
| `package-outcomes/A/{1,3,4}.json` | Three strict worker `done` outcomes |
| `package-outcomes/X/2.json` | Independent branch worker outcome |
| Three `criterion-executed` events for A | Exact command, exit code 7, stdout, stderr, and working directory |
| `recovery-rung-attempted` for issuance 3 | Typed `retry`, first failure evidence, and unchanged base brief |
| `recovery-rung-attempted` for issuance 4 | Typed `local-patch`, second failure evidence, and composed evidence-bearing brief |
| `recovery-parked` | Ordered retry, local-patch, and replan records, including what each tried and its evidence |
| `observations` | Worker-visible `PCE_RECOVERY_RUNG` and `PCE_PACKAGE_BRIEF` for every process |
| Package-agent stdin test | `PCE_RECOVERY_SUPPLEMENT` is appended to the full WP5-composed brief before the live worker reads stdin |
| Final driver snapshot | A parked, X complete, A dispatch budget 0, next rung replan |

`identical_failure_sequence_changes_actual_dispatches_when_ladder_is_removed` runs the same red
criterion under two persisted policies. Limits 1/1 produce three actual worker spawns; limits 0/0
produce one. This observes `Command::spawn` behavior rather than merely asserting that policy code
was consulted.

`repeated_environment_failures_do_not_spend_recovery_budget` makes the worker process exit 70 five
times, more times than the default ladder has dispatching rungs, and succeed on the sixth attempt.
The journal contains five `worker-environment-failed` events and no recovery-rung event. Status still
reports retry remaining 1 and local-patch remaining 1.

## Charging rule

A strict worker `failed` outcome and a red independently executed criterion charge the work budget.
A worker process/toolchain exit that produces no typed package judgement is recorded as an
environment failure and does not charge it. `mis-specified` is neither charged nor retried; the
existing WP7 path parks it immediately because the graph, not the implementation attempt, must be
re-authored.

## Commands

```text
cargo test --test package_recovery
cargo test --test package_driver
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
```
