# WP10 — the driver owns its own dispatch

Every piece of the harness is built and merged. One seam is missing, and two independent sessions
papered over it rather than reporting it as the gap it is. This package closes it, and it is the last
thing before the harness can be pointed at real work.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `main` at commit `a26c398` in a **new worktree**. The stack is merged; there is no
branch stack to reason about any more.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool right now, and as of `a26c398` they are running against
the merged harness:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in `/Users/nicolaslazaro/Desktop/work/pce` replaces the binary those runs
invoke, and a `git checkout` there rewrites the skills they read.

1. Work in your own git worktree, never the main checkout. Keep your own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.
4. Additive only: `pce dispatch codex`, `pce dispatch gate`, the anchored role registry, the
   placeholder vocabulary and the route-anchor doctrine in `skills/pce/SKILL.md` stay untouched.

## The tools, stated rather than assumed

**prime-agent** — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, docs at
`packages/coding-agent/docs/`. Installed version 0.7.2. Headless is `prime-agent -p`; in print mode
it reads piped stdin and merges it into the prompt. No `--output-schema`. `--mode json` is an event
stream, not a result object.

**herdr** — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; the checkout is
0.8.0. The installed binary is the authority.** herdr does not recognise prime-agent as an agent
kind, so a worker shows as `unknown` and vanishes on exit. **Never consult herdr for completion.**

## What already exists, and where

- **The graph** — `crates/core/src/work_package_graph.rs`. Frozen graphs, typed edges, readiness.
- **Dispatch** — `crates/core/src/package_completion.rs`. `pce dispatch package` composes the herdr
  invocation and issues; a supervisor spawns without a shell and atomically writes a result file to
  `<VISION_DIR>/.pce/package-results/<PACKAGE_ID>/<ISSUANCE_SEQUENCE>.json` recording duration, stop
  time, exit code or signal, and artifact presence. Completion derives from the event log and
  filesystem only; a sentinel test proves herdr is never consulted.
- **The worker brief** — `crates/core/src/package_worker.rs`. `pce package brief` composes
  deterministically; `pce package agent` pipes it on stdin and exposes `PCE_PACKAGE_OUTCOME`.
  Outcomes are `done`, `failed`, or `mis-specified`.
- **The gate** — `crates/core/src/package_gate.rs`. Findings anchored to witness/repair commit pairs.
- **The driver** — `crates/core/src/package_driver.rs`. Ready antichain, concurrent dispatch,
  criteria in prepared detached clones, coordinated replay, amendments, restart derivation.
- **Recovery** — `crates/core/src/package_recovery.rs`. Retry, local patch, park.
- **The render** — `crates/core/src/run_render.rs`. `pce package render`.

Evidence worth reading first: `docs/evidence/wp7-driver.md`, whose final section contains the
adapter this package deletes.

---

## What is actually wrong

`pce package driver-run` takes a worker argv. It spawns that argv and waits for it to exit. It does
not compose a dispatch, does not compose a worker brief, and does not know herdr or prime-agent
exist.

Everything needed to do those things is built. Nothing joins them. Two sessions hit this seam from
opposite sides and both worked around it instead of naming it:

- The recovery work found the driver does not compose worker briefs, and added a separate
  *supplement channel* that the real brief path appends to.
- The driver follow-up found the driver does not compose dispatch, and joined the two contracts with
  a shell script.

That script is the reason this package exists:

```sh
RESULT_PATH=$(python3 -c '...' "$HERDR_DISPATCH")
i=0
while [ ! -s "$RESULT_PATH" ]; do
  i=$((i+1)); [ "$i" -lt 1800 ] || { echo "timed out waiting for $RESULT_PATH" >&2; exit 124; }
  sleep 1
done
```

Two defects from this project's own corpus, both reintroduced in eight lines.

**The hand-written poller.** One run made 98 detached dispatches with no completion signal and
needed a poller written per batch; two of three workers had finished and were sitting idle when the
human asked how the orchestrator knew. Completion-as-a-file exists precisely so nobody writes this
again.

**The bounded wait that fabricates an outcome.** `exit 124` after 1800 seconds is a harness timeout
converting a working child into a failure. That is the whole finding of #166 — a `pr-reviewer` child
exceeding the orchestrator's ten-minute ceiling was killed after issuance had been appended, so the
round was spent with nothing to show, and the step hit its cap with one round lost to
infrastructure. The binary was given an unbounded wait for exactly this reason.

Neither defect is in anyone's design. Both arrived in the component left holding a join that no
package owned.

---

## What must be true when you are done

### The driver composes the dispatch

