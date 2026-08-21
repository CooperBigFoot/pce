# Brief: test-spawned agent shims outlive their tests and burn CPU forever

Status: GRILLED 2026-08-17 — ready to dispatch.

Supersedes the DRAFT of the same date. That draft misattributed the CPU burn to the pce
continuation and asked questions premised on that misattribution. The corrected diagnosis is
below; do not work from the draft's framing.

## The incident

Fifteen orphaned processes reparented to PID 1, the oldest alive over four days, together
consuming roughly three CPU cores. Every one traced to a **debug** binary from a test target
directory (`pce-wp11-target`, `pce-wp6-gate`, `pce-cluster-b-*`, `pce-integration-target`, …),
several from target directories already deleted — the process surviving on the unlinked inode.
Two were spawned the same day by routine `cargo test --workspace` runs. None was the installed
release binary. This is purely test-lifecycle leakage, accumulating across every suite run on
the machine. The census was killed by hand after diagnosis; nothing is leaking right now.

## The corrected mechanism

The census grepped for `pce __dispatch-continuation` and found it, but that process is the
innocent parent, not the burner.

- `execute_dispatch` blocks in a plain `child.wait()` (`src/main.rs:13546`). A blocked
  continuation consumes essentially no CPU. Production continuations are unaffected by any of
  this.
- The burner is the **test agent shim** — the `/bin/sh` script the harness puts on `PATH` in
  place of `codex`/`claude` (`tests/support/mod.rs:165` CODEX_SHIM, `:204` CLAUDE_SHIM). When a
  test wants to freeze an agent mid-run it sets `PCE_CODEX_BLOCK_FILE` / `PCE_CLAUDE_BLOCK_FILE`,
  and the shim executes `while [ ! -e "$BLOCK_FILE" ]; do sleep 0.01; done`
  (`tests/support/mod.rs:185` and `:234`). `sleep` is external, so that is ~100 fork+exec per
  second per blocked shim.
- The block file lives inside the harness `TempDir` (`tests/support/mod.rs:282`). `CliHarness`
  has **no `Drop`** that terminates anything; the only `Drop` in the support module is
  `WorkspaceFixtureDirectory` at `:49`, which just `remove_dir_all`s a path. When a test panics
  before writing the block file, the temp directory is unlinked, the block file becomes
  unreachable forever, and the shim spins until the machine reboots.
- Some tests never write the block file by design — e.g.
  `existing_dispatch_identity_is_never_overwritten` (`tests/dispatch.rs:2500`) uses a
  `never-release` path deliberately.

So: the shim burns, the continuation idles above it, and nothing owns killing either.

## Settled decisions — implement these, do not reopen

1. **No production behaviour changes at all.** This change is confined to `tests/`. In
   particular, pce must not gain any wall-clock ceiling on a dispatched agent, and
   `pce dispatch check-in` must not gain a "running too long" flag. ADR-0012
   (`docs/adr/0012-dispatch-liveness-is-observed-never-enforced.md`) decided that liveness is
   observed and never enforced by termination, after a ten-minute ceiling killed a `pr-reviewer`
   mid-review and cost a step a round for reasons unrelated to its code. `CONTEXT.md` further
   defines a check-in as reporting liveness and artifact production "and nothing else." Both
   were re-confirmed in this grill.

2. **The test harness owns the kill.** `CliHarness` gains a `Drop` that terminates the process
   groups it caused, *before* its `TempDir` is unlinked. `Drop` runs on panic, which is the case
   that produced the fifteen orphans.

3. **How the harness finds what to kill.** pce writes a per-dispatch identity sidecar under
   `<log_path>.dispatches/<issuance>.json` (`dispatch_identity_directory`, `src/main.rs:13001`;
   written by `persist_dispatch_process_identity`, called from `execute_dispatch` at
   `src/main.rs:13522`). Schema-version-1 sidecars record the owning continuation's process
   number and start identity (see `CONTEXT.md`, "Dispatch process identity"). The continuation
   is spawned with `command.process_group(0)` (`src/main.rs:13369` region, in
   `launch_dispatch_continuation` at `:13350`), so it is a process-group leader and its recorded
   process number **is** its process-group id. One signal to that negated pgid ends the
   continuation and the shim beneath it together. Read the sidecars from inside the harness's own
   temp directory only — never scan the machine, or concurrently running `cargo test` binaries
   and unrelated runs will interfere.

