# Brief: a package is charged for failing to revert another package's repair, and the counter's own doc comment says it should not be

Status: READY TO DISPATCH — no grill. **This parked a package that had already completed.** The
decisions below are evidence-resolvable; examine the evidence, decide, implement, and record what you
decided and why in your completion report and `CONTEXT.md`. Boundary: if a decision would change
ratified doctrine (an ADR, a frozen criterion), stop and report. Written 2026-08-21.

Two workstreams. They are independent; do both.

---

## Workstream A — hardening invalidation must not charge the package that did not cause it

### The defect

`charged_failure_count` (`crates/core/src/package_driver.rs:2571`) opens with:

```rust
/// Count only worker-reported or criterion-judgement failures attributed to package work.
```

and then counts three event kinds:

```rust
DriverEvent::WorkerFailed { .. }
    | DriverEvent::PackageFailed { .. }
    | DriverEvent::PackageHardeningInvalidated { .. }
```

`PackageHardeningInvalidated` is neither worker-reported nor a criterion judgement. It is the driver
reporting that it could not construct a counterfactual revert of a **different** package's credited
repair, in order to re-prove that repair still hardens something. The doc comment and the
implementation contradict each other, and the implementation charges the wrong package.

The failure is also deterministic. A revert that conflicts will conflict again for a fresh worker
against the same lineage, so the retry rung cannot help by construction — yet the ladder spends it,
then spends local-patch, then replans. A package can be driven from `package-completed` to
`recovery-parked` without any worker doing anything wrong.

