# Brief: a remembered merge conflict that has since been resolved is treated as a fault, so the successful case cannot dispatch

Status: READY TO DISPATCH — no grill. **A live run is stopped and the package cannot even park.** The
decisions below are evidence-resolvable; examine the evidence, decide, implement, and record what you
decided and why in your completion report and `CONTEXT.md`. Boundary: if a decision would change
ratified doctrine (an ADR, a frozen criterion), stop and report. Written 2026-08-21.

## The defect

When a package's composition recorded a conflicted join, the driver re-creates that conflict before
dispatching so the worker inherits the same tree. It requires the remembered conflict to reproduce
exactly:

```rust
if merge.status.success() || actual_paths != *expected_paths {
    record_dispatch_spawn_failure(&command.log_path, &node, issuance.sequence())?;
```
`src/main.rs:8879`

`merge.status.success()` — a **clean merge** — is the first disjunct of the failure condition. So the
driver fails when the conflict is gone. A clean compose is the successful case; it is being treated
as a fault.

The identical shape exists at the assembly site:

```rust
if merge.status.success() || actual_paths != conflicted_paths {
    bail!("assembly conflict for repository {repository} did not reproduce: expected {:?}, observed {:?}", ...)
```
`src/main.rs:5095-5097`

Fix both. Do not leave one.

The failure is also unreachable by every door the run has. It is recorded with
`scope: "dispatch-environment"`, so it does not charge the ladder and the package never parks — it
simply never spawns. There is no rung, no overrule, no `--recovery-reset` and no plan version that
touches it, and no flag disables the precondition. The only remedy available to an orchestrator is to
manufacture a merge conflict to satisfy the check, which is fabricating evidence.

## The evidence

hfx `planning/2026-08-07-close-the-seven-basin-coverage-gap`, plan version 11, ten of thirteen
packages complete. SB3's attributed recovery reset landed and worked — `recovery-spending-reset SB3`
is in the journal with its `record_sha256`, the nine-failure park is discharged and the ladder is
fresh. SB3 then dispatched twice and spawned neither time:

```
worker-dispatched    SB3  issuance 33
worker-spawn-failed  SB3  issuance 33  scope=dispatch-environment
    "conflicted join for repository hfx did not reproduce:
     expected [\"adapters/tdx-hydro/build_adapter.py\"], observed []; stdout: Already up to date."
worker-dispatched    SB3  issuance 34
worker-spawn-failed  SB3  issuance 34  (identical)
```

`Already up to date.` is the whole diagnosis. SB3's two recorded conflicts both predate SB9, and
**SB9's entire job was reconciling `adapters/tdx-hydro/build_adapter.py` across five contributors.**
With SB9 merged, SB7 is an ancestor and the merge is clean. The prerequisite work succeeded, and
succeeding is what broke the dispatch.

Nothing in the tree is wrong. Nothing was spent — the run remains at approximately EUR 3.00 with no
cloud footprint.

## What to change

**Decisions delegated to you:**

1. **What a vanished conflict means.** It means an intervening completion resolved it, which is the
   outcome the graph's dependency edges exist to produce. Recommendation: when the recorded conflicted
   paths do not reproduce **and the merge is clean**, recompose from the clean merge and dispatch,
   journalling that the conflict no longer occurs and naming the completion that resolved it — the
   run should record that its own dependency ordering worked. Decide and justify; this is the
   substantive half of the brief.
2. **What remains a genuine failure.** Reserve the spawn failure for an observed conflict set that is
   **different and non-empty** — the tree conflicts, but not where the record says. That is a real
   surprise about composition and must still refuse. Make the condition express that distinction
   rather than testing `!=` against the remembered list.
3. **Whether the assembly site behaves identically.** `:5095` has the same disjunct and the same
   consequence, one phase later. Decide whether the resolution is shared code or two call sites
   agreeing, and say which you chose.
