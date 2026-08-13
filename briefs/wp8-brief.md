# WP8 — the run, rendered

A graph run is currently legible only by reading a JSON journal. The picture that makes it
understandable has so far been drawn by hand, once, for one graph. This package makes the binary draw
it from the run.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `feature/wp7-driver` at commit `9c1be9d` in a **new worktree**. That is where the journal
you are rendering exists.

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
`feature/wp7-driver` itself, and WP3 in its own worktree. All three touch `src/main.rs`. Put your
logic in its own module under `crates/core/src/`, keep `src/main.rs` changes to argument parsing and
dispatch, and expect to rebase. Do not restructure shared code to suit yourself.

WP3 is adding a recovery ladder with per-package budgets and a parked state. You will not have its
types. Render what exists on your base; leave the surface open enough that a parked package and a
remaining budget can be added later without redesigning the page.

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

**WP1** — frozen graphs. `graph.json` at a vision directory, frozen to `graph.vN.json`, schema at
`skills/pce/schemas/work-package-graph.schema.json`. A graph carries `vision`, `plan_version`,
`authored_at_ref`, and `packages`. A package carries `id`, `title`, `repositories`, `criteria`,
`dependencies`. A criterion carries `name`, `input`, `observation`, `command`. Edges are
`buildability`, `safety`, `risk-ordering`; the first two bind readiness, the third is advisory and
overridable. Logic in `crates/core/src/work_package_graph.rs`. A real committed fixture is at
`crates/core/tests/data/rivretrieve-work-package-graph.json` — seven packages, use it.

**WP7** — the driver journal. Per package: dispatches and issuances, worker outcomes, criterion
executions carrying command, working directory, exit status, stdout and stderr, gate findings with
their witness/repair refs, accepted findings recorded as durable criterion amendments, rejections
carrying reason `witness-passed` or `repair-failed`, and parked packages. `pce package driver-status`
already derives a snapshot; read `crates/core/src/package_driver.rs` and
`docs/evidence/wp7-driver.md` before designing anything.

---

## What this is for

A human is not going to read a JSON journal to find out why a run stopped. They are going to look at
a picture, find the red node, and read the one thing attached to it.

This is also the surface where the graph gets checked. Every serious problem found in the RivRetrieve
graph so far — a missing safety edge that would have allowed compiling one country's data on an
unvalidated layout, a dependency that was over-strong, a proposed refactor that turned out to
*lengthen* the critical path — was found by looking at the drawn graph, not by reading its JSON. The
picture is not documentation of the work. It is an instrument for finding defects in the plan.

So it has to be honest about structure, not merely pretty: the edge kinds must be distinguishable at
a glance, because that is the distinction the reader is checking.

---

## What must be true when you are done

### One command, one self-contained file

The binary takes a graph and a journal and writes a single HTML file. No external stylesheet, no
CDN, no font URL, no remote image, no network at runtime. It must open correctly from `file://` and
survive being attached to a message.

Deterministic: the same graph and the same journal produce a byte-identical file. Any timestamp shown
comes from the journal, never from the clock — a rendering that differs run to run cannot be diffed,
and diffing two renderings is how a human sees what changed.

### It renders the graph as a graph

Nodes laid out in dependency order, edges drawn between them, edge kinds visually distinct. Hand-
authored inline SVG — no charting library, no runtime, no generated path soup. Lay out by dependency
depth; a graph whose arrows all point the same way is most of what makes it readable.

Seven packages must lay out legibly. So must one. Do not hard-code for the fixture.

### It renders the run over the graph

Each node carries its state — not yet ready, ready, running, complete, failed, parked — and the
states must be distinguishable without reading the labels. Colour alone is not enough; a state must
survive being printed in grey.

Attached to each node, reachable without leaving the page:

- Its criteria, each with the result of its last execution: the command, the exit status, and its
  output. **A failing criterion must show its actual output.** That is the single most useful thing
  on the page and the reason a human opens it.
- Criterion amendments accepted from gate findings, marked as amendments and attributed to the gate
  that found them, distinct from the criteria the human authored.
- Gate findings, including rejected ones with their reason. A rejected finding is evidence about the
  gate and must not be hidden merely because it was not credited.

### The critical path is computed, not annotated by hand

