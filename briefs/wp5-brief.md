# WP5 — the work-package worker

Compose the brief a worker receives from its node in the graph, run it through prime-agent, and
collect its outcome. This is the first package where the whole chain runs end to end.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `feature/wp4-herdr-dispatch` in a **new worktree**. That branch stacks on
`feature/wp1-work-package-graph`, which stacks on `repair/orchestration-dead-ends`. None of the four
is merged.

Existing worktrees, for reference:

- `/Users/nicolaslazaro/Desktop/work/pce-wp1-graph` — WP1, commit `0dc0589`
- `/Users/nicolaslazaro/Desktop/work/pce-wp4-dispatch` — WP4, commit `0f1c800`

## What already exists, and where

**WP1** built frozen work-package graphs. A graph is `graph.json` at a vision directory, frozen to
`graph.vN.json`, conforming to `skills/pce/schemas/work-package-graph.schema.json`. A package
carries an id, a title, the repositories it touches, its criteria, and typed dependencies. A
criterion carries `name`, `input`, `observation`, and `command`. Edges are `buildability`, `safety`,
or `risk-ordering`; the first two bind readiness, the third is advisory and overridable. Verbs:
`pce graph check`, `pce graph freeze`, and `pce ready --graph <PATH> [--override-risk-ordering]`.
Core logic in `crates/core/src/work_package_graph.rs`.

**WP4** built dispatch. PCE composes the exact herdr invocation — worktrees, environment, agent
name, argv — and herdr owns the process. A hidden supervisor,
`pce dispatch package-worker --result <PATH> --required-artifact <PATH> -- <ARGV>`, spawns the
worker without a shell, waits, and atomically writes a result file recording duration, stop time,
exact exit code or signal, and artifact presence. Completion is derived by
`pce dispatch package-completions`, which reads only the event log and the filesystem. Results live
at `<VISION_DIR>/.pce/package-results/<PACKAGE_ID>/<ISSUANCE_SEQUENCE>.json`, guaranteed outside
every worktree. Core logic in `crates/core/src/package_completion.rs`.

## The tools, stated rather than assumed

**prime-agent** — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, docs at
`packages/coding-agent/docs/`.

- Headless invocation is `prime-agent -p "<prompt>"`.
- In print mode it **reads piped stdin and merges it into the initial prompt**. This is how a large
  brief should reach it — pipe the brief, keep argv short. It mirrors PCE's existing `--plan-file`
  pattern for Codex.
- It has **no `--output-schema`** and cannot be forced to emit a schema-validated object. Anything
  structured it produces must be a file it writes, which the WP4 supervisor already observes via
  `--required-artifact`.
- `--mode json` emits an event stream, not a result object. Do not mistake it for structured output.

