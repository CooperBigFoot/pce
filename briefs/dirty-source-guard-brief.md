# Brief: the dirty-source guard counts the driver's own output as dirtiness, and names nothing

Status: READY TO DISPATCH — no grill. **This aborted a live run and its error message prescribes a
remedy that deletes the run.** The decisions below are evidence-resolvable; examine the evidence,
decide, implement, and record what you decided and why in your completion report and `CONTEXT.md`.
Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion), stop and report.
Written 2026-08-21.

## The defect

`refuse_dirty_source_repositories` (`src/main.rs:3677-3696`) runs
`git status --porcelain --untracked-files=normal` in each `--repository` source and aborts if the
output is non-empty:

```
source repository `taqsim` is dirty; commit, stash, or remove its changes before driver-run
```

Two things are wrong with it.

**1. The driver's own output is a source of the dirtiness it refuses.** The vision directory lives at
`<repository>/planning/<vision>/`, inside the very repository the guard requires clean. The driver
writes its journal, every package brief, every outcome document, park evidence and `supervision.md`
there. So a run that has produced anything has, by construction, dirtied the tree it will next be
asked to find clean.

Four of the five live runs pass only by accident: `hfx`, `bluesmith`, `palaestra` and `pourpoint` all
happen to carry a `planning/` entry in `.gitignore` (`hfx/.gitignore:20`, `bluesmith/.gitignore:2`,
`palaestra/.gitignore:26`, `pourpoint/.gitignore:23-24`). `taqsim` did not, and aborted. Nothing in
pce establishes or checks that convention — it is folklore, and the guard silently depends on it.

**2. The message prescribes destruction and names no paths.** `git stash -u` and `git clean` remove
untracked `planning/` — the journal, the briefs, the outcomes, the park evidence. Following the error
message literally deletes the admissible proof of the run in order to satisfy a cleanliness check.
`git commit` mutates a participating repository. The taqsim orchestrator refused all three and
stopped, which was correct; a less careful worker would not have.

The message also names no offending path, so a supervisor cannot tell whether the dirtiness is
`uv.lock` (load-bearing) or the driver's own journal (not).

## The evidence

taqsim `planning/2026-08-20-taqsim-modelling-layer-on-incidence`. After a clean mechanical freeze to
plan version 3 (`graph.v3.json`, sha256 `37782cb9e5fd…`, `graph check` clean), the relaunch aborted
before dispatching TQ2:

```
driver-aborted: source repository `taqsim` is dirty; commit, stash, or remove its changes
```

The dirty set was `M uv.lock`, `?? CONTEXT.md`, `?? docs/`, `?? planning/` — **identical to what
three prior launches ran against without complaint.** The guard is new in `fe132c8` (installed digest
`273f5c34…`); the tree did not change, the binary did. TQ2 was never dispatched, so nothing further
was spent.

## What to change

**Decisions delegated to you:**

1. **What the guard must not count.** At minimum the driver's own vision directory — the tree it is
   about to write into — cannot be evidence that the repository is unfit to run.
   Recommendation: exclude the vision directory from the status check for the repository that
   contains it, and keep refusing everything else. Decide whether the exclusion is the vision
   directory alone or all of `planning/`; the narrower one is defensible and safer, say which you
   chose and why.
2. **What the message says.** It must list the offending paths. It must not prescribe `git clean` or
   `git stash -u` against a tree that may hold the only copy of a run's evidence. Rewrite it so a
   supervisor can act without guessing, and so the suggested remedies cannot destroy run proof.
3. **Whether the guard should have been on by default at all.** It appeared with no migration and
   broke a run whose tree was unchanged. Decide whether it stays default-on with the exclusion, or
   becomes advisory with an explicit opt-in. Recommendation: default-on with the exclusion — the
   guard is protecting something real (a dirty source composes an unreproducible base), it was just
   over-broad. Say what you concluded.
4. **Whether the vision directory belongs inside a participating repository at all.** This is the
   structural question underneath the bug and you may conclude it is out of scope for this brief.
   If you conclude that, say so and do not change the layout.

## Tests

The decisive test is the abort that happened: a source repository whose *only* dirtiness is the
driver's own vision directory must launch. Pair it with the negative — a source repository with a
modified tracked file outside the vision directory must still refuse, and the refusal must name that
file. Either test alone passes trivially; the pair is what makes them meaningful. Add a third
asserting the message contains no `git clean` or `git stash -u` recommendation.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `fe132c8`, pushed and clean.
- Installed binary digest `273f5c34…`, and `~/.local/bin/pce` is a symlink into the main checkout's
  `target/release/pce`. Do **not** `cargo build --release` in the main checkout and do not run
  `./install.sh`; that is a fleet install across five live runs. The supervisor owns installation.
- Merge and **edit** in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness`. Full suite there: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets`, `cargo test --workspace`. Known parallel-load flakes in
  `tests/dispatch.rs` (`gate_execution_echoes_large_input_without_deadlock`,
  `gate_execution_drains_three_pipes_concurrently`) — verify in isolation before blaming a change.
- Key code: `src/main.rs:3677-3696` (`refuse_dirty_source_repositories`), its call site in the
  driver-run path, and the existing assertion at `tests/package_composition.rs:1028` which pins the
  current message text and will need updating.
- The vision directory is derived by `driver_vision_directory(command)`; reuse it rather than
  re-deriving the path.

## The waiting consumer

taqsim, plan version 3, TQ2 blocked behind this and TQ4/TQ5 behind TQ2. The supervisor has already
made taqsim's tree clean by hand (`fbfe4e7`: gitignore `/planning/`, commit `CONTEXT.md`, the two
ADRs, and a `uv.lock` regeneration), so taqsim is unblocked today and this brief is not what releases
it. What this brief prevents is the next run whose repository lacks the folklore `.gitignore` entry —
and the next orchestrator that follows the error message instead of refusing it.

A healthy first pass: a driver-run whose source repository contains only an uncommitted vision
directory launches and dispatches, and a driver-run against a genuinely modified tracked file refuses
with that file named.
