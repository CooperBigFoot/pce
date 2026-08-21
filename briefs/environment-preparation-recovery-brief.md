# Brief: a failed environment preparation is a dead end, so the retry that fixes it makes the journal unreadable

Status: READY TO DISPATCH — no grill. **A live run has written a journal its own reader rejects, and
every supervisor lever is closed.** The decisions below are evidence-resolvable; examine the evidence,
decide, implement, and record what you decided and why in your completion report and `CONTEXT.md`.
Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion), stop and report.
Written 2026-08-21.

## The defect

`DriverPackageState::EnvironmentPreparationFailed` (`crates/core/src/package_driver.rs:842`) is set in
exactly one place and cleared in none:

```rust
// package_driver.rs:1874-1879 — the only write
if !succeeded {
    *state = DriverPackageState::EnvironmentPreparationFailed {
        repository: repository.clone(),
        command: command.clone(),
    };
}
```

`grep EnvironmentPreparationFailed` over the crate and the binary returns four hits: the variant
declaration, one classification arm at `:1343`, this assignment, and a test. **Nothing transitions out
of it.**

The `EnvironmentPreparationExecuted` arm opens with:

```rust
// package_driver.rs:1860
if !matches!(state, DriverPackageState::Judging { .. }) {
    return Err(PackageDriverError::EventAfterTerminal { package: package.clone() });
}
```

So once a preparation command fails, **a later successful run of that same preparation command is
rejected as an event after a terminal state**. The event that records the recovery is unrepresentable
in the state machine that must replay it. The driver writes it — the write path has no such guard —
and then cannot read what it wrote.

The consequence is not a failed command. It is that the whole journal becomes underivable from record
one, so `driver-status`, `driver-run` and `criteria-run` all refuse, and every recorded proof in that
journal becomes unreadable.

## The evidence

taqsim `planning/2026-08-21-conservation-by-declared-resolution`, 59 records.

```
39: environment-preparation-executed  TQ1  taqsim  uv sync   outcome=failed     exit 1
...
57: environment-preparation-executed  TQ1  taqsim  uv sync   outcome=succeeded  exit 0
58: environment-preparation-executed  TQ1  incidence  cargo fetch  outcome=succeeded  exit 0
59: driver-aborted  "failed to derive predecessor plan before advancing:
                     driver event occurs after package `TQ1` reached a terminal state"
```

Record 39 failed because the `incidence` dependency pin was unresolvable. The operator published the
dependency, which fixed it. Record 57 is the *same command succeeding* — the remedy, correctly
performed and correctly journalled. It is what makes the journal unreadable.

Bisected directly, which is the decisive fact:

```
head -56 journal -> driver-status derives cleanly
head -57 journal -> failed to derive driver state
```

Record 57 alone. TQ1 sits in `EnvironmentPreparationFailed` from record 39, and the guard at `:1860`
admits only `Judging`.

**The journal is not corrupt.** Records 57 and 58 are true, were written by the driver, and describe
what actually happened. This is a read-path defect against a legal history: the write path accepts a
transition the read path denies. No journal edit is required or wanted, and the orchestrator was right
to refuse to edit or truncate it.

Every exit is closed, which is why this is a brief and not a ruling: `driver-run` on the predecessor
graph appends nothing and exits blocked; on the successor it aborts; `criteria-run --package TQ1`
refuses with the same error; a further mechanical freeze meets the same predecessor-derivation refusal;
and `--recovery-reset` does not apply, because TQ1 is not recovery-parked and the record's own
validation derives a snapshot over these same records.

## What to change

**Decisions delegated to you:**

1. **What a successful preparation after a failed one means.** It means the environment was repaired
   between attempts, which is the ordinary case and the reason a preparation command is re-runnable at
   all. Recommendation: accept `EnvironmentPreparationExecuted` with a succeeded outcome while the
   package is in `EnvironmentPreparationFailed`, and return it to `Judging { issuance }` — the state it
   held before the failed preparation. Decide whether the issuance is carried on the failed state or
   recovered another way, and say which. This is the substantive half of the brief.