4. **What the worker is told.** If the driver recomposes from a clean merge, the brief must not
   describe a conflicted join the worker will not find. Check the conflicted-join section of
   `compose_package_worker_brief` and fix the wording alongside the behaviour, so a worker is not sent
   after a conflict that no longer exists. This is the second time that section has misdescribed a
   join to a worker.

## The pattern worth naming

The hfx orchestrator observes that this is the **third distinct block in one vision from state
recorded at one moment being asserted against a changed world** — after the criterion fixture pinned
to a previous campaign's numbers, and the park counter that could not see a plan advance. Each time,
a recording that was true when written became a precondition that a later, correct change falsified.

Consider whether that belongs in `CONTEXT.md` as a named hazard for authors of driver preconditions:
a remembered observation may be used to *explain* a state, but it must not be required to still hold
unless the thing it records is something the run is asserting does not change. Say what you concluded;
if you think it does not generalise, say that instead.

## Tests

The decisive test is SB3's situation: a package whose recorded conflicted paths no longer conflict
because a later completion resolved them must **dispatch**, and the journal must say the conflict no
longer occurs. Pair it with the two negatives — a package whose recorded conflict still reproduces
exactly must compose and dispatch as today, and a package that conflicts on a *different* non-empty
path set must still refuse to spawn. The trio is what makes them meaningful; the first alone would
pass by deleting the check.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `d9ff77a`, pushed and clean.
- Installed binary digest `f83d4548…`. `~/.local/bin/pce` is a symlink into the main checkout's
  `target/release/pce`, so **do not** `cargo build --release` there and do not run `./install.sh` —
  that is a fleet install across five live runs. The supervisor owns installation.
- Merge and **edit** in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness`. Full suite there: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets`, `cargo test --workspace`. The suite is currently green
  with zero failures — if you see a failure, it is yours. Known parallel-load flakes in
  `tests/dispatch.rs` (`gate_execution_echoes_large_input_without_deadlock`,
  `gate_execution_drains_three_pipes_concurrently`) — verify in isolation before blaming a change.
- **Two other briefs are in flight and both touch this repository**:
  `briefs/hardening-invalidation-charging-brief.md` (`crates/core/src/package_driver.rs`
  `charged_failure_count`, and `src/main.rs:3677` `refuse_dirty_source_repositories`) and
  `briefs/composition-retry-storm-brief.md`. Your region is `src/main.rs:8840-8890` and
  `src/main.rs:5067-5100` plus the conflicted-join section of
  `crates/core/src/package_worker.rs`. Stay inside it; the supervisor merges.
- Key code: `src/main.rs:8879` (dispatch-time reproduction check and
  `record_dispatch_spawn_failure`), `src/main.rs:5095-5097` (assembly-time equivalent), and the
  conflicted-join section of `compose_package_worker_brief` in `crates/core/src/package_worker.rs`.
- Evidence: hfx's `driver-journal.jsonl` — `worker-spawn-failed` for SB3 at issuances 33 and 34,
  both `scope: "dispatch-environment"`, and the `recovery-spending-reset` for SB3 immediately before
  them. Full report: `orchestrator-feedback/2026-08-21-close-the-seven-basin-coverage-gap.md`.

## The waiting consumer

hfx, plan version 11, ten of thirteen packages complete. SB3 is the only thing between the run and
SB4/SB5, and it cannot spawn. SB3's recovery ladder is fresh and its authorized bounded paid campaign
(approximately EUR 1.70 marginal, against a EUR 40.00 ceiling with SB6's cost gate live) has not
started. The ladder is not at risk while this is unfixed, because the failure is environment-scoped —
but neither is any progress.

A healthy first pass: SB3 dispatches on a plain relaunch with no new plan version and no further
human record, the journal states that the recorded conflict no longer occurs and names SB9 as the
completion that resolved it, and SB3's worker receives a brief that does not promise it a conflicted
join.
