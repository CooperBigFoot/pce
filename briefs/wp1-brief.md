# WP1 — the work-package graph

Make `graph.json` a real thing PCE reads: a typed dependency graph of work packages, frozen as an
immutable plan version, with a ready set the binary computes.

Decide your own method. What follows is what must be true when you are done, what will destroy live
work if you get it wrong, and what not to touch.

---

## Where to work

Branch from `repair/orchestration-dead-ends` in a **new worktree** — that branch carries verified,
unmerged work (snapshot schema v2, spending limits, parking) that this builds on.

Orient from `crates/core/src/run_state.rs`, `src/main.rs`, `skills/pce/schemas/graph.schema.json`
(today's milestone/step graph), and the `pce ready` implementation. Read
`skills/to-graph/SKILL.md` — it is the authoring doctrine this schema must serve.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool right now. The installed surface is symlinked into the
main checkout:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in the main checkout replaces the binary those runs invoke.

1. Work in a git worktree, never the main checkout.
2. Export `CARGO_TARGET_DIR` to a path of your own.
3. Never run `install.sh`. Never modify anything under the main checkout path.
4. Do not push, merge, tag, or open PRs.

**Additive only.** Today's `milestones.json` / `steps.json` path must keep working unchanged — six
runs depend on it. This adds a parallel representation; it does not replace one. Retiring the step
tier is WP9, not this.

---

## What must be true when you are done

### A graph is a committed, validated, frozen artifact

`graph.json` at a vision directory root, conforming to a new committed schema. A package carries an
id, a title, the repositories it touches, its criteria, and its typed dependencies. A criterion
carries `name`, `input`, `observation`, and `command` — where `command` is what a driver will
execute and whose exit status decides completion.

Freezing digests the bytes and records the version. A frozen version is immutable: minting `n+1`
must leave `n` readable and unchanged. Both versions coexist.

### Edges are typed, and the type is validated

Exactly three kinds, each with a different obligation:

| kind | means | must carry | binds? |
|---|---|---|---|
| `buildability` | B does not compile or run until A merged | the code fact — symbol, interface, path | yes |
| `safety` | B does something irreversible that A protects | the irreversible act | yes |
| `risk-ordering` | B after A so a defect is found cheaply | what is learned by ordering it so | **no** |

Validation is per-kind: an edge missing its required justification is invalid, not merely
undocumented.

### Transitive reduction must respect edge kind

**This is the subtle one, and it is a correctness requirement, not a nicety.**

An edge may only be omitted as transitively implied when every alternative path to it is *at least
as binding*. A hard edge whose only alternative path runs through a `risk-ordering` edge must be
stated explicitly — because overriding that soft edge would otherwise silently disconnect a
`safety` or `buildability` constraint.

Concretely, in the fixture below: `RR4` compiles Poland, which deletes the publisher artifact, so it
needs the certification harness. Its only other path to `RR2` is `RR3 →(risk-ordering) RR4`.
Override that soft edge for throughput and Poland would compile with nothing protecting it. The
`RR2 →(safety) RR4` edge is therefore explicit.

Reject a graph that omits such an edge. This rule is the reason typed edges exist at all.

### `pce ready` computes the ready set from a frozen graph

A package is ready when every **binding** dependency has merged. `risk-ordering` edges are honoured
by default and ignored under an explicit override flag; the override must be visible in the output,
never silent.

The verb keeps its existing loud-failure discipline: a digest mismatch, an unparseable graph, or a
package referencing an unknown id fails loudly with no fallback.

### The event log can record a graph

`pce log --kind planning-artifact-approved` must accept a graph-scoped node rather than only
`m<m>-s<s>`.

---

## Acceptance criteria

The fixture is the real RivRetrieve graph, derived from that vision's own acceptance criteria.
Commit it as test data.

```
RR1  shared store reader          deps: —
RR2  certification harness        deps: RR1  buildability  "read-back calls the shared reader's query path"
RR3  ca_eccc on the store         deps: RR2  safety        "the compile deletes the publisher artifact; the store becomes the only copy"
RR4  pl_imgw on the store         deps: RR2  safety        "Poland's compile deletes its artifacts too"
                                        RR3  risk-ordering "Poland falsifies the layout; a defect found here invalidates Canada's stores"
RR5  bulk surface + consent       deps: RR3  buildability  "cache_status reads the provider's store declaration from CacheConfig"
RR6  raw -> receipts              deps: RR1  buildability  "the store_excerpt receipt is built from the reader's executed query and manifest facts"
RR7  retire legacy + document     deps: RR4  buildability  "deleting the legacy subtree requires the port to have removed its references"
                                        RR5  buildability  "the documented surface must exist"
                                        RR6  buildability  "the receipts name must be final before it is documented"
```

These must all pass, as tests:

1. Ready set of the fixture at zero merged packages is exactly `{RR1}`.
2. With `RR1` merged: `{RR2, RR6}`. With `RR2, RR6` merged: `{RR3}`. With `RR3` merged:
   `{RR4, RR5}`. With `RR4, RR5` merged: `{RR7}`.
3. With the risk-ordering override and `RR2` merged, `RR4` becomes ready while `RR3` is unmerged —
   and the output states that an override was applied.
4. Removing the explicit `RR2 →(safety) RR4` edge makes the graph **invalid**, because the only
   remaining path to `RR2` runs through a soft edge.
5. An edge missing the justification its kind requires is rejected, one case per kind.
6. A cyclic graph is rejected.
7. A package with no criteria is rejected. A criterion with no `command` is rejected.
8. Freezing version 2 leaves version 1 byte-identical and readable.
9. A digest mismatch between a recorded approval and the current bytes fails loudly.
10. `milestones.json` / `steps.json` behaviour is unchanged — the existing `pce ready` tests still
    pass untouched.

Plus the repository's own gates.

---

## Do not touch

- **The step tier.** No deletion, no deprecation. WP9 owns that.
- **Dispatch, spawn, liveness, herdr, prime-agent.** WP4 and WP5.
- **The authoring pass** — turning a vision into a graph is WP2 and lives in a skill, not the
  binary. This package only reads, validates, freezes and computes readiness.
- **Executing criteria commands.** The driver runs them; WP7. Expose them, do not invoke them.
- **The fourteen open `pce:ticket` issues.** Open questions, not work items.

## Report back with

- The schema, and which validation rules are enforced where.
- How transitive reduction respects edge kind, and the test that proves it.
- Whether anything in the fixture graph could not be expressed.
- Anything you found that contradicts what this brief assumes.
