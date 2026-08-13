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
- `cargo test --workspace` (all non-ignored tests passed; the known concurrent large pipe-drain test failed twice in full-suite runs and passed alone, then the complete suite passed on the final rerun)

## Scope note

The loop command uses an injected worker executable so its concurrency and recovery semantics are
deterministic under test. Existing WP4 Herdr dispatch remains additive and unchanged. A live
prime-agent/Herdr seeded-defect campaign was not run; doing so requires an interactive
Herdr-managed pane (`HERDR_ENV=1`) and authenticated prime-agent execution. The deterministic Git and
shell scenario exercises the same criterion, coordinated replay, amendment, dependency-release,
parking, and disk-only fold paths.


## Follow-up: environment preparation

The driver accepts repeatable `--prepare NAME=COMMAND` options beside `--repository NAME=PATH` on
`driver-run`, `criteria-run`, and `replay-finding`. Each command runs through `/bin/sh -c` with its
working directory set to that repository's detached driver-owned clone. It runs once for the
criteria materialization and once for each coordinated replay side. No option preserves prior
behavior.

Every attempted preparation appends `environment-preparation-executed`, carrying the package,
materialization label, repository, exact command, explicit `succeeded`/`failed` outcome, working
directory, exit status, stdout, and stderr. A nonzero status folds to the distinct terminal state `environment-preparation-failed` and
no `criterion-executed` or `finding-replayed` event is appended. Tests cover criteria preparation
failure, four successful two-repository/two-state replay preparations, and repair-side preparation
failure before either proposed-command execution.

The worker brief now states: “Commit all completed work before reporting done. Criteria run against
the resulting commit, not the working tree, so uncommitted work will not be judged.”

## Real Prime Agent and Herdr 0.7.1 drive

Run root: `/tmp/pce-wp7-real-prepare-1786612230`. Isolated PCE binary:
`/tmp/pce-wp7-follow-target/debug/pce`. Installed `herdr --version`: 0.7.1. Installed
`prime-agent --version`: 0.7.2.

The driver invocation was:

```sh
/tmp/pce-wp7-follow-target/debug/pce package driver-run --graph /tmp/pce-wp7-real-prepare-1786612230/graph.json --journal /tmp/pce-wp7-real-prepare-1786612230/driver-events.jsonl --repository demo=/tmp/pce-work-package-worktrees/pce-2dbcea68b4a5a4654db1581a94c7/00-demo --prepare demo=touch .prepared -- /tmp/pce-wp7-real-prepare-1786612230/herdr-worker-adapter.sh
```

The adapter invoked actual `pce dispatch package`, which asked actual Herdr 0.7.1 to create the
worktree and start `pce package agent`; that command piped the composed brief to actual
`prime-agent --no-session --thinking minimal -p`. Completion was observed only from the atomically
published WP4 result file and package outcome, never from Herdr.

Observed sequence:

1. Driver appended `worker-dispatched` for T1 issuance 1.
2. Herdr created `/tmp/pce-work-package-worktrees/pce-2dbcea68b4a5a4654db1581a94c7/00-demo` and started agent `pce-2dbcea68b4a5a4654db1581a94c7`.
3. Prime Agent created and committed `known.txt` as commit
   `8035b4f67565d9ee551f5695cec4da13e58a93de` with contents `known`, then wrote
   `{"outcome":"done"}`.
4. WP4 wrote `.pce/package-results/T1/1.json`: exit 0, artifact present, duration 13641 ms.
5. Driver appended `worker-done`, cloned committed HEAD, ran `touch .prepared`, and recorded its
   zero exit as environment preparation.
6. The criterion `test -f .prepared && test "$(cat known.txt)" = known` exited zero in that same
   clone and was recorded.
7. Driver appended `gate-finished` with its no-findings gate and `package-completed`; final outcome
   was `finished`.

Recorded artifacts:

- `/tmp/pce-wp7-real-prepare-1786612230/vision.md`
- `/tmp/pce-wp7-real-prepare-1786612230/graph.json`
- `/tmp/pce-wp7-real-prepare-1786612230/driver-events.jsonl`
- `/tmp/pce-wp7-real-prepare-1786612230/dispatch-events.jsonl`
- `/tmp/pce-wp7-real-prepare-1786612230/herdr-dispatch.json`
- `/tmp/pce-wp7-real-prepare-1786612230/package-outcomes/T1/1.json`
- `/tmp/pce-wp7-real-prepare-1786612230/.pce/package-results/T1/1.json`
- `/tmp/pce-wp7-real-prepare-1786612230/herdr-worker-adapter.sh`
- `/tmp/pce-wp7-real-prepare-1786612230/seed-repo`
- `/tmp/pce-work-package-worktrees/pce-2dbcea68b4a5a4654db1581a94c7/00-demo/known.txt` and its committed Git history

The native driver currently accepts a worker argv rather than directly composing WP4 dispatch, so a
small adapter was required to join its synchronous worker contract to WP4's asynchronous Herdr
start/result contract. The spawned implementation worker was actual Prime Agent through actual
Herdr; the adapter contained no implementation logic. Consolidating environment preparation into
tracked contract `install`/`preflight` was not attempted because those fields contain prose rather
than executable commands.
