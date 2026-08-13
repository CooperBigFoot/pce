# WP3 — the bounded recovery ladder

When a package fails, something has to decide whether to try again, try differently, or stop. Today
that decision does not exist, and the last time PCE tried to build one it built a verb nothing read.
This package builds the decision, and proves something reads it.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `feature/wp7-driver` at commit `9c1be9d` in a **new worktree**. The ladder's inputs are
driver outcomes; branching earlier means inventing types that already exist.

The stack, none of it merged:

| branch | worktree | commit | built |
|---|---|---|---|
| `repair/orchestration-dead-ends` | `pce-repair` | `ede9840` | spending limits, parking, typed retry |
| `feature/wp1-work-package-graph` | `pce-wp1-graph` | `0dc0589` | frozen graphs, typed edges, readiness |
| `feature/wp4-herdr-dispatch` | `pce-wp4-dispatch` | `0f1c800` | herdr dispatch, supervisor, completion |
| `feature/wp5-package-worker` | `pce-wp5-worker` | `9532d0d` | brief composition, worker, outcomes |
| `feature/wp6-package-gate` | `pce-wp6-gate` | `454d8c2` | gates, findings, witness/repair anchoring |
| `feature/wp7-driver` | `pce-wp7-driver` | `9c1be9d` | the driver loop, criteria execution, replay |

**Two other packages are being built on this same base right now** — a WP7 follow-up continuing on
`feature/wp7-driver` itself, and WP8 in its own worktree. All three touch `src/main.rs`. Put your
logic in its own module under `crates/core/src/`, keep `src/main.rs` changes to argument parsing and
dispatch, and expect to rebase. Do not restructure shared code to suit yourself.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool right now:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in `/Users/nicolaslazaro/Desktop/work/pce` replaces the binary those runs
invoke.

1. Work in your own git worktree, never the main checkout. Keep your own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.
4. Additive only: `pce dispatch codex`, `pce dispatch gate`, the anchored role registry, the
   placeholder vocabulary and the route-anchor doctrine in `skills/pce/SKILL.md` stay untouched.

## What already exists, and where

**WP1** — frozen graphs. Packages with ids, titles, repositories, criteria, typed dependencies.
`buildability` and `safety` bind readiness; `risk-ordering` is overridable. Logic in
`crates/core/src/work_package_graph.rs`.

**WP4** — dispatch and completion, derived from the event log and filesystem only. A non-zero exit
is a *result*, not an error. An issuance with no result file is a productless attempt and never
becomes complete by inference. Logic in `crates/core/src/package_completion.rs`.

**WP5** — the worker. Outcomes are `done`, `failed`, or `mis-specified`, the last naming a criterion
or a missing dependency.

**WP6** — the gate. Findings anchored to witness/repair commit pairs. Logic in
`crates/core/src/package_gate.rs`.

**WP7** — the driver. Computes the ready antichain, dispatches concurrently, executes criteria in
detached driver-owned clones, replays gate falsifiers in coordinated multi-repository states, records
accepted findings as durable criterion amendments, parks `mis-specified` packages without blocking
unrelated branches, and re-derives its whole view from disk after a restart. Logic in
`crates/core/src/package_driver.rs`; the loop in `src/main.rs`; evidence in
`docs/evidence/wp7-driver.md`. Verbs: `pce package driver-status`, `driver-run`, `driver-criteria`,
`driver-replay`.

**On `repair/orchestration-dead-ends`** there is prior art: spending limits, parking, and typed
retry. Read it before designing. It exists because of the failure described next.

---

## Why this package exists

PCE has already shipped a recovery mechanism that did nothing.

A defect cap counted rounds and stopped work at three. An `escalation-close` verb existed to lift it.
Admission never read that verb's output. So four separate orchestrator runs reached the cap and
stopped dead: the escape hatch was present, documented, and inert, and a human watching could not
help because nothing they did changed what admission computed.

That is the failure this package must not repeat, and it dictates the shape of the acceptance
criteria: **it is not enough for the ladder to exist and be correct. Something must demonstrably read
it, and removing it must change behaviour.**

The second thing that failure taught is what the stop should mean. The cap behaved like a verdict —
*this work is bad* — when what it actually measured was expenditure. Those are different, and
conflating them makes a human argue with the machine about quality when the real question is budget.
So: **the ladder is a spending limit, honestly named, and generous. Parking is the real stop.**

---

## What must be true when you are done

### Three rungs, tried in order

1. **Retry.** The same package, the same brief, a fresh agent. This is for the failure that is not
   about the work — a flaky environment, a truncated run, a model that wandered. It costs nothing to
   describe and often works.
