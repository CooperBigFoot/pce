# WP14 — a repeated environment failure has to stop

A package whose environment fails is returned to pending and charged nothing, so it is dispatched
again immediately, fails the same way, and is returned to pending again. One drive did this **4,265
times** before it was killed, burning 53 minutes of CPU and writing a megabyte of journal in which
two events alternate forever.

The rule that produced it is correct and must survive: an outcome not attributable to the work does
not spend the work's repair budget. What is missing is the backstop.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `main` at commit `82bbdae` in a **new worktree**. It carries the whole harness including
composition, assembly, gate fault isolation and pane cleanup.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in `/Users/nicolaslazaro/Desktop/work/pce` replaces the binary those runs
invoke, and a `git checkout` there rewrites the skills they read.

1. Work in your own git worktree, never the main checkout. Keep your own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.
4. **Do not touch `/Users/nicolaslazaro/Desktop/work/RivRetrieve` or `/tmp/pce-work-package-worktrees`.**
   They hold evidence from the first real run. Use `PCE_WORK_PACKAGE_WORKTREE_ROOT` for fixtures.
5. **Any drive you run must be bounded.** This package exists because an unbounded one was left
   spinning; do not repeat it. Watch it, and kill it yourself if it does not terminate.

## The tools, stated rather than assumed

**herdr** — installed 0.7.1, checkout 0.8.0 at `/Users/nicolaslazaro/Desktop/thirdparty/herdr`.
**The installed binary is the authority.** It does not recognise prime-agent; never consult it for
completion.

**prime-agent** — installed 0.7.2, `prime-agent -p`, reads piped stdin, no `--output-schema`.

## What already exists

- `crates/core/src/package_recovery.rs` — the ladder: retry, local patch, park, with named limits
  and `charged_failure_count`.
- `crates/core/src/package_driver.rs` — `DriverEvent::WorkerEnvironmentFailed`, which folds the
  package back to pending and charges nothing. Also `EnvironmentPreparationFailed`, a separate
  state for a `--prepare` command that failed.
- `crates/core/src/run_render.rs` — `pce package render`.

## The evidence

`/private/tmp/claude-501/-Users-nicolaslazaro-Desktop-work-pce/a49caab8-cea2-441a-b7a4-515bea358c8c/scratchpad/wp12-loop-evidence.jsonl`

Read it. Every record after the first few is one of these two, alternating:

```json
{"event":"worker-dispatched","package":"C","issuance":N}
{"event":"worker-environment-failed","package":"C","issuance":N,
 "reason":"package dispatch stopped without a successful required artifact: Exited { code: ExitCode(1) }"}
```

The fixture's worker was broken, which is ordinary. What is not ordinary is that the driver never
concluded anything about it.

---

## Why the current rule is right, and what it is missing

This project already paid for the opposite mistake. Four runs died at a defect cap of three with an
escalation verb that admission never read — a stop that could not be lifted. The correction was that
the limit is a **spending limit**, honestly named and generous, and that an upstream cause does not
charge the role reporting it. Both halves of that stand.

But "does not charge the work's budget" was read as "is not counted at all", and an outcome that is
never counted can recur without end. A run that cannot stop is not more generous than one that stops
too early; it is a different way of failing, and it costs a core and a human's attention instead of a
round.

The distinction to preserve is between **a transient environment fault**, which should cost the work
nothing and be retried freely, and **a persistent one**, which is a fact about the environment that
no number of further attempts will change.

## What must be true when you are done

### Repeated identical environment failure terminates

A package whose environment fails the same way, repeatedly, stops. Where you draw that line and what
"the same way" means are yours to decide and to state — the reason string is available, and so is
whatever else you judge to identify a recurrence rather than a coincidence.

State the rule in one sentence in your report, and make the bound nameable and adjustable in the same
spirit as the existing retry and local-patch limits, rather than a constant buried in a function.

### The stop is a stop, not a failure of the work

A package stopped this way did not fail its criteria and its worker did not do anything wrong. It
must be distinguishable in the journal from a criterion failure, from a worker failure, from a
`mis-specified` graph fault, and from a package parked by exhausting the recovery ladder. A human
reading the journal cold must be able to tell that the environment never became workable, and see
what it said each time.

Whether it consumes the recovery ladder's budget is your call — but say which you chose and why. The
existing rule that an environment failure charges nothing was written for the transient case, and it
is exactly the rule this package is qualifying.

### An intermittent failure still recovers

A package whose environment fails once, or twice, and then succeeds must complete normally with no
budget spent. Bounding the pathological case must not break the case the original rule was written
for. This is the criterion most likely to be lost while fixing the other one.

### Nothing else changes

`EnvironmentPreparationFailed` — a `--prepare` command that failed — already has its own terminal
state and is not part of this. The recovery ladder's rungs, its limits, and the test proving that
removing it changes what the driver dispatches all stay exactly as they are.

### It survives a restart

The count, whatever it is, re-derives from the journal alone. A driver killed and restarted must
reach the same conclusion about a package that has already failed its environment repeatedly, and
must not begin again from zero.

---

## Acceptance criteria

1. A package whose environment fails repeatedly and identically reaches a terminal outcome instead of
   being redispatched forever. Demonstrate with a worker that cannot ever succeed.
2. That outcome is distinguishable in the journal from a criterion failure, a worker failure, a
   `mis-specified` fault, and a ladder-exhausted park.
3. The journal retains what the environment said, at least on the first and last attempt, so a human
   can read the cause cold.
4. A package whose environment fails once and then succeeds completes normally, with no recovery
   budget spent. This must not regress.
5. The bound is named and adjustable, alongside the existing retry and local-patch limits.
6. The count re-derives from the journal after a restart and does not reset.
7. A package stopped this way blocks only its own dependents; the rest of the graph proceeds, and the
   run reaches a terminal outcome rather than hanging.
8. Every existing test passes unchanged — readiness waves, herdr sentinel, RR2 briefs, `TMPDIR`
   headroom, gate tests including malformed-finding isolation, driver tests including the antichain
   barrier, recovery tests including the one proving that removing the ladder changes dispatch,
   WP11's retry-identity and spawn-failure tests, WP12's composition and assembly tests, WP13's pane
   ownership tests, render tests, landing-skill doctrine tests.

**Plus one real end-to-end drive, executed and reported.** A two-package graph in a repository you
create, driven by real prime-agent 0.7.2 through real herdr 0.7.1, where one package's environment
fails every time and the other completes. The run must terminate on its own. Report the journal, the
terminal outcome, and how many attempts were made before it stopped.

Then render it with `pce package render` and report the output path.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test in `tests/dispatch.rs` is known to fail intermittently and pass on an
isolated rerun.

## Do not touch

- The recovery ladder's rungs, its retry and local-patch limits, or the rule that a transient
  environment failure charges no budget in the transient case.
- `EnvironmentPreparationFailed` and the `--prepare` path.
- Composition, assembly, gate fault isolation, pane cleanup, the wait mechanism, brief composition,
  criteria execution, the renderer's internals.
- The live RivRetrieve run's evidence.
- The two open `pce:ticket` issues, #46 and #135.

## Report back with

- The rule, in one sentence, and where the bound is named.
- What the terminal outcome is called and how it reads in the journal beside the four it must not be
  confused with.
- Whether it consumes recovery budget, and why you chose that.
- How an intermittent failure still recovers.
- The real drive, including the attempt count before it stopped.
- Anything you found that contradicts what this brief assumes.