**herdr** — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`.

- **Installed and running: 0.7.1. The checkout is 0.8.0.** The checkout is ahead of what runs; the
  **installed binary is the authority** where they disagree.
- herdr does not recognise `prime-agent` as an agent kind. Its full list is `pi`, `omp`, `claude`,
  `codex`, `copilot`, `devin`, `droid`, `kimi`, `opencode`, `kilo`, `hermes`, `qodercli`, `cursor`.
  A prime-agent worker will show as `unknown` and vanish on exit without reporting `done`.
- Therefore **never consult herdr for completion.** WP4 settled this and a sentinel test enforces it.

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
   Retiring them is WP9.

---

## What must be true when you are done

### The brief is composed by the binary, from the graph

Not a template a human fills in. Given a frozen graph and a package id, the binary produces the
complete brief. Same graph and same package must produce a byte-identical brief.

### It carries four things, in this order

1. **The vision's goal and its acceptance criteria.** What the whole thing is for.
2. **The whole graph, in summary** — every package's id, title and criteria. Summary means exactly
   that: ids, titles, criteria. Not other packages' internals.
3. **The worker's own package, in full** — its criteria with commands, its repositories and their
   worktree paths, its dependencies and their kinds.
4. **An explicit boundary statement**: every other package in that graph is someone else's work and
   is out of bounds.

### The graph is supplied so the worker knows what is *not* its job

This framing is the point, and it must be explicit in the composed text. A worker that can see
neighbouring packages could otherwise justify doing one because it is convenient, and scope creep
is what makes a review unit unreviewable. Naming the boundary converts shared context from a licence
into a constraint.

The second reason is coherence. The corpus's most expensive recorded failure — eleven providers'
legacy pipelines deleted, correct against the vision as written, approved by two critics — was work
that was locally right and globally wrong. A worker holding the whole graph can see when its change
would strand a sibling.

### The worker does not judge its own work

The worker is told its criteria so it knows what it is aiming at. It is **not** the thing that
decides whether they pass. The driver executes the criteria commands and their exit status is the
judgement.

The composed brief must say this plainly to the worker. A worker that self-certifies is the
rubber-stamp defect this whole workflow exists to catch: the corpus contains a checker that passed
all thirty-nine of its own tests while being structurally unable to fail.

Expose the criteria commands; do not execute them in this package. Execution belongs to the driver,
WP7.

### There are three outcomes, not two

The worker writes an outcome file — the `--required-artifact` the WP4 supervisor already observes —
carrying exactly one of:

- **done** — the work is believed complete.
- **failed** — the work could not be completed, with what blocked it.
- **mis-specified** — the package itself is wrong: its criteria do not serve the vision's goal, or a
  dependency it needs is missing from the graph.

The third is the one that does not exist today and matters. A worker holding the vision and the
graph may legitimately conclude the graph is wrong. That is neither a defect in its work nor grounds
for a retry — it routes to re-authoring as plan version `n+1`. Without it, a worker that notices the
graph is wrong will either build the wrong thing or stall silently.

`mis-specified` must name which criterion or which missing dependency is at fault. An unspecific
`mis-specified` is not actionable and should be rejected as malformed.

---

## Acceptance criteria

Use the committed RivRetrieve fixture graph at
`crates/core/tests/data/rivretrieve-work-package-graph.json` throughout.

1. Composing a brief for `RR2` yields text containing: the vision's goal, every one of the seven
   packages by id and title with their criteria, `RR2`'s own criteria with commands, `RR2`'s
   dependency on `RR1` with kind `buildability` and its reason, and an explicit statement that
   `RR1`, `RR3`, `RR4`, `RR5`, `RR6` and `RR7` are out of bounds.
2. The brief contains no package's internals other than the target's.
3. The brief states that the worker's own assessment is not the judgement and that the criteria will
   be executed independently.
4. Composition is deterministic: the same graph and package produce byte-identical output across
   runs.
5. A package naming two repositories produces a brief carrying both worktree paths.
6. An outcome file parses to exactly one of `done`, `failed`, `mis-specified`.
7. A `mis-specified` outcome naming no criterion and no missing dependency is rejected as malformed.
8. A missing outcome file is not any of the three — it remains a productless attempt, per WP4.
9. Every existing WP1 and WP4 test still passes unchanged, including the five readiness waves and
   the herdr sentinel test.

**Plus one real end-to-end run, executed and reported.** Construct a deliberately trivial
single-package graph — for example, a package whose work is to create a file with known contents and
whose criterion command is a `test` on that file. Then:

compose the brief → spawn prime-agent headless through herdr with the brief on stdin → the worker
does the work and writes its outcome → the supervisor records the result → completion is derived
from disk.

Report the observed sequence and every recorded artifact. Keep the package trivial: this proves the
chain, not the model.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.

---

## Do not touch

- WP1's graph, schema, freezing, or readiness. WP4's composition, naming, merge observation,
  supervisor, or completion derivation.
- **Executing criteria commands.** Expose them. The driver runs them — WP7.
- Gates and adversarial review — WP6.
- The driver loop — WP7.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — WP9.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`. Open questions, not work items.

## Report back with

- The composed brief for `RR2`, in full, so its content can be judged directly.
- How the boundary statement is worded, and what stops a worker treating the graph as permission.
- The outcome file format and how `mis-specified` is validated.
- The end-to-end evidence: the trivial graph, the actual invocation, the observed sequence, the
  recorded artifacts.
- Anything you found that contradicts what this brief assumes.
