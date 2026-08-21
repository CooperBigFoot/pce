# Brief: a graph must say which repository each authored ref belongs to

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; the
evidence is three independent runs hitting the same wall in one week. Examine it, decide,
implement, and record what you decided and why in your completion report and CONTEXT.md.
Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion), stop
and report.

## The defect, thrice proven

A graph carries exactly one `authored_at_ref` (`crates/core/src/work_package_graph.rs:22`),
and composition uses it as the base for EVERY repository a package names
(`compose_git_commits` callers; resolution via `git rev-parse --verify <ref>^{commit}`).
`pce graph check` only requires the field be non-empty (`work_package_graph.rs:276`).

1. **bluesmith/stopwatch (2026-08-18):** authored_at_ref was a bluesmith oid; stopwatch can
   never resolve it. All six packages uncomposable; six byte-identical
   `package-composition-failed` events exhausted the entire `environment_failures: 6` budget
   in under one second, before any worker existed. Recovered only by freezing v2 with
   symbolic `"HEAD"`.
2. **pourpoint/hfx (2026-08-18):** authored_at_ref 0f0bada resolves in pourpoint only;
   launch blocked pre-flight, v2 revision required before first dispatch.
3. **palaestra/orthographos (2026-08-17):** the authoring orchestrator foresaw the trap and
   wrote `"main"` — pinning nothing. Its own report: "the graph does not pin what it was
   authored against; it pins a moving target."

The workaround (a symbolic ref) survives only because `package-base-composed` records the
resolved oid in the journal — proof stays exact, but the frozen plan stops stating its own
ground, and a mid-run `git pull` in any source worktree silently moves every later base.

## What to build (decide the details yourself)

Per-repository authored refs, expressible in the graph, verified before they can hurt:

- **Schema:** how a graph states one ref per repository name. Decide the shape (e.g. a map
  alongside or replacing the scalar) and the back-compatibility rule for the scalar form —
  three frozen graphs in the wild use it (incidence v1–v5, gridded-statics v1–v2, bluesmith
  v2 once frozen), and old journals must keep deriving.
- **Check:** `pce graph check` (and freeze) verifies each declared ref RESOLVES in a
  repository — decide how check learns repository paths (it currently reads only the graph;
  candidates: an optional `--repository NAME=PATH` on check, verification deferred to
  driver-run preflight, or both). The bluesmith failure mode — burning the environment
  budget on a ref that could never resolve — must become a single pre-flight refusal that
  names the repository and the ref.
- **Composition:** each repository composes at its own declared ref. Decide what happens
  when a repository is named by a package but has no declared ref (refuse at freeze, most
  likely).
- **Plan-version transitions:** `ensure_driver_plan_version` compares graphs; decide how
  ref changes interact with completion carry-forward (a changed ref for a repository whose
  packages are complete — carried or invalidated? The journal's recorded base oids are the
  evidence to reason from).
- **Authoring:** `skills/to-graph/SKILL.md` step 8's frozen shape and the multi-repository
  guidance must show the per-repo form, and step 2's descent obligation should name the ref
  per repository it read.
- **The three live visions:** state in your report what each should do after this lands
  (likely: nothing forced — symbolic refs keep working; next natural plan version can pin).

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at a02ea6f or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  `./install.sh` from the MAIN checkout only. Do NOT install: multiple drivers are live and
  the human/supervisor coordinates installs at quiet boundaries.
- Key code: `crates/core/src/work_package_graph.rs` (schema, parse, check :276);
  `src/main.rs` composition (`compose_git_commits` and its callers for package bases and
  assembly, ref resolution ~:5177), `ensure_driver_plan_version`, `run_graph_freeze`;
  `skills/to-graph/SKILL.md`.
- Evidence: bluesmith vision dir
  `/Users/nicolaslazaro/Desktop/work/bluesmith/planning/2026-07-29-signal-bearing-dudh-warm-window`
  (journal with six identical composition failures, supervision.md); pourpoint vision dir
  (pre-flight block, supervision.md); palaestra graph v1/v2 (`"main"` workaround) and its
  orchestrator's findings in `briefs/palaestra-to-graph-prompt.md`'s follow-ups.
- Original ticket lineage: Program #37 pending finding #4 ("graph schema has a single
  authored_at_ref, cannot say which repository a ref belongs to; blocks a genuinely
  multi-repository vision") — this brief discharges it; note that in the report so the
  eventual ticket write-up links here.
