# WP7 — the driver

Everything before this package built a piece that something else has to run. This is that something
else. When it works, a frozen graph and a worker binary are enough to take a vision from nothing to
finished without a human in the loop.

It is also the first package that is allowed to execute a criterion command. Every package before it
was explicitly forbidden from doing so, and deferred here.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `feature/wp6-package-gate` in a **new worktree**. The stack, none of it merged:

| branch | worktree | commit | built |
|---|---|---|---|
| `repair/orchestration-dead-ends` | `pce-repair` | `ede9840` | spending limits, parking, typed retry |
| `feature/wp1-work-package-graph` | `pce-wp1-graph` | `0dc0589` | frozen graphs, typed edges, readiness |
| `feature/wp4-herdr-dispatch` | `pce-wp4-dispatch` | `0f1c800` | herdr dispatch, supervisor, completion |
| `feature/wp5-package-worker` | `pce-wp5-worker` | `9532d0d` | brief composition, worker, outcomes |
| `feature/wp6-package-gate` | `pce-wp6-gate` | `454d8c2` | gates, findings, witness/repair anchoring |

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

## The tools, stated rather than assumed

**prime-agent** — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, docs at
`packages/coding-agent/docs/`. Headless is `prime-agent -p`; in print mode it reads piped stdin and
merges it into the prompt. It has **no `--output-schema`**; anything structured must be a file it
writes. `--mode json` is an event stream, not a result object.

**herdr** — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; the checkout is
0.8.0. The installed binary is the authority.** herdr does not recognise prime-agent — its kinds are
`pi`, `omp`, `claude`, `codex`, `copilot`, `devin`, `droid`, `kimi`, `opencode`, `kilo`, `hermes`,
`qodercli`, `cursor` — so a worker shows as `unknown` and vanishes on exit. **Never consult herdr
for completion.**

## What already exists, and where

**WP1** — frozen work-package graphs. `graph.json` at a vision directory, frozen to `graph.vN.json`,
schema at `skills/pce/schemas/work-package-graph.schema.json`. A package has an id, title,
repositories, criteria, and typed dependencies. A criterion has `name`, `input`, `observation`,
`command`. Edges are `buildability`, `safety`, `risk-ordering`; the first two bind readiness, the
third is advisory. Verbs: `pce graph check`, `pce graph freeze`, `pce ready ... [--graph]
[--override-risk-ordering]`. Logic in `crates/core/src/work_package_graph.rs`.

**WP4** — dispatch. PCE composes the exact herdr invocation; herdr owns the process. A supervisor
spawns without a shell and atomically records duration, stop time, exit code or signal, and artifact
presence. `pce dispatch package` issues; completion is derived from the event log and filesystem
only, and a sentinel test proves herdr is never consulted. Results at
`<VISION_DIR>/.pce/package-results/<PACKAGE_ID>/<ISSUANCE_SEQUENCE>.json`, outside every worktree.
Logic in `crates/core/src/package_completion.rs`.

**WP5** — the worker. `pce package brief` composes deterministically from vision, graph and package.
`pce package agent` pipes it on stdin. Outcomes are strict `done`, `failed`, or actionable
`mis-specified` — the last naming a criterion or a missing dependency.

**WP6** — the gate. `pce package gate-brief` and `pce package gate-agent`. A gate that finds a defect
authors exactly two commits per repository it touches: a **witness** introducing the falsifier and
nothing else, and a **repair** descending directly from it. A finding carries a description, the
repair, a `proposed_criterion_command`, and a non-empty `repository_refs` list of
`{repository, witness_ref, repair_ref}`. WP6 resolves those refs, requires reachability, requires
the witness to be the repair's sole direct parent, and confirms ancestry — but **never executes the
proposed command**. Logic in `crates/core/src/package_gate.rs`; evidence in
`docs/evidence/wp6-followup-end-to-end.md`.

**Already in the tree:** `pce gate replay --repo-root --evidence --execution-ref --broken-ref
--repaired-ref --schema --output --expected`. It proves a finding is real by demonstrating behaviour
at a broken ref and a repaired ref. Read it first. It takes recorded evidence rather than a raw
command, and bridging that gap is this package's job — WP6 correctly declined to guess how.

---

## What must be true when you are done

### The criteria are the judgement, and this package executes them

A package's criteria commands run after its worker reports `done`, in the package's own repositories
at the artifact the worker produced. Their exit status is the verdict. The worker's own opinion is
input, never evidence — WP5's brief already tells the worker so, and this is the code that makes it
true.

A criterion command is a shell string authored by a human through `to-graph`. Run it through a
shell, in the repository root, and record for each: the command, the exit status, and captured
output. That record is the evidence a human reads when something is wrong, and every criterion
execution must produce one whether it passed or failed.

A package with a failing criterion is not complete and does not release its dependents.

### A gate finding is credited only if its falsifier actually falsifies

This is the check the whole witness/repair discipline exists to enable, and WP6 deliberately left it
here.

For each finding, execute its `proposed_criterion_command` at the witness state and again at the
repair state. **It must fail at the witness and pass at the repair.** A finding where either does
not hold is rejected: not credited to the gate, not recorded as a defect found, and the reason
recorded. A rejected finding is the mechanism that stops a gate inventing work, and it must be
possible to see rejections in the log rather than only successes.

