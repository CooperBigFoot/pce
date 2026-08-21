# WP12 — the graph must compose the work, not only schedule it

Eight packages built real code against a real repository and every one of them branched from the
same commit. Not one contains another. A `buildability` edge — the central idea of this design —
orders dispatch in time and then hands the dependent a tree without its dependency in it.

Nothing merges either. The run ends as a fan of divergent branches that have never met, all green,
and no criterion anywhere can see that they conflict.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `main` at commit `a058b2f` in a **new worktree**.

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
3. Do not push, merge, tag, or open PRs **in this repository**.
4. **Do not touch `/Users/nicolaslazaro/Desktop/work/RivRetrieve` or `/tmp/pce-work-package-worktrees`.**
   They hold the evidence from the first real run. Build your own fixtures elsewhere; WP11 added
   `PCE_WORK_PACKAGE_WORKTREE_ROOT` for exactly this.
5. A sibling package is concurrently fixing gate fault-isolation and herdr pane cleanup in
   `src/main.rs` and the driver. Keep your changes where they belong and expect to rebase.

## The tools, stated rather than assumed

**herdr** — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; the checkout is
0.8.0. The installed binary is the authority.** `herdr worktree create --cwd --branch --base --path`
is what composition emits. herdr does not recognise prime-agent; never consult it for completion.

**prime-agent** — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, installed 0.7.2. Headless
is `prime-agent -p`, reads piped stdin, no `--output-schema`.

## What already exists

- `crates/core/src/herdr_dispatch.rs` — `compose_herdr_work_package_dispatch` builds the worktree
  path, the herdr agent name, the branch, and the **base ref**. WP11 just added the attempt number
  to all of them; the branch is now `pce/<vision>/<package>/attempt-<n>`.
- `crates/core/src/work_package_graph.rs` — packages, typed edges, readiness.
- `crates/core/src/package_driver.rs` and the loop in `src/main.rs` — ready antichain, criteria in
  prepared detached clones, coordinated replay, amendments, recovery ladder.
- `crates/core/src/run_render.rs` — `pce package render`.

## The evidence, from the run that found this

The RivRetrieve graph, eight packages, seven completed. Afterwards:

```
RR2: no   RR3: no   RR4: no   RR5: no   RR6: no   RR8: no      (contains RR1?)
```

Nine branches, all rooted at `b8d6deb`. `RR2` depends on `RR1` by `buildability` with the reason
*the read-back comparison calls the shared reader's query path* — and `RR2` was built with no shared
reader present. Its criteria passed anyway, which tells you what those criteria were actually
measuring.

`RR2`, `RR3` and `RR5` all touch the store internals.

---

## What must be true when you are done

### A dependent is built on top of what it depends on

When a package is dispatched, its worktree starts from a base that contains the completed work of
every package it depends on. That is what a `buildability` edge asserts, and today the assertion is
decorative.

Two sub-cases you must decide and state:

- **Several dependencies.** A package with more than one must see all of them. Combining them is
  itself work that can fail; a conflict between two dependencies is a real finding about the graph,
  not an error to swallow. Whatever you do, a failure to combine must be recorded as its own typed
  outcome — not as a worker failure, and not as a criterion failure.
- **An overridden `risk-ordering` edge.** That edge is not a code fact, so the packages it orders
  are deliberately allowed to proceed in parallel. Overriding it must not silently start composing
  work the graph said need not compose. Say what you chose.

### A retry inherits the same base

WP11 makes a retry branch from the graph's authored base. That was correct when nothing composed and
is wrong the moment this package lands: a retried dependent would silently lose its dependencies and
fail for a reason nobody could see. A retry must start from the same composed base its first attempt
did.

**This brief supersedes WP11 on that one point.** Keep WP11's separate identity per attempt — the
distinct worktree, agent name and branch — and change only what the base is derived from. Its
existing test that attempt two does not contain attempt one's commit will need to become a test
about the *composed* base instead of the authored one; update it rather than deleting it.

### The composed whole is gated before anything is promoted

Per-package criteria prove pieces. Nothing currently proves the assembly, and this project has
measured the difference: independently green units do not necessarily compose, and a conflict that
neither unit can observe alone is exactly what the retired milestone tier existed to catch.

When the graph finishes, the union of every package's work must be assembled and **every package's
criteria re-executed against that assembly**, including criterion amendments accepted from gate
findings. A criterion that passed in its own package and fails on the assembly is the finding this
whole step exists to produce, and it must be reported as distinct from a package failure.

The run is not finished until that holds. A graph whose packages all completed but whose assembly
fails is a distinct terminal outcome from success — the same distinction the driver already draws
between `finished` and `blocked`.

Do not open pull requests, and do not merge into anyone's default branch. Producing and gating the
assembly is this package's job; promoting it is a human act and stays out of scope.

### It survives a restart, like everything else

Every fact above re-derives from the journal and the filesystem after the driver is killed and
restarted, with no in-process state. That rule has held through every package so far and holds here.

---

## Acceptance criteria

1. A package with one `buildability` dependency is dispatched into a worktree whose base contains
   that dependency's completed work. Prove it by content, not by ref name.
2. A package with two dependencies sees both.
3. A failure to combine two dependencies is recorded as its own typed outcome, is distinguishable in
   the journal from a worker failure and from a criterion failure, and does not charge the recovery
   budget.
4. A retried dependent starts from the same composed base as its first attempt, and still gets its
   own worktree, agent name and branch. WP11's per-attempt identity tests still pass.
5. `safety` edges compose exactly as `buildability` does. State what you chose for an overridden
   `risk-ordering` edge and why.
6. When every package completes, the assembly is produced and every package's criteria — authored
   and amended — are executed against it, with each execution recorded.
7. A criterion that passes in its own package and fails on the assembly produces a distinct,
   reportable terminal outcome. **Demonstrate it with a deliberately conflicting pair of packages**,
   each green alone.
8. Nothing opens a pull request or merges into a default branch.
9. Restart re-derives identically from disk at every stage, including part-way through assembly.
10. Every existing test passes unchanged — readiness waves, herdr sentinel, RR2 briefs, `TMPDIR`
    headroom, gate tests, driver tests including the antichain barrier, recovery tests including the
    one proving that removing the ladder changes dispatch, WP11's retry-identity and spawn-failure
    tests, render tests, landing-skill doctrine tests.

**Plus one real end-to-end drive, executed and reported.** A three-package graph in a repository you
create, driven by real prime-agent 0.7.2 through real herdr 0.7.1, where the third package genuinely
requires code written by the first two — it must not be able to pass its criteria unless it can see
them. Show the assembly produced and all criteria re-executed against it.

Then render it with `pce package render` and report the output path.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test in `tests/dispatch.rs` is known to fail intermittently and pass on an
isolated rerun.

## Do not touch

- The graph schema and readiness, brief and gate-brief composition, criteria execution and
  materialization, coordinated replay, amendments, the recovery ladder's rungs and budgets, the wait
  mechanism, the renderer's internals. Extend rather than alter.
- WP11's per-attempt identity. Only the base ref changes.
- Gate fault isolation and herdr pane cleanup — a sibling package owns both, concurrently.
- The live RivRetrieve run's evidence.
- The two open `pce:ticket` issues, #46 and #135.

## Report back with

- How a dependent's base is produced, and what happens when two dependencies conflict.
- What you chose for an overridden `risk-ordering` edge, and why.
- Where the assembly is built, and what re-executing criteria against it costs.
- What an assembly failure looks like in the journal beside a package failure.
- The real drive, including the deliberately conflicting pair.
- Anything you found that contradicts what this brief assumes.
