# PCE workflow feedback: 2026-08-19-incidence-python-binding

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill, session 3a3b5e2d`
- Run: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-19-incidence-python-binding`, graph.v1.json, plan version 1
- Outcome: `blocked` (driver-status `outcome: "blocked"`; IPB1-IPB4 complete, IPB5 and IPB6 parked, IPB7 pending)

## Executive summary

The driver executed four of seven packages to completion unattended in about 65 minutes, including
gate-found repairs on all four. It then parked two packages and exited cleanly. The mechanics worked.

The highest-impact findings are about what survives a park, not about whether parks happen. A parked
package's only durable explanation is a single typed fault with no free-text field, and the worker's
pane is destroyed before a supervisor can read it. For IPB5 the fault was reconstructible from the
composed base in about ten minutes. For IPB6 it was not reconstructible at all: the fault id names a
capability rather than a package, no measurement was committed, and the worker's reasoning is gone.
A second finding concerns a graph-authoring gap that the freeze accepted: IPB5 declares a dependency
on work that no IPB4 criterion obliges IPB4 to build, so IPB4 completed legitimately while leaving
IPB5 unbuildable.

## Evidence reviewed

- `planning/2026-08-19-incidence-python-binding/driver-journal.jsonl` (102 records, 105,988 bytes)
- `planning/2026-08-19-incidence-python-binding/graph.v1.json` (7 packages, plan version 1)
- `package-outcomes/IPB5/5.json`, `package-outcomes/IPB6/6.json`
- `.pce/package-briefs/IPB5/5.md`
- `pce package driver-status --graph graph.v1.json --journal driver-journal.jsonl`
- `herdr pane read wYY:p1 / wYZ:p1 / wYY:p2 / wYZ:p2`; `herdr workspace get wYY / wYZ`
- Git observations at `26862998eab1f0e601da3a80c06c4ee6ff56d4b3` (IPB5 base) and
  `8eb551d911fd03db5bc485d81d448843bafc5dfe` (IPB6 base)