2. **Local patch.** The same package, a brief that additionally carries the recorded evidence of what
   failed: which criterion, its command, its exit status, its output. A worker that can see the
   failure is a different worker from one that cannot, and the corpus is clear that the loops which
   closed in one round were the ones where the critique supplied the specifics.
3. **Replan.** Stop. The package is parked for a human to re-author as plan version `n+1`.

A rung is attempted only when the rung before it is exhausted. The ladder is per package, per graph
run.

### Not every failure climbs the ladder

A failure that is not attributable to the work must not consume the work's budget. The clearest case
is an environment failure — a preparation step or a toolchain that could not be made to run says
nothing about whether the artifact is correct. There is prior doctrine here worth honouring: an
upstream root cause does not charge the role that reported it.

Decide and state which outcomes charge the budget and which do not. Getting this wrong in the
generous direction wastes tokens; getting it wrong in the strict direction reproduces the original
bug, where a run died of a cause it could not have fixed.

`mis-specified` skips the ladder entirely and goes straight to replan. Retrying a worker that told
you the graph is wrong cannot help, and WP7 already parks it — make sure the ladder agrees rather
than fighting it.

### Exhausting the ladder parks, it does not error

A package that has climbed all three rungs is parked, with a record naming every rung attempted, what
was tried, and the evidence at each. The rest of the graph keeps running; only this package's
dependents stay unreleased. A driver that halts the whole run because one branch is stuck throws away
the parallelism the graph exists to provide.

A parked package must be legible enough that a human reading only the journal can tell what happened
without re-running anything. That is the property that was missing when four runs died at the cap.

### The limit is nameable, adjustable, and visible before it bites

A human must be able to see the remaining budget of a running package, not only discover it at
exhaustion. Discovering a limit by hitting it is the same failure in a slower form.

### Something reads it, and this is checkable

The ladder must be wired into the driver's actual decision about what to dispatch next. Make it
possible to demonstrate that behaviour changes when the ladder says stop — not by asserting that the
code path exists, but by a test where the same failure sequence produces different dispatches under
different ladder states.

---

## Acceptance criteria

1. A package whose criteria fail once is retried with the same brief, and the retry is recorded as a
   retry rather than as a fresh dispatch.
2. A package that fails again receives a local-patch brief carrying the recorded criterion failure —
   its command, exit status, and output — and that content is present in the composed text.
3. A package that fails a third time is parked, and its parking record names all three rungs, what
   was attempted at each, and the evidence.
4. A parked package leaves the rest of the graph running; only its own dependents stay unreleased.
5. A `mis-specified` outcome parks immediately without consuming a retry or a local patch.
6. An outcome you classify as not attributable to the work does not advance the ladder. Prove it with
   a package that hits that outcome more times than the ladder has rungs and still has budget.
7. Remaining budget for every package is inspectable while the run is in progress.
8. **Removing the ladder changes what the driver dispatches.** Demonstrate with a test in which the
   identical failure sequence yields different dispatch decisions under different ladder states. An
   assertion that the ladder is consulted is not sufficient; the original bug would have passed such
   an assertion.
9. Ladder state re-derives identically from disk after a restart, consistent with WP7's rule.
10. Every existing WP1, WP4, WP5, WP6 and WP7 test passes unchanged — the five readiness waves, the
    herdr sentinel, the RR2 briefs, the `TMPDIR` headroom assertion, the five gate tests, and all
    four driver tests including the antichain barrier.

**Plus one real end-to-end run, executed and reported.** A graph with two branches where one package
fails deterministically on every attempt. Show it climbing retry → local patch → park, show the
local-patch brief containing the actual failure output, and show the other branch completing
throughout. Report the observed sequence and every recorded artifact.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test is known to fail intermittently and pass on rerun; it predates this work.

## Do not touch

- WP7's materialization, antichain dispatch, restart derivation, amendments, replay, or its existing
  parking of `mis-specified`. Extend rather than alter — a WP7 follow-up is editing that code
  concurrently.
- The artifact surface that renders a run for a human — WP8, in flight.
- WP1's graph and schema. WP6's findings format.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — WP9.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`. Open questions, not work items.

## Report back with

- The three rungs as implemented, and what a local-patch brief actually contains.
- Which outcomes charge the budget and which do not, and why you drew the line there.
- What a parked package looks like in the journal, read cold.
- How a human sees remaining budget mid-run.
- The test that proves removing the ladder changes dispatch.
- The end-to-end evidence.
- Anything you found that contradicts what this brief assumes.
