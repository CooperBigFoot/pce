# WP4 — dispatch leaves the binary

Spawning, isolation and liveness move to herdr. PCE stops hand-rolling process management and keeps
what it is actually for: composing the envelope, recording what happened, and deriving state.

Also closes a gap WP1 surfaced: there is no durable notion of a work package having merged, so
`pce ready` currently takes the merged set as an argument.

Decide your own method. What follows is what must be true when you are done, what will destroy live
work if you get it wrong, and what not to touch.

---

## Where to work

Branch from `feature/wp1-work-package-graph` in a **new worktree** — WP1's frozen graphs and ready
set are what this dispatches against. That branch stacks on `repair/orchestration-dead-ends`.

Orient from `src/main.rs` (`pce dispatch`), `crates/core/src/dispatch.rs`,
`crates/core/src/work_package_graph.rs`, and the dispatch route anchors in `skills/pce/SKILL.md`.
Read `briefs/pce-redesign-design.md` for where this sits.

herdr is installed and running. Learn its CLI from the binary, not from memory:

```bash
herdr --help
herdr worktree     # create, open, list, remove
herdr agent        # start, list, get, read, send, wait, rename
herdr wait         # output, agent-status
```

`herdr agent start <name> [--cwd PATH] [--env KEY=VALUE] -- <argv...>` is the spawn primitive.
`herdr agent wait <target> --status <idle|working|blocked|unknown> [--timeout MS]` is the liveness
primitive. Agent names must match `[a-z][a-z0-9_-]{0,31}` and be unique among live agents.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool right now:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

1. Work in a git worktree, never the main checkout. Own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.

**Additive only, and this matters more here than in WP1.** The existing `pce dispatch codex` /
`pce dispatch gate` verbs, the anchored role registry, the closed placeholder vocabulary and the
route-anchor doctrine in `SKILL.md` all stay exactly as they are. Six runs depend on every one of
them. Deleting the route anchors is WP9's job, after the replacement has actually run something.

You are adding a second dispatch path beside the first, not replacing it.

---

## What must be true when you are done

### The binary composes the invocation; herdr owns the process

PCE builds the exact herdr invocation for a package — worktrees, working directory, environment,
agent name, argv — and invokes it. herdr then owns the process: its lifetime, its liveness, and its
visibility to a human.

This split is deliberate. #41 established that the binary composes every envelope so a route cannot
be mistyped or half-remembered, and that lesson survives. What does not survive is PCE also owning
`spawn`, waiting, and a pid-plus-start-identity sidecar to answer "is it alive" — herdr answers
that natively.

Do not put the composition in a shell script. Do not have the binary re-implement waiting.

### One worktree per repository per package

A package names the repositories it touches. Each gets its own worktree, created through herdr, and
the worker receives every path. A single-repository package is the degenerate case, not a special
one.

### The environment is constructed, not inherited

Carry forward what the repair branch already decided: the child environment is built, not
inherited, and `TMPDIR` is synthesized by the binary to a root it guarantees is writable — not
forwarded from the operator's shell. Whatever herdr's `--env` requires, the *decision* about what
the child sees stays in the binary.

### A worker is findable again after a restart

The driver will die and be restarted. It must be able to find workers it started. So an agent's
name is **derived deterministically** from the vision and the package id — not generated — and
`herdr agent list` plus that derivation is enough to recover the full picture with no local state.

Names are constrained to `[a-z][a-z0-9_-]{0,31}`; collisions across visions must be impossible, not
unlikely.

### Liveness is observed, never inferred from output

Running, finished, and dead are distinguishable through herdr's agent status. Never read a child's
stdout to decide whether it is alive. A worker that finished without producing its required
artifact is a productless attempt, which the existing accounting already models.

### A work package's merge state is derived, not supplied

Replace `pce ready --merged <ID,...>` with observation. A package is merged when every repository
it touches has merged that package's contribution, observed from git and GitHub the way the step
tier already does it.

Merge state stays **three-valued**. `merged`, `not merged`, and `inconclusive` — and inconclusive is
never collapsed into either of the others. That rule is from #38 and it is not negotiable: a run
that cannot observe merge state must say so rather than guess.

---

## Acceptance criteria

**Composition is unit-testable without herdr.** The largest part of this must be provable offline:
given a package and a vision, assert the exact argv, the exact worktree set, the exact environment,
and the exact derived agent name. These tests run in CI and require no herdr session.

1. A single-repository package composes exactly one worktree and the expected argv.
2. A two-repository package composes two worktrees, and the worker receives both paths.
3. The derived agent name is deterministic, matches herdr's name grammar, and two different visions
   with the same package id produce different names.
4. `TMPDIR` in the composed environment is binary-chosen, and is not the invoking shell's value.
5. Merge state is three-valued; an unreachable remote yields `inconclusive` and never `not merged`.
6. A package spanning two repositories is `merged` only when both are, and `inconclusive` if either
   observation is inconclusive.
7. `pce ready` derives merge state itself; the `--merged` argument is gone, and WP1's five wave
   assertions still pass with merge state derived from a fixture rather than supplied.

**One real spawn, actually executed.** A trivial worker — a package whose command is something like
`true` — started through herdr, observed to completion, and recorded as one issuance and one
completion.

This test cannot run without a live herdr session, so gate it on herdr's availability and skip
cleanly otherwise. **But it must exist, and you must run it once and report the evidence.** A
composition test that never spawns anything proves the argv, not the mechanism.

Plus: `cargo fmt --check`, `clippy`, `cargo test --workspace`, and every existing dispatch test
still green and untouched.

---

## Do not touch

- **The existing dispatch verbs, route anchors, role registry, and placeholder vocabulary.** WP9.
- **The step tier**, `milestones.json`, `steps.json`, and their readiness path.
- **The brief a worker receives** — what a package's prompt says is WP5.
- **Gates.** WP6.
- **The driver loop.** WP7.
- **The fourteen open `pce:ticket` issues.**

## Report back with

- The composition surface: what the binary decides versus what herdr decides, and where the seam is.
- The agent-name derivation, and why collisions are impossible rather than unlikely.
- How merge state is observed, and what makes it inconclusive.
- The real-spawn evidence — the actual command, the actual observed transitions.
- Anything you found that contradicts what this brief assumes.