Given a ready package, the driver itself composes and issues the dispatch: the herdr invocation, the
worktrees, the environment, the agent name, and an argv that runs `pce package agent` with the
composed brief reaching the worker on stdin. No caller supplies a worker command for the
implementation agent, and no shell appears anywhere in the chain.

The same holds for the gate. A gate is dispatched the same way, with the gate brief.

Keep an explicit, named override so the existing deterministic tests can inject a fake worker — but
the *default* path must be the real one, and it must be the path the real drive exercises.

### The wait is not a poll, and never invents an outcome

However the driver learns a result file has appeared, three things must hold.

**It lives in the binary.** Not in a script, not in a caller, not in a skill.

**No invented deadline turns a slow worker into a failure.** A worker that takes six hours is slow,
not failed. If you want an upper bound at all it must be supplied deliberately by the caller, and
reaching it must be recorded as *the driver stopped waiting*, never as the worker having failed —
the issuance stays unaccounted and enumerable, exactly as it does today when a worker is SIGKILLed.

**The file on disk stays the authority.** Whatever notification mechanism you use is an optimisation
over re-deriving from the filesystem, never a substitute for it. Restart must still reconstruct the
identical view with no in-process state, and must still collect an already-written result without
redispatching.

### The recovery supplement folds into the brief

Retry, local patch and park are correct and their tests stay. What must change is the plumbing: the
driver composes the full worker brief and the local-patch evidence is part of that brief, not a
supplement appended by a different code path. A worker should not be able to tell whether it is a
first attempt or a repair from the *shape* of what it received.

### The environment is the one the route contract already mandates

The composed dispatch carries the environment entries the anchored route contract requires, and a
binary-owned short `TMPDIR` as already implemented. One recorded incident had every Codex executor
on macOS fail because two PCE surfaces permitted disjoint temp roots; do not reintroduce a second
opinion about the environment by composing it in a new place.

---

## Acceptance criteria

1. `driver-run` requires no worker argv for the implementation agent, and composes the dispatch,
   the brief and the environment itself.
2. No shell interprets any part of the dispatch chain. No `sleep` and no fixed iteration count
   appears in the wait.
3. A worker that takes substantially longer than any previous test worker still completes normally,
   with one issuance and one completion. Demonstrate with a worker that outlives whatever wait
   granularity you chose by a wide margin.
4. An issuance whose worker is SIGKILLed before writing a result stays unaccounted and enumerable
   forever. Nothing converts elapsed time into a failure.
5. If a caller supplies a deliberate upper bound and it is reached, the record says the driver
   stopped waiting; the issuance remains unaccounted and is not credited as a worker failure.
6. A local-patch attempt receives one composed brief containing the criterion failure evidence
   inline. No separate supplement channel remains.
7. Gates are dispatched through the same composed path as workers.
8. Restart re-derives identically from disk and collects an already-written result without
   redispatching — the existing test stays green.
9. Every existing test passes unchanged: readiness waves, the herdr sentinel, the RR2 briefs, the
   `TMPDIR` headroom assertion, the gate tests, the four driver tests including the antichain
   barrier, the three recovery tests including the one proving that removing the ladder changes
   dispatch, and the render tests.

**Plus one real end-to-end drive, executed and reported.** A two-package graph where the second
depends on the first, in a real repository, driven by real prime-agent 0.7.2 through real herdr
0.7.1, with **no adapter script of any kind**. Ready set → composed dispatch → worker → preparation
→ criteria → gate → dependent released → dependent built → loop terminates.

Then render it with `pce package render` and report the output path.

The previous drive is at `docs/evidence/wp7-driver.md`; write yours beside it and say plainly what
is different, in particular that nothing outside the binary participates in the join.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test in `tests/dispatch.rs` is known to fail intermittently and pass on an
isolated rerun.

## Do not touch

- The graph and schema, dispatch composition and result-file format, brief and gate-brief
  composition, criteria execution and materialization, coordinated replay, amendments, the recovery
  ladder's rungs and budgets, the renderer. Extend rather than alter.
- Making the tracked repository contract's `install` and `preflight` fields executable — that is
  #135 and #105, and it changes a surface every concurrent run reads.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — the cutover package.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`. Open questions, not work items.

## Report back with

- How the driver composes a dispatch, and what the composed argv looks like in full.
- The wait mechanism, and what happens at every way a worker can stop: clean exit, non-zero exit,
  signal, and never finishing at all.
- Where the local-patch evidence now sits in the composed brief.
- The real drive: the graph, the invocation, the observed sequence, the artifacts, the rendered page.
- Anything you found that contradicts what this brief assumes.