- `planning/2026-08-19-incidence-python-binding/supervision.md` (this run's captured evidence)

## What worked

### Gate-then-reproof on every completed package

- Evidence: each of IPB1-IPB4 shows `gate-finished`, `package-repair-merged` with a `previous_oid`
  and `hardened_oid`, then `gate-reproof-executed` for every authored criterion *plus* the new
  amendment criterion. Example: IPB2 `finding-replayed` produced `tests/test_deep_nesting.py`,
  merged as repair_ref `a84fc0e04c6c84abc913cc22fbf394077313be98`, and all three criteria re-ran.
- Effect: four packages landed with a strictly larger criterion set than authored, and the
  re-execution proves the repair did not break the authored criteria.

### Gate dispatch failure recovered without human involvement

- Evidence: `{"event":"gate-failed","package":"IPB3","issuance":3,"gate":"package-gate-3-1",
  "reason":"gate dispatch stopped without an outcome","detail":"gate dispatch exit status: Exited { code: ExitCode(1) }"}`
  followed immediately by `gate-dispatched` attempt 2 as `package-gate-3-2`, which finished normally.
- Effect: a transient dispatch failure cost one retry rung and no human attention. IPB3 completed.

### Parked workspaces are not cleaned up

- Evidence: `dispatch-pane-cleanup` records exist for IPB1-IPB4 workspaces but not for `wYY` (IPB5)
  or `wYZ` (IPB6). Both worktrees were still present and readable after the driver exited, at
  exactly their `package-base-composed` base oids, with clean status and empty `git diff --stat`.
- Effect: the composed base of each parked package remained inspectable, which is what made the
  IPB5 refutation attempt possible at all.

## Friction and failures

### A `mis-specified` outcome carries no rationale field

- Severity: `high`
- Phase: `execution / escalation`
- Observation: `package-outcomes/IPB6/6.json` is, in full,
  `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"basin-scale core execution scalability"}}`.
  That is the entire durable record of why a package the human must now rule on was refused.
  The `package-parked` journal record adds nothing beyond the same string.
- Evidence: the outcome contract in `.pce/package-briefs/IPB5/5.md` section "Required outcome" defines
  four shapes; only `failed` has a free-text field (`blocked_by`). Both `mis-specified` shapes carry
  ids only.
- Inference: the schema treats a mis-specification as machine-routable, but `missing-dependency` with
  a non-package id is not routable — no graph edge can satisfy `basin-scale core execution scalability`.
  The one case that most needs prose is the one case that cannot carry it.
- Impact: I could not determine whether IPB6's worker meant "the engine is too slow" or "no basin-scale
  model document exists to measure". I spent roughly 20 minutes building the binding from the IPB6 base
  and writing a scaling probe to produce a number for the human, and still could not settle the worker's
  intent. The human now rules with a measurement I made rather than the reasoning the worker had.

### The worker pane is destroyed before a park can be read

- Severity: `high`
- Phase: `recovery / evidence preservation`
- Observation: `dispatch-worker-identified` records the worker agent at pane `wYY:p2` / `wYZ:p2`.
  `herdr pane read wYY:p2` and `wYZ:p2` both returned
  `{"code":"pane_not_found","message":"pane wYY:p2 not found"}`. The surviving pane `p1` in each
  workspace contained one line: the shell prompt.
- Evidence: no `dispatch-pane-cleanup` record exists for either workspace, so the workspace itself was
  deliberately retained — yet the agent pane inside it was gone anyway.
- Inference: workspace retention on park and worker-pane lifetime are governed separately. Retaining the
  workspace preserves the worktree but not the reasoning.
- Impact: the `/work-graph` skill's section 5 instructs the supervisor to capture
  `herdr pane read <pane_id>` as evidence item 2 before analysis. For a park — the exact interrupt that
  section exists to serve — that item was unobtainable for both packages.

### `driver-status` has no top-level `status` field

- Severity: `low`
- Phase: `terminal boundary`
- Observation: the skill's section 7 speaks of `driver-status` being `Finished` or `Blocked`. The actual
  JSON has no `status` key; the terminal verdict is `outcome: "blocked"` (lowercase), alongside
  `assembly`, `base_currency_acceptances`, `ready`, `packages`, `amendments`, `recovery`.
- Evidence: `pce package driver-status ... | python3 -m json.tool` — first attempt to read `.status`
  returned `None`.
- Inference: skill prose and binary output drifted, or the skill uses the driver's own printed status
  vocabulary rather than the JSON field name.
- Impact: one wasted command. No incorrect conclusion, because `outcome` and empty `ready` are unambiguous.

### Two packages were dispatched concurrently and both parked on the same underlying gap

- Severity: `low`
- Phase: `execution`
- Observation: IPB5 (issuance 5) and IPB6 (issuance 6) were dispatched back to back after IPB4 completed,
  and both parked. IPB5's park is about a missing accessor on IPB4's surface; IPB6's is about a missing
  basin-scale model.
- Evidence: consecutive `worker-dispatched` records with no intervening completion, then two
  consecutive `package-parked` records.
- Inference: these are two distinct gaps, not one; the concurrency did not cause either.
- Impact: none observed. Recorded only to note that concurrent dispatch did not amplify the failure.

## Recommendations

### Give both `mis-specified` shapes a required free-text field

- Addresses: "A `mis-specified` outcome carries no rationale field"
- Change: extend the outcome contract to
  `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"...","rationale":"..."}}`
  and the same for `kind:"criterion"`, with `rationale` required and non-empty. Journal it verbatim on
  `package-parked`.
- Location: the package worker outcome schema, and the "Required outcome" section of the brief template
  emitted by `pce package brief`.
- Trade-off: one more field a worker can fill badly; the park record grows.
- Confidence: `high`

### Retain the worker agent pane whenever the workspace is retained

- Addresses: "The worker pane is destroyed before a park can be read"
- Change: on `package-parked`, keep the pane recorded in `dispatch-worker-identified` alive (or capture
  its scrollback to `<vision-dir>/park-evidence/<PACKAGE>/<ISSUANCE>.txt`) with the same lifetime rule
  already applied to the workspace.
- Location: the driver's park path, adjacent to wherever it suppresses `dispatch-pane-cleanup` for a
  parked package.
- Trade-off: panes accumulate for parked packages until the human resolves them; a scrollback file costs
  disk instead.
- Confidence: `high`

### Reject a `missing-dependency` id that is not a package id in the graph

- Addresses: "A `mis-specified` outcome carries no rationale field" (the routability half)
- Change: when a worker reports `kind:"missing-dependency"` with an `id` absent from the graph's package
  ids, journal it as a distinct event — a mis-specification against the plan rather than a dependency
  complaint — so the supervisor and the human can tell the two apart mechanically.
- Location: driver outcome interpretation, `package-parked` emission.
- Trade-off: a worker that mistypes a real package id gets a differently-shaped park.
- Confidence: `medium`

### Align the skill's terminal-status vocabulary with the binary's JSON

- Addresses: "`driver-status` has no top-level `status` field"
- Change: in `/work-graph` section 7, say the terminal verdict is the `outcome` field with values
  `blocked` / `finished`, rather than `Finished` / `Blocked`.
- Location: `.claude/skills/work-graph/SKILL.md`, section 7.
- Trade-off: none.
- Confidence: `high`

## No-change decisions

- **The IPB3 gate dispatch failure.** It self-recovered on the next attempt and cost one retry rung.
  A single transient `ExitCode(1)` from gate dispatch does not justify a workflow change.
- **The graph defect itself** — IPB5 depending on a log accessor that no IPB4 criterion requires — is a
  `/to-graph` authoring problem, not a `/work-graph` orchestration problem. `pce graph check` reported
  `valid: true`, correctly: a dependency `reason` is prose and cannot be mechanically checked against the
  dependee's criteria. Proposing that it should be is a design change well beyond this report's evidence.
  Recorded under follow-up instead.

## Suggested follow-up

- Consider whether `/to-graph` should surface, at authoring or freeze time, every dependency whose
  stated `reason` names a capability that no criterion of the dependee package mentions. In this run that
  check would have caught IPB5 -> IPB4 before any work started, at the cost of a heuristic on prose.
  This is an experiment, not a required change: the same heuristic would likely produce false positives
  on well-authored graphs.
