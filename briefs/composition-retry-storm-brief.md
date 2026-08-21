# Brief: a composition-infrastructure failure retries forever and the environment budget never fires

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-20, while the defect was live.

## What happened

The bluesmith run `planning/2026-07-29-signal-bearing-dudh-warm-window` advanced to plan version 10
and immediately entered a retry storm on package W5. **53 dispatches, issuances 44 through 96, every
one failing with a byte-identical reason**, and no worker ever ran:

```
package composition infrastructure failed in stopwatch: failed to inspect unsuccessful merge of
package W3 commit ae8a8a5feeaf8930e1c99dbeb076bf41ca6390ff: fatal: cannot change to
'planning/2026-07-29-signal-bearing-dudh-warm-window/.pce/compositions/26983-759701599de03701-1787215130694921000':
No such file or directory
```

The run's own `recovery-configured` record sets `{"retry_attempts":1,"local_patch_attempts":1,
"environment_failures":6}`. Counting `worker-environment-failed` per plan version across the whole
journal gives `{1: 5, 2: 2, 5: 2, 6: 2, 9: 1, 10: 53}`. **The limit is 6 and plan version 10 spent
53.** This is WP14's guarantee — "a repeated environment failure has to stop" — not holding for this
failure class. WP14's brief records a prior instance that repeated 4,265 times; this is the same
pathology in a different path.

## Three defects, in priority order

### 1. The environment-failure budget does not cap this class

- Evidence: the per-plan-version census above, against the recorded limit of 6. The journal is
  876 KB and holds 825 events, most of them this loop.
- Every failure is recorded as `worker-environment-failed`, so either the charge is not applied for
  failures raised during composition (before any worker exists), or the counter resets per issuance
  rather than accumulating within a plan version.
- **This is the part that must be fixed first.** Whatever else is true about the underlying git
  problem, a driver that cannot make progress must stop rather than spend the machine.
- Decision delegated: whether the cap counts per plan version, per package, or per run. Recommendation:
  keep the existing accounting and make sure the composition path actually charges it — the smallest
  change that restores the guarantee. State what you found: if the charge was applied and the cap
  simply did not refuse, that is a different bug and worth saying so plainly.

### 2. A composition failure that no retry can change is retried anyway

- The reason string is byte-identical across all 53 attempts. Nothing about re-dispatching an
  unchanged package against an unchanged base can alter it.
- Change: stop on the **second identical** environment-classified failure for the same package, park
  with that reason, and let the human rule. This is the same principle as the repeated-worker-blocker
  park landed in `5360580`, applied to the environment class rather than the worker-reported one, and
  the skill already tells supervisors to stop after the second identical environment failure — the
  binary should not need a human to notice.
- Confidence that the rule is right: high. Decision delegated: whether "identical" means the exact
  reason string or a normalised form.

### 3. The conflicted-join inspect path assumes its worktree still exists

- Location: `src/main.rs:3374-3387`. Composition creates a detached worktree under
  `<vision-dir>/.pce/compositions/<pid>-<label>-<nanos>` via `git -C <source> worktree add`, merges
  each dependency oid into it, and on a failed merge runs
  `git -C <worktree> diff --name-only --diff-filter=U` to recover the conflicted paths. When that
  directory is absent the code `bail!`s with the message above.
- That path exists to hand a conflicted join to the dependent worker as its first task
  (`79a13a7 feat: a conflicted join is the dependent worker's first task`). Instead of reaching that
  hand-off, the run died on missing infrastructure and retried.
- A missing composition worktree is a **defect**, not an environment condition. It should emit a
  typed event naming the absent path and park, never re-enter the dispatch loop.

## The mechanism: the loop was not retrying, it was replaying

Established after the first draft of this brief, and it changes the fix. **All 53 failures name the
identical composition path**, character for character:

```
.pce/compositions/26983-759701599de03701-1787215130694921000
```

That path is `<pid>-<label>-<nanos>` computed at creation time from `std::process::id()` and
`SystemTime::now().as_nanos()` (`src/main.rs:3334-3342`). Two independent attempts cannot produce the
same nanosecond, and two different driver processes cannot produce the same pid. Verified:
`grep -o "compositions/[0-9]*-" driver-journal.jsonl | sort -u` yields exactly **one** pid across the
whole journal, and `26983` is not the pid of any driver process observed today.