Note that the driver **already has** a non-charging shape for exactly this situation:
`DriverEvent::RepairCreditStale` (`:398`) with
`StaleRepairCreditReason::CounterfactualUnconstructable` (`:316-326` — "The repair hunk cannot be
reverted from the composed tree although its criterion passes"). It is not counted by
`charged_failure_count`. The driver emitted the charging event where the non-charging one describes
the situation precisely.

### The evidence

pourpoint `planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`,
plan version 19, 17 of 18 packages complete.

GD6 ran its full gate re-proof, emitted `package-completed`, executed all six assembly criteria, and
was then invalidated:

```
package-hardening-invalidated  GD6  hardened_package=GD15  repository=pourpoint
    gate=package-gate-23-1  finding=0  repair_ref=7b68223fdf2228a706517e2d1dd0cb53706c5a28
    detail: Auto-merging scripts/released_wheel_proof.py
            CONFLICT (content): Merge conflict in scripts/released_wheel_proof.py
            error: could not revert 7b68223... fix: gate direct worker reads from production trace
recovery-parked  GD6  "recovery spending exhausted after 3 attributable failures in this journal
                       recovery epoch; supply an attributed --recovery-reset record to open a fresh ladder"
```

The repair `7b68223` is credited to **GD15**, not GD6. It lives in `scripts/released_wheel_proof.py`,
a file GD6 does not touch and whose divergence comes from twelve subsequent commits. The event was
emitted twice, and the recorded park attempts show the retry rung re-running the identical
unconstructable revert and producing the identical stderr.

The park record renders the charge as a criterion named
`gate:package-gate-23-1:finding:0:restore-provability` with command
`restore repair 7b68223… credited to GD15` (`:2640-2650`) — the record itself names another package
as the subject of the failed act while charging GD6 for it.

### What to change

**Decisions delegated to you:**

1. **Whether `PackageHardeningInvalidated` should be charged at all, and to whom.** It is not a
   judgement about the package's work. Recommendation: stop counting it in `charged_failure_count`,
   matching that function's stated contract, and make the counter's doc comment and its match arms
   agree — the previous defect in this same function was the same shape of disagreement. Decide
   whether an unconstructable counterfactual should instead be recorded as `RepairCreditStale` with
   `CounterfactualUnconstructable`, which already exists and is already uncounted. This is the
   substantive half of the workstream.
2. **What happens to the package.** If it is not charged, GD6's state after invalidation must still
   be defined. It had already reached `package-completed`. Say what you chose and why; do not leave a
   package that can neither complete nor park.
3. **Whether the retry rung should be offered for a deterministic driver-side failure.** A revert
   conflict is not worker-sensitive. If any charging path survives your decision in (1), a rung that
   cannot change the outcome should not be spent. Say what you concluded rather than adding a
   mechanism by reflex.
4. **What the operator sees.** The park record names GD15's repair as the failed act while charging
   GD6. Whatever you decide, a reader must be able to tell whose failure it is and what would
   resolve it — a stale repair credit needs an owner.

### Tests

The decisive test is the sequence that happened: a package reaches `package-completed`, a remembered
repair credited to a **different** package fails to revert against the rebuilt lineage, and the
completed package must not lose a recovery rung for it. Pair it with the true-positive — a package
whose own work genuinely fails a criterion must still be charged and must still park exactly as
today. Add a third asserting `charged_failure_count` counts exactly the event kinds its doc comment
names, so this function's contract stops drifting from its body.

---

## Workstream B — the dirty-source guard's remaining breadth

`d9ff77a` scoped the guard to exclude the active vision directory and made the message name offending
paths. The pourpoint orchestrator confirmed both landed, and reports the remaining question as still
open: a repository-root documentation edit still aborts a run.

Observed after the fix, with the driver otherwise ready to launch:

```
driver-aborted: source repository `pourpoint` is dirty outside the driver vision directory;
driver-run requires these paths to be clean while preserving /Users/…/planning/2026-08-07-…-row-seam:
 M CONTEXT.md
?? docs/codex-non-interactive.md
```

**Decisions delegated to you:**

1. **Whether a dirty source tree can affect a run at all.** Establish this from the code rather than
   assuming it either way: determine whether package composition, criterion execution and assembly
   read only committed refs, or whether any path reads the source worktree directly. The answer
   decides everything below, and it is a question about the code, not a preference.
2. **If nothing reads the worktree,** the guard is protecting reproducibility that is not at risk, and
   refusing a launch over an untracked note is a false positive that costs a run. Decide whether it
   becomes a warning that names the paths and proceeds, or stays refusing. Recommendation: warn and
   proceed, because the cost of a false positive here is a stopped run and the cost of a false
   negative is nothing you have been able to demonstrate.
3. **If something does read the worktree,** name it in `CONTEXT.md` — the guard is then load-bearing
   and the orchestrators need to know precisely what it protects, because right now nobody can tell.

### Tests

Whatever you decide, pin it: a source repository dirty only outside the vision directory must
produce the behaviour you chose, and the test must assert the reason, not just the exit status.

---

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
- Key code for A: `crates/core/src/package_driver.rs:2571` (`charged_failure_count`), `:316-326`
  (`StaleRepairCreditReason`), `:398` (`RepairCreditStale`), `:727` and `:2640-2650`
  (`PackageHardeningInvalidated` and how it renders as recovery evidence), and the emit site in
  `src/main.rs:5681`.
- Key code for B: `src/main.rs:3677` (`refuse_dirty_source_repositories`) and
  `tests/package_composition.rs` (`driver_refuses_an_already_dirty_source_before_composition`,
  `relative_launch_allows_its_untracked_vision_directory_in_the_source`).
- Evidence: pourpoint's `driver-journal.jsonl` in the vision directory above — two
  `package-hardening-invalidated` events for GD6 and the `recovery-parked` record carrying the
  attempt ladder.

## The waiting consumer

pourpoint, plan version 19, 17 of 18 packages complete, stopped with GD6 parked. GD6 had already
completed. A `--recovery-reset` for GD6 would reopen the ladder and then reproduce the identical
invalidation, so the human door does not help here and must not be spent on it — which is why this is
a brief and not a ruling.

A healthy first pass: a package that completes and is then hit by an unconstructable revert of
another package's credited repair keeps its recovery ladder, and the record names the package that
owns the stale repair.