2. **Whether `EnvironmentPreparationFailed` should be terminal at all.** It is currently terminal only
   by omission — nothing declares it so; it simply has no outgoing edge. Decide whether it is a
   blocked-but-recoverable state with an explicit set of admissible successors, and make that explicit
   rather than emergent. A state that is terminal because nobody wrote the arm is the shape of this
   defect.
3. **Whether the write path and the read path can disagree at all.** The driver appended records 57 and
   58 without consulting the guard that later rejects them. Whatever you decide above, a fold that can
   reject the driver's own append is a standing hazard, not a one-off. Recommendation: make the
   append path validate against the same fold before writing, so an inadmissible event is refused at
   write time with a diagnosable error rather than discovered on the next read. Say what you concluded;
   if you judge that too large for this brief, say so and scope it to a follow-up.
4. **What a reader sees.** `failed to derive driver state / driver event occurs after package TQ1
   reached a terminal state` names neither the record index nor the state TQ1 was actually in. A
   supervisor cannot bisect from it — I had to truncate the journal by hand to find record 57. The
   error must name the offending record and the state that rejected it.

## Tests

The decisive test is the sequence above: a package whose preparation command fails, then succeeds on a
later attempt, must derive cleanly and be dispatchable, with no journal edit. Pair it with the
negative — a preparation that fails and is never retried must still hold the package blocked exactly
as today, and must not silently become runnable. Add a third that folds the taqsim shape end to end:
fail, unrelated packages complete, succeed, and assert the full journal derives from record one. If
you take decision 3, add a fourth asserting the append path refuses an inadmissible event at write
time.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `15a7e2d`, pushed and clean.
- Installed binary digest `e8d5d373…`. `~/.local/bin/pce` is a symlink into the main checkout's
  `target/release/pce`, so **do not** `cargo build --release` there and do not run `./install.sh` —
  that is a fleet install across five live runs. The supervisor owns installation.
- **Work in `/Users/nicolaslazaro/Desktop/work/pce-envprep`, branch `integration/envprep-recovery`**,
  a dedicated worktree created for you at `15a7e2d`. Do not edit the main checkout and do not edit
  `pce-integration`. Full suite in your worktree: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets`, `cargo test --workspace`. The suite is green at `15a7e2d`
  with zero failures — if you see a failure, it is yours. Known parallel-load flakes in
  `tests/dispatch.rs` (`gate_execution_echoes_large_input_without_deadlock`,
  `gate_execution_drains_three_pipes_concurrently`) — verify in isolation before blaming a change.
- Report your branch tip. Do not merge, fast-forward, or install.
- Key code: `crates/core/src/package_driver.rs:842` (the variant), `:1343` (its classification),
  `:1853-1880` (the `EnvironmentPreparationExecuted` arm and its guard), `:1798-1810`
  (`WorkerDone` → `Judging`, the state to return to), and the append path in `src/main.rs` that writes
  `EnvironmentPreparationExecuted` without consulting the fold.
- Live evidence you can read directly:
  `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-21-conservation-by-declared-resolution/driver-journal.jsonl`.
  Do not modify it. `head -56` derives; `head -57` does not.
- Full report: `orchestrator-feedback/2026-08-21-conservation-by-declared-resolution.md`.

## The waiting consumer

taqsim `2026-08-21-conservation-by-declared-resolution`. IQ1 and IQ2 are complete with paired-proof
amendments and are pushed. TQ1's worker attempt survives on
`pce/2026-08-21-conservation-by-declared-resolution/TQ1/attempt-3` and has never been judged. The run
cannot take a single step until this lands, and the operator has been given no lever that reaches it.

A healthy first pass: the existing 59-record journal derives cleanly with no edit, `driver-status`
reports TQ1 judging, and a plain relaunch judges TQ1's attempt against its criteria.