4. **A leak fails the suite.** Human decision, taken knowing the flake risk: after reaping, the
   harness verifies the groups are gone and fails, naming the test, if one survives. Keep the
   check scoped strictly to groups recorded by that harness's own sidecars, so another run's
   leftovers can never fail an innocent suite.

5. **The shim self-limits too.** Raise the poll interval above `sleep 0.01` and give the wait
   loop a hard iteration ceiling, so a shim that escapes reaping anyway exits by itself in
   minutes rather than days. Pick the interval and ceiling yourself; tests that intentionally
   block (e.g. `tests/dispatch.rs:2038`, `:2244`, `:2348`, `:2454`, `:2509`) must still pass, so
   the ceiling has to sit comfortably above the longest legitimate block in the suite.

## Acceptance criteria

Each is name / input / observation. The last two rows of the table are the paired
non-vacuity obligation ADR-0012 requires of any liveness-shaped check: a check that only ever
kills, and one that only ever passes, are both green against a one-sided test.

| Name | Input | Observation |
|---|---|---|
| Crashed test still cleans up | A test blocks its shim on a file that is never created, then panics | After the test binary exits, no process from that test's recorded group is alive |
| A leak is loud | A test spawns a blocked shim whose group the harness does not record, then exits | The suite fails and the failure names that test |
| The leak check is not vacuous | An ordinary dispatch test that releases its shim normally | The suite is green; no leak failure is raised |
| Escaped shim self-limits | Run the shim directly with a block file that never appears and no parent watching it | It exits on its own within its ceiling rather than running indefinitely |
| Blocked shim is cheap | A healthy blocked shim left waiting 60 seconds | Its accumulated CPU time stays under one second |
| Live runs unchanged | A dispatch whose child runs longer than any previously enforced ceiling | pce never signals it, and `pce dispatch check-in` reports the same fields as before |

"Crashed test still cleans up" and "Blocked shim is cheap" are **not assertable from inside the
test that causes them** — the first is only true after the test process exits, the second is a
measurement of the machine. Verify both from outside the run and report the measurement.

## Glossary — already written, do not redo

This grill added two rows to `CONTEXT.md` at the repo root:

- Aliases to avoid: "Release file" → "Shim block file".
- Relationships: "Dispatch continuation and the test agent shim".

Leave them as they stand unless the implementation contradicts them. No ADR was written: this
decision is easy to undo, and the one hard decision in play was already ADR-0012.

## Environment facts

- Repo `/Users/nicolaslazaro/Desktop/work/pce`, main at `a3d5852` (includes the same-day
  replan-overrule and composition-join merges).
- Merge only in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness`. Run the full suite there, then fast-forward main.
- `cargo build --release` + `./install.sh` — run `install.sh` from the **MAIN** checkout. Running
  it from `pce-integration` re-points the skills and binary symlinks at the wrong worktree. The
  install swaps `~/.local/bin/pce` under six live PCE runs, so **the human times the install**;
  do not run it unprompted.
- Reproduction / census:
  `ps -eo pid,ppid,etime,command | grep '[d]ispatch-continuation'` — anything reparented to PID 1
  from a debug target dir is a leak. Also grep for the shim's `sleep` children; they are the ones
  actually burning CPU. Expect zero of both before and after your change.
- Key code: `tests/support/mod.rs` (shims at `:165`/`:204`, poll loops at `:185`/`:234`,
  `CliHarness` at `:282`, existing `Drop` at `:49`); `tests/dispatch.rs` blocking tests listed
  above; `src/main.rs:13001` (identity directory), `:13350` (`launch_dispatch_continuation`),
  `:13487` (`execute_dispatch`), `:13546` (the blocking wait), `:9873`
  (`run_dispatch_check_in`); `crates/core/src/dispatch_check_in.rs` (report shape — expected to
  remain unchanged).
- **Explicitly out of scope:** herdr pane leakage from e2e fixtures (~26 counted the same day) is
  a separate pending ticket on Program #37. Do not fold it in.
