# WP6 — the gate as node exit

A package is not finished when its worker stops, and not finished when its criteria pass. It is
finished when something has tried to break the built artifact and failed.

Also folds in a correction to WP5's `TMPDIR` handling.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `feature/wp5-package-worker` in a **new worktree**. The stack, none of it merged:

| branch | worktree | commit | built |
|---|---|---|---|
| `repair/orchestration-dead-ends` | `pce-repair` | `ede9840` | spending limits, parking, typed retry |
| `feature/wp1-work-package-graph` | `pce-wp1-graph` | `0dc0589` | frozen graphs, typed edges, readiness |
| `feature/wp4-herdr-dispatch` | `pce-wp4-dispatch` | `0f1c800` | herdr dispatch, supervisor, completion |
| `feature/wp5-package-worker` | `pce-wp5-worker` | `9532d0d` | brief composition, worker, outcomes |

## What already exists, and where

**WP1** — frozen work-package graphs. `graph.json` at a vision directory, frozen to `graph.vN.json`,
schema at `skills/pce/schemas/work-package-graph.schema.json`. A package has an id, title,
repositories, criteria, and typed dependencies. A criterion has `name`, `input`, `observation`,
`command`. Edges are `buildability`, `safety`, `risk-ordering`. Verbs: `pce graph check`,
`pce graph freeze`, `pce ready --graph <PATH> [--override-risk-ordering]`. Logic in
`crates/core/src/work_package_graph.rs`.

**WP4** — dispatch. PCE composes the exact herdr invocation; herdr owns the process. A supervisor,
`pce dispatch package-worker --result <PATH> --required-artifact <PATH> -- <ARGV>`, spawns without a
shell and atomically records duration, stop time, exit code or signal, and artifact presence.
`pce dispatch package-completions` derives completion from the event log and filesystem only — a
sentinel test proves herdr is never consulted. Results at
`<VISION_DIR>/.pce/package-results/<PACKAGE_ID>/<ISSUANCE_SEQUENCE>.json`. Logic in
`crates/core/src/package_completion.rs`.

**WP5** — the worker. `pce package brief` composes a deterministic brief from vision, graph and
package: vision goal and criteria, the whole graph in summary, the target package in full, an
explicit scope boundary. `pce package agent` pipes it to the worker on stdin and exposes
`PCE_PACKAGE_OUTCOME`. Outcomes are strict `done`, `failed`, or actionable `mis-specified`.

**Already in the tree and directly relevant:** `pce gate replay --repo-root --evidence
--execution-ref --broken-ref --repaired-ref --schema --output --expected
<conforming-verdict|nonconforming-verdict>`. It exists to prove a finding is real by demonstrating
behaviour at a broken ref and a repaired ref. Read it before designing anything new; the idea below
is its generalisation, and reusing it beats reinventing it.

## The tools, stated rather than assumed

**prime-agent** — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, docs at
`packages/coding-agent/docs/`. Headless is `prime-agent -p "<prompt>"`, and in print mode it reads
piped stdin and merges it into the prompt — that is how a large brief reaches it. It has **no
`--output-schema`**; anything structured must be a file it writes. `--mode json` is an event stream,
not a result object.

**herdr** — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; the checkout is
0.8.0.** The checkout is ahead of what runs and **the installed binary is the authority.** herdr does
not recognise prime-agent — its kinds are `pi`, `omp`, `claude`, `codex`, `copilot`, `devin`,
`droid`, `kimi`, `opencode`, `kilo`, `hermes`, `qodercli`, `cursor` — so a worker shows as `unknown`
and vanishes on exit. **Never consult herdr for completion.**

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

## Part one: shorten the package `TMPDIR`, do not remove it

WP5 removes `TMPDIR` for the prime-agent child because prime-agent's nested daemon socket exceeded
Darwin's 104-byte `sun_path` limit. The diagnosis was right; the fix is wrong.

The cause is that the package temporary directory embeds human-readable names:

```rust
let temporary_directory = PathBuf::from("/tmp/pce-work-package-tmp")
    .join(graph.vision())
    .join(package.id().as_str());
```

For a real vision that is `/tmp/pce-work-package-tmp/2026-08-11-the-store-is-the-only-copy/RR2` —
67 characters, leaving 37 for everything the child puts underneath.

**Shorten it to a short opaque digest and restore the binary-owned `TMPDIR` for prime-agent
children.** There is a precedent in this codebase already: the herdr agent name is `pce-<28 hex>`
for exactly this reason — a readable name did not fit a length limit, so it became a digest. Do the
same here. Something on the order of `/tmp/pce-tmp/<12 hex>` leaves ample room.

Removing `TMPDIR` is the worse fix because WP4's own comment states why the binary owns it:
*"doing so makes identical routes depend on an unforwarded shell variable."* Removing it for
prime-agent children hands exactly the agents that do the real work whatever the environment
happens to supply — which is the class of failure that produced the original `TMPDIR` report, where
two PCE surfaces permitted disjoint temp roots and every Python repository broke.

Requirements: the binary-owned `TMPDIR` is restored for prime-agent children; its path is short
enough for a nested Unix socket on Darwin; a regression test asserts the actual child environment
contains a binary-owned `TMPDIR`; and a test asserts the path length leaves at least 40 bytes of
headroom under 104.