**Multi-repository semantics, which WP6 explicitly deferred to you.** A finding may name several
repositories, each with its own witness/repair pair. The command is package-scoped, not
repository-scoped, so the two states are *coordinated*: every named repository at its witness ref
together, run once, expect failure; then every named repository at its repair ref together, run
once, expect success. Do not pair repositories independently — a per-repository replay would run the
package's command against a mixed state that never existed. Say in the code where the command's
working directory comes from when a finding spans repositories; a package-level convention decided
once beats a guess made per finding.

Do not mutate the gate's worktrees to reach those states. Materialise each state somewhere the
driver owns.

### An accepted finding's command becomes a criterion — without editing a frozen graph

The falsifier's whole purpose is that the defect cannot recur silently. A command that proves a
defect and is then discarded has bought one repair and no protection.

But the graph is frozen, and frozen means frozen. So an accepted finding is recorded as an
**amendment** to the package's criteria — durable, attributed to the gate that found it, carrying
the command and the refs that proved it. Re-running the package must include amended criteria
alongside the authored ones. Whether amendments are folded into a graph is a later authoring
decision, not this package's.

### The loop runs, and it runs more than one package at once

Compute the ready set, dispatch every ready package, wait, execute criteria, gate, advance, repeat.
A package becomes ready when every `buildability` and `safety` dependency has completed; a
`risk-ordering` dependency binds unless overridden. Two packages ready at once must dispatch at
once — a driver that serialises an antichain wastes the graph's entire point.

The loop terminates when no package is running and none is ready. Reaching that state with packages
still incomplete is a distinct, reportable outcome from finishing them all.

### `mis-specified` stops the package, it does not retry it

A worker reporting `mis-specified` has said the graph is wrong. Retrying cannot help. Park the
package with its stated reason, do not release its dependents, and keep the rest of the graph
running — the other branches are unaffected by one node's specification being wrong.

Re-authoring as plan version `n+1` is a human act and is out of scope here. Surface it; do not
attempt it. The bounded recovery ladder for the cases that *are* retryable is WP3, not this package.

### It survives a restart

The driver will be killed mid-run and restarted. Discard every scrap of in-process state, re-derive
from the event log and the filesystem, and reach the identical answer about which packages are
running, which finished, which failed, which are parked, and what each package's criteria last did.
This is WP4's rule extended to the whole loop, and it is what makes the driver safe to kill.

---

## Acceptance criteria

Use the committed RivRetrieve fixture at
`crates/core/tests/data/rivretrieve-work-package-graph.json` where a real graph is needed.

1. A package whose worker reports `done` and whose criteria all exit zero is complete, and each
   criterion has a recorded execution carrying its command, exit status and output.
2. A package whose worker reports `done` and one of whose criteria exits non-zero is **not**
   complete, does not release its dependents, and the failing criterion is identifiable from the log
   alone.
3. A gate finding whose proposed command fails at the witness state and passes at the repair state
   is accepted, and its command is recorded as a durable amendment to that package's criteria.
4. A finding whose proposed command **passes** at the witness state is rejected, not credited, and
   the rejection and its reason are recorded.
5. A finding whose proposed command **fails** at the repair state is likewise rejected and recorded.
6. A finding spanning two repositories is replayed with both repositories at witness together and
   both at repair together — demonstrate that the coordinated states are what the command sees.
7. Two packages ready simultaneously are dispatched concurrently, not one after the other.
8. A worker reporting `mis-specified` parks its package, leaves dependents unreleased, and does not
   stop packages on unrelated branches.
9. Killing the driver mid-run and restarting it re-derives an identical view of running, complete,
   failed and parked packages from disk alone.
10. Criteria execution and falsifier replay never mutate a worker's or gate's worktree.
11. Every existing WP1, WP4, WP5 and WP6 test passes unchanged — the five readiness waves, the herdr
    sentinel, the RR2 briefs, the `TMPDIR` headroom assertion, and the five gate tests. WP6's test
    that WP6 itself does not execute a proposed criterion **stays**; this package executing it is not
    a reason to relax that.

**Plus one real end-to-end run, executed and reported.** Take the seeded-defect scenario from
`docs/evidence/wp6-followup-end-to-end.md` — a criterion that checks a file exists while its contents
are wrong — and extend it to a graph with at least two packages where one depends on the other.
Drive it: ready set → dispatch → worker → criteria executed → gate → falsifier replayed and accepted
→ dependent released → dependent built → loop terminates. Report the observed sequence and every
recorded artifact.

Then run it a second time with the driver killed partway through, and show it resumes to the same
end state.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. Note:
one concurrent pipe-drain test is known to fail intermittently and pass on rerun; it predates this
work.

---

## Do not touch

- WP1's graph, schema, freezing, readiness. WP4's composition, naming, supervisor, completion. WP5's
  brief composition and outcome parsing. WP6's findings format and ref validation. Extend rather
  than alter.
- The bounded recovery ladder — retry, local patch, replan — WP3.
- The artifact surface that renders a run for a human — WP8.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — WP9.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`. Open questions, not work items.

## Report back with

- How a criterion is executed and what its recorded execution contains.
- The coordinated multi-repository replay: how both states are materialised, and where the command's
  working directory comes from.
- How an accepted finding's command is stored as an amendment, and how a re-run picks it up.
- What a rejected finding looks like in the log.
- The end-to-end evidence, including the kill-and-resume run.
- Anything you found that contradicts what this brief assumes.
