# WP7 driver evidence

The automated executable evidence is `tests/package_driver.rs`, run with a target directory isolated
from the installed PCE binary.

## Judgement records

`criteria_record_command_status_and_output_and_block_dependents` drives a two-package graph. The
worker package is folded to `judging`, then both authored shell strings run in a detached,
driver-owned shared clone. The journal contains, for every execution, its criterion identity and
origin, exact command, selected working directory, exit code or signal, stdout, and stderr. The
second command exits 7. The snapshot is `blocked`, the dependent ready set is empty, and the source
worktree's status is byte-identical before and after.

## Coordinated replay and amendments

`coordinated_replay_accepts_amendment_and_rejects_false_findings` creates two real Git repositories.
The witness state has `value=witness` in both repositories and the repaired state has `value=repair`
in both. The command reads the primary checkout and `$PCE_WORKTREE_1`; it is invoked exactly once in
each coordinated state. Materialization uses sibling detached shared clones below the journal's
`driver-materializations` directory. The working directory is always repository zero in graph
package order. `PCE_WORKTREES` and indexed `PCE_WORKTREE_n` variables expose the complete state.

The fail-at-witness/pass-at-repair transition records an accepted `finding-replayed` event. Folding
that event produces a durable effective criterion whose origin carries the gate identity and
finding ordinal and whose repository refs are resolved commit OIDs. `criteria-run` folds these
amendments after the immutable graph criteria, so later runs include them without changing graph
bytes. A proposed command that passes at its witness records a rejected event with reason
`witness-passed` and creates no amendment. The source gate worktrees remain clean.

## Antichain, parking, and restart derivation

`driver_dispatches_antichain_concurrently_and_parks_only_its_branch` supplies two initially-ready
packages. Each worker writes a barrier marker; package A cannot finish unless X was already spawned.
The test terminates, proving the entire ready antichain was spawned before waiting. X reports a
criterion mis-specification and is parked exactly once. A and its dependent B complete, while X's
dependent Y remains pending. The terminal outcome is distinctly `blocked` rather than `finished`.

`package_driver::tests::restart_rederives_identical_running_complete_failed_and_parked_views`
discards the first fold and reconstructs from fresh graph/journal inputs, yielding an identical
snapshot. `restarted_driver_collects_existing_outcome_without_redispatch` starts a fresh driver process with only
a running issuance and its issuance-keyed outcome on disk. It collects A, releases and runs B, and
finishes. The journal contains exactly one A dispatch, proving restart did not redispatch it.

## Commands executed

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets` (passes with pre-existing warnings)
- `cargo test --workspace` (all non-ignored tests passed)

## Scope note

The loop command uses an injected worker executable so its concurrency and recovery semantics are
deterministic under test. Existing WP4 Herdr dispatch remains additive and unchanged. A live
prime-agent/Herdr seeded-defect campaign was not run; doing so requires an interactive
Herdr-managed pane (`HERDR_ENV=1`) and authenticated prime-agent execution. The deterministic Git and
shell scenario exercises the same criterion, coordinated replay, amendment, dependency-release,
parking, and disk-only fold paths.