---

## Part two: the gate

### What the gate is for

The criteria are the **floor**, not the ceiling. They state what was known to matter when the graph
was authored. The gate exists to find what they miss — the defect class that does not fail but
passes.

That class is the whole reason this workflow exists. From the corpus: a checker whose success
outcome was unreachable passed thirty-nine tests including thirty-seven named rejections. An inert
lint rule passed all eighteen of its own gates and was exposed only by deleting its own config line.
A frozen baseline wrong by 7.644x was verified to ten significant figures by five consecutive
reading rounds and found by one re-derivation. **Reading found none of these. Executing found all
of them.**

### The gate may repair, and this revises the design

The design document says building and attacking never share permissions. That is too blunt, and
here is the honest reasoning for changing it.

The measured hazard is real: repair is the highest-defect-density act in the corpus — fourteen
vacuous assertions, ten of them introduced while remedying an earlier one. The rule that follows is
**whoever repairs must not be the final judge.**

In this design the final judge is not an agent at all. It is the criteria commands, executed by the
driver. So a gate that repairs is not self-certifying, provided every repair is followed by
mechanical re-execution. The transcription hop — a separate cold agent retyping a fix the gate
already wrote verbatim — is pure cost, and the corpus shows the loops that closed in one round were
exactly those where the critique supplied the replacement.

So: **the gate may repair, bounded strictly to the defect it named.** No refactoring, no adjacent
improvement, no scope expansion, and never a push, merge or tag. Cross-package necessity becomes a
report, exactly as it does for the worker.

### Every finding must carry its own falsifier

This is the part that makes a repairing gate safe, and it is the core requirement of this package.

A finding that a criterion missed is, by definition, invisible to the existing criteria. So a gate
that finds and fixes such a defect leaves nothing behind that would catch it again. Therefore every
finding must carry:

1. **what is wrong**, concretely;
2. **the repair**, applied;
3. **a criterion command that fails before the repair and passes after it.**

Item 3 is checkable, and it is what distinguishes a real finding from a plausible one. It is the
same shape as the existing `pce gate replay` verb's broken-ref / repaired-ref contract — read that
implementation first.

The gate produces the command and the two refs. **Executing it is the driver's job, WP7.** Expose
it; do not run it here.

A finding whose proposed criterion does not fail at the pre-repair ref is not a finding. That is the
test that stops a gate inventing work.

### Finding nothing is a valid outcome

A gate that attacks an artifact and fails to break it writes an empty findings list. That is
success, and it must be distinguishable from a gate that crashed or wrote nothing — which remains a
productless attempt under WP4's existing accounting.

---

## Acceptance criteria

Use the committed RivRetrieve fixture at
`crates/core/tests/data/rivretrieve-work-package-graph.json`.

1. A gate brief composed for `RR2` carries the vision goal, the graph in summary, `RR2` in full, the
   built artifact's ref, and the criteria **marked as already-passed floor rather than as targets**.
2. The gate brief states the repair bound: only the named defect, no refactoring, no scope
   expansion, no push or merge.
3. A gate outcome parses strictly to either an empty findings list or a list where **every** finding
   carries a description, a repair, a proposed criterion command, a pre-repair ref and a
   post-repair ref.
4. A finding missing any of those five is rejected as malformed.
5. An empty findings list is a distinct, valid outcome — not conflated with a missing outcome file.
6. A missing outcome file remains a productless attempt under WP4, unchanged.
7. The proposed criterion command is exposed for the driver and is **not executed** in this package.
8. `TMPDIR` for a prime-agent child is binary-owned, short, and leaves ≥40 bytes of headroom under
   Darwin's 104-byte socket-path limit.
9. Every existing WP1, WP4 and WP5 test passes unchanged — the five readiness waves, the herdr
   sentinel, the RR2 brief composition, and the strict outcome parsing.

**Plus one real end-to-end run, executed and reported.** Build a deliberately trivial graph with a
seeded defect the criteria cannot see — for example, a package whose criterion checks a file exists
while the file's contents are wrong. Then: worker builds it → criteria would pass → gate attacks →
gate finds the defect, repairs it, and proposes a criterion that fails at the pre-repair ref and
passes at the post-repair ref.

Report the observed sequence and every recorded artifact. Keep it trivial; this proves the
mechanism, not the model.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.

---

## Do not touch

- WP1's graph, schema, freezing, readiness. WP4's composition, naming, merge observation,
  supervisor, completion. WP5's brief composition and outcome parsing — extend rather than alter.
- **Executing any criterion command**, including a gate's proposed one. That is WP7.
- The driver loop — WP7.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — WP9.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`. Open questions, not work items.

## Report back with

- The composed gate brief for `RR2`, in full.
- How the repair bound is worded, and what stops a gate expanding scope.
- The findings format, and how a finding without a working falsifier is rejected.
- The `TMPDIR` path shape and its measured length headroom.
- The end-to-end evidence: the seeded defect, the invocation, the observed sequence, the artifacts.
- Anything you found that contradicts what this brief assumes.