Longest binding-dependency chain through the graph. Mark it. It is the answer to "what is actually
holding this up", and it is currently derived by a human staring at the picture.

Where a `risk-ordering` edge lies on or near that path, it is a *choice* a human could override, and
the page should let them see that it is one. That distinction — a fact you must obey versus an
ordering you chose — is the whole reason the edge kinds are typed.

### It works on a run that has not started, and on one that has stopped

An unrun graph renders as pure structure: no states, no executions, just the plan. A stopped run
renders whatever it got to. Neither is an error case, and both must be handled without a separate
code path that can rot.

## The design system, so you do not invent one

Two hand-authored reference pages ship with this brief:

```
briefs/reference/rivretrieve-conversion.html
briefs/reference/work-package-graph.html
```

**Read them before writing any markup.** They are the target. Take the palette, the type treatment,
the node and edge drawing, and the theme handling directly from them rather than designing something
new — a rendered run and a hand-authored explanation of the same graph should look like the same
system.

Two specifics that are easy to get wrong and are already solved in those files:

**The page must be readable in both light and dark.** The viewer's theme has three states: an
explicit `data-theme="dark"` or `data-theme="light"` on the root element, and a default with neither
stamped, where only `prefers-color-scheme` distinguishes them. Define the full light palette as
tokens on bare `:root`; redefine only the tokens under `@media (prefers-color-scheme: dark)` guarded
as `:root:not([data-theme="light"])`; redefine them again under `:root[data-theme="dark"]`. Never
give a colour its only definition inside a media or `[data-theme]` block. Give `body` an explicit
token background.

**SVG colours go through the same tokens.** The reference files do this by attribute indirection so
the drawing follows the theme rather than fighting it:

```css
svg [stroke="#A8442A"] { stroke: var(--warn); }
svg [fill="#A8442A"]   { fill:   var(--warn); }
svg [fill="#FFFFFF"]   { fill:   var(--ground); }
```

Wide content scrolls inside its own container; the page body never scrolls sideways.

---

## Acceptance criteria

1. One command renders a graph and a journal to a single HTML file with no external references of any
   kind. Verify by inspection that no URL scheme other than `data:` appears.
2. The same inputs produce a byte-identical file across runs.
3. The seven-package RivRetrieve fixture renders with every package present, every dependency drawn,
   and the three edge kinds visually distinct.
4. A single-package graph renders legibly.
5. A graph with no journal renders as structure alone, without error.
6. Each package shows its state, distinguishable without relying on colour alone.
7. A failed criterion shows its command, exit status and captured output.
8. An accepted amendment is shown, marked as an amendment and attributed to its gate, distinct from
   authored criteria.
9. A rejected finding is shown with its reason.
10. The critical path is computed from binding dependencies and marked, and a `risk-ordering` edge is
    identifiable as an overridable choice rather than a fact.
11. The page renders correctly in light and dark, including every SVG stroke and fill. State how you
    checked this.
12. Every existing WP1, WP4, WP5, WP6 and WP7 test passes unchanged — the five readiness waves, the
    herdr sentinel, the RR2 briefs, the `TMPDIR` headroom assertion, the five gate tests, and all
    four driver tests including the antichain barrier.

**Plus one real render, executed and reported.** Drive a small multi-package graph to a state that
includes at least one complete package, one failed criterion with real output, and one accepted gate
amendment. Render it. Report the output path and describe what the page shows, so the rendering can
be judged without opening it.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test is known to fail intermittently and pass on rerun; it predates this work.

## Do not touch

- WP7's driver logic, materialization, replay, amendments, or journal *format* — a WP7 follow-up is
  editing that code concurrently. Read the journal; do not reshape it. If you genuinely need a field
  that is not recorded, report that rather than adding it.
- WP3's recovery ladder, in flight in another worktree.
- WP1's graph and schema. WP6's findings format.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — WP9.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`. Open questions, not work items.

## Report back with

- The command and its arguments.
- How the layout is computed, and how it behaves as the graph grows.
- How the three edge kinds and the package states are encoded, and how each survives greyscale.
- How you verified both themes.
- The real render: the graph, the run state, the output, and what the page shows.
- Anything the journal does not record that the page needed.