So the driver was not composing 53 times and failing 53 times. It was **re-emitting a single stale
composition failure**, from a process that died long ago, whose directory was cleaned up with it.
Nothing a retry could do would change the outcome, because no retry was happening — which is exactly
why the reason string never varied.

The corroborating experiment ran itself: when the orchestrator relaunched the driver as a new process
at issuance 97, composition **succeeded immediately** and the run moved on to a genuine worker failure
(`Exited { code: ExitCode(1) }`), then dispatched issuance 98 with a live worker pane. A fresh process
performed a real composition; the old one never did.

So the highest-value question is not "why did the directory vanish" — it vanished with its owning
process, correctly. It is **why a package that has a recorded composition failure re-dispatches
against that record instead of composing afresh**, and where that stale attempt is held. Start at the
composition-failure path and at how `package-composition-failed` is folded into driver state on
resume; the journal holds 59 `package-composition-failed` events against 66
`worker-environment-failed`.

Prior leads, retained because they may still matter for cleanup hygiene, all observed 2026-08-20:

- `<vision-dir>/.pce/compositions/` is **empty** — every composition worktree directory is gone.
- The `stopwatch` repository has **51 registered worktrees**, most marked `prunable` in
  `git worktree list`, i.e. registrations whose directories no longer exist.
- The composition worktree is registered in `stopwatch` but its directory lives under the
  **bluesmith** vision directory. Any cleanup that reasons about one repository's tree while the
  directory belongs to another vision's path is a candidate.
- `DriverMaterialization`'s `Drop` (`src/main.rs:6426-6429`) does `remove_dir_all` on its own root,
  and `remove_clean_worktree` / `worktree remove --force` appear at `:3441`, `:4322`, `:6322`. One of
  these, or a `git worktree prune` triggered as a side effect of another git command in the same
  repository, is the likely remover. Establish which before changing behaviour.

Given the replay mechanism above, the primary fix is that a composition attempt must be **performed,
not remembered**: a recorded composition failure may explain a past attempt but must never stand in
for a new one, and a path naming a dead process must never be read as live infrastructure. A cheap
invariant that would have refused this outright: before using a composition worktree path, require
its pid component to be the current process. Recovering conflicted paths without a checkout at all
(`git merge-tree` against the two oids) is still worth costing as a way to delete the class rather
than guard it, but it is secondary to the replay.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `5360580`, pushed.
- Merge only in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness`; full suite there (`cargo fmt --check`,
  `cargo clippy --workspace --all-targets`, `cargo test --workspace`). Known parallel-load flake:
  `tests/dispatch.rs gate_execution_echoes_large_input_without_deadlock`.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh` — that
  symlink is a fleet install across four live runs. The supervisor owns installation.
- Key code: `src/main.rs:3325-3450` (composition worktree lifecycle, merge, conflicted-path
  inspection, removal); `crates/core/src/package_recovery.rs:39-110` (`EnvironmentFailureLimit`,
  default 6); the environment-failure charge path in `crates/core/src/package_driver.rs` and
  `src/main.rs`.
- Evidence to read first:
  `/Users/nicolaslazaro/Desktop/work/bluesmith/planning/2026-07-29-signal-bearing-dudh-warm-window/driver-journal.jsonl`
  — the storm is issuances 44-96 at the tail. `git worktree list` in
  `/Users/nicolaslazaro/Desktop/work/stopwatch` shows the 51 registrations.

## Tests

Every fix ships a test. At minimum: a driver that receives a composition-infrastructure failure twice
for the same package stops instead of re-dispatching, and the environment-failure limit refuses at
its configured count for failures raised during composition — the case this run proves is currently
unguarded. Prefer tests asserting the emitted journal events over unit tests of internal helpers.

## The waiting consumer

bluesmith `planning/2026-07-29-signal-bearing-dudh-warm-window`, plan version 10, W5 at issuance 96,
driver exited. W1-W4 and W7 are complete and carried; W5 re-dispatches and W6 unblocks behind it once
composition works. A healthy first pass: W5's composition either produces a base or hands the
conflicted join to the worker as `79a13a7` intends, and in the failure case the driver parks after
the second identical failure with the reason in the journal — not 53 dispatches later.
