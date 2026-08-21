# PCE workflow feedback: 2026-08-20-taqsim-modelling-layer-on-incidence (second report)

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `planning/2026-08-20-taqsim-modelling-layer-on-incidence` in `taqsim`, journal `driver-journal.jsonl`, now on `graph.v2.json`
- Outcome: `blocked three times; two human rulings and one supervisor overrule applied; TQ2 unresolved with its overrule exhausted`

## Executive summary

The first report for this run was archived to `archive/2026-08-20/` while the run was still going, so
this is a continuation covering what happened afterwards rather than a revision of it. Its findings
stand and are not repeated.

The headline finding is that **a park-overrule cannot change worker behaviour**. The supervisor
refuted a false complaint at a named ref, spent `TQ2`'s one-shot overrule, and the redispatched brief
was byte-identical to the pre-overrule brief apart from a worktree path. The rationale — which named
the exact oid and the required edit — never reached the worker. `TQ2` parked again on the same
reason, now with no door left, and became a human ruling. The allowance designed to prevent
escalation was consumed producing one.

Closely related: a worker's brief names its dependencies and describes them in full, tells the worker
those packages are "someone else's work", and gives no address for their output. Three separately
dispatched workers independently concluded the same capability was missing.

Also recorded: a low-severity `pce graph freeze` diagnostic that points an operator at a lock file
when the vision directory is the thing that does not resolve; and, on the positive side, a pattern
worth keeping — across five completed packages every defect found in this run was found by a gate and
none by an authored criterion, including in a package whose criteria this supervisor authored.

## Evidence reviewed

- `planning/2026-08-20-taqsim-modelling-layer-on-incidence/driver-journal.jsonl` (81 events through the run-2 terminal boundary, continuing under plan version 2)
- `planning/2026-08-20-taqsim-modelling-layer-on-incidence/supervision.md`
- `pce package driver-status` at the run-2 terminal boundary
- `graph.v1.json`, drafted `graph.json`, frozen `graph.v2.json`, and `events.jsonl`
- The human's failed and succeeding `pce graph freeze` invocations as pasted into the session

## What worked

### Package gates found every defect in this run; authored criteria found none

- Evidence: four packages completed, each with exactly one accepted gate finding, and in every case
  the authored criteria passed at exit 0 both before and after the repair:
  - `IN1` — `power_operation_preserves_v2_numerical_semantics_when_roundtripped`; a power expression
    roundtripped through the model document returned `String("v1")` instead of `"v2"`. Authored
    criterion `Power operation computes exactly` passed regardless.
  - `TQ1` — `test_build_refuses_a_subsecond_start_before_compilation`; `DID NOT RAISE ValueError`.
    Authored criterion `Time is not optional` covers a *missing* start, not a sub-second one.
  - `TQ3` — `test_saved_run_refuses_tampered_cached_flow`. Authored criterion `A saved run refuses a
    stranger` covers a tampered version *stamp*, not a tampered *payload*.
- Effect: three real defects caught, each one step adjacent to what its authored criterion proves.
  The recurring shape is that an authored criterion proves the case it names and the gate proves the
  neighbouring case the author did not think to name. This is the clearest evidence in this run for
  the gate mechanism paying for its cost — worth having on record before anyone proposes trimming
  gates to save workspaces, since gates roughly doubled the workspace count per package.
- Caveat, stated because the sample is small: four packages in one run, one graph, two repositories.
  This is a pattern worth watching, not a measured rate.

### Re-resolving the frozen graph from disk caught nothing, and that is the point

- Evidence: after the human froze plan version 2, `pce graph check --file graph.v2.json` returned
  valid with the expected warning, and `diff graph.json graph.v2.json` was empty — confirming the
  frozen artifact was byte-identical to the draft that had been checked.
- Effect: cheap confirmation that nothing was edited between drafting and freezing. The rule that the
  supervisor re-resolves from disk rather than trusting the prior conversation cost two commands and
  makes a whole class of drift undetectable-to-detectable.

## Friction and failures

### `graph freeze` reports a lock-file error when the vision directory itself is unresolvable

- Severity: `low`
- Phase: `freeze`
- Observation: the human ran
  `pce graph freeze --vision-dir planning/2026-08-20-taqsim-modelling-layer-on-incidence --repository …`
  from a different repository (`zarafshan-taqsim`) than the one owning the vision (`taqsim`). It
  failed with:

  ```
  Error: failed to open graph freeze lock planning/2026-08-20-taqsim-modelling-layer-on-incidence/.pce-graph-freeze.lock

  Caused by:
      No such file or directory (os error 2)
  ```

- Evidence: `.pce-graph-freeze.lock` exists and is well-formed at
  `taqsim/planning/2026-08-20-taqsim-modelling-layer-on-incidence/.pce-graph-freeze.lock` (0 bytes,
  mode 600, present since 17:21).
  `zarafshan-taqsim/planning/2026-08-20-taqsim-modelling-layer-on-incidence` does not exist. The same
  command succeeded unchanged when re-run from `taqsim`.
- Inference: the relative `--vision-dir` is resolved and the lock open is attempted without first
  checking that the vision directory exists, so the first failure surfaces at the deepest path
  touched rather than at the argument that was actually wrong.
- Impact: one operator round trip. More than cosmetic because the natural response to "failed to open
  graph freeze lock" is to investigate or delete the lock file, and a `.pce-graph-freeze.lock` is
  exactly the kind of file an operator will remove to unstick a tool. Here nothing about the lock was
  wrong.

### A park-overrule clears driver state but conveys nothing to the next attempt

- Severity: `high`
- Phase: `recovery`
- Observation: `TQ2` parked (issuance 7) claiming `incidence` lacked expression-derived multi-branch
  partitions. The supervisor refuted this at a named ref and issued `pce package driver-overrule`
  with a rationale naming the exact oid, the evidence command, and the required action ("bump
  `[tool.uv.sources] incidence` rev from `3eef8c8…` to `3a64a54…` in `pyproject.toml`"). The driver
  accepted the overrule and redispatched. `TQ2` parked again at issuance 8 with substantially the
  same complaint, still probing `pyproject.toml incidence revision 3eef8c8…`.
- Evidence:
  - `diff .pce/package-briefs/TQ2/7.md .pce/package-briefs/TQ2/8.md` differs on exactly one line —
    the worktree path (line 155). Every other byte is identical.
  - Grepping brief 8 for `3a64a54`, `overrule`, `rationale`, `bump` returns no matches.
  - The rationale is present in the journal as `package-park-overruled`.
- Inference: the overrule is modelled purely as driver-side state ("this park no longer blocks"), and
  the brief is regenerated from the graph without consulting the overrule history. The rationale is
  therefore write-only — it reaches the audit trail and the human, and never the agent whose behaviour
  it is meant to change.
- Impact: high, and specifically because the overrule is one-shot per package. A supervisor that
  correctly refutes a false complaint spends the package's only cheap door on a redispatch that is
  bit-for-bit as uninformed as the one that just failed. The likely outcome is a second identical
  park with no door remaining, escalating to graph revision — which is exactly what happened here.
  The mechanism converts a recoverable situation into a human ruling while consuming the allowance
  designed to prevent that.

### A worker's brief names its dependencies but gives no address for their output

- Severity: `high`
- Phase: `execution / handoff`
- Observation: `TQ2`'s brief describes `IN2` fully (title, all criteria, and the dependency reason at
  line 160), then states at line 195: `IN1, IN2, TQ1, TQ3, TQ4, and TQ5 are out of bounds. They are
  someone else's work.` The worktree it is handed pins `incidence` at a rev predating `IN2`. Nothing
  in the brief says where `IN2`'s output is or that the pin may need updating.
- Evidence: `.pce/package-briefs/TQ2/8.md` lines 78, 160, 195; `pyproject.toml` at the composed base
  `8cd855f…` pinning `rev = "3eef8c8877dd98e464d640bee05a894a3855767c"`; three consecutive parks
  (issuances 4, 7, 8) from three separately dispatched workers all concluding the capability was
  missing.
- Inference: the worker's reading is defensible rather than careless. Told that a capability comes
  from a package that is out of bounds, and finding it absent at the only revision it can see, "this
  is mis-specified" is the correct conclusion from the information given. Three independent workers
  reaching it supports that this is the brief's shape, not worker variance.
- Impact: a package cannot consume its own declared dependency's output. Combined with the first
  finding in the archived report (cross-repository edges deliver nothing), the run required two
  manual `git push` interventions and still could not complete `TQ2`.

### The dirty-source guard refuses to run when the run's own state is what makes the source dirty

- Severity: `high`
- Phase: `driver launch`
- Observation: after installing `pce` `273f5c34…`, `driver-run` aborted immediately:
  `{"event":"driver-aborted","reason":"source repository `taqsim` is dirty; commit, stash, or remove
  its changes before driver-run"}`. `TQ2` was never dispatched.
- Evidence: `git -C taqsim status --short --branch` reports ` M uv.lock`, `?? CONTEXT.md`, `?? docs/`,
  `?? planning/`, none gitignored. This state is **unchanged since the first launch in this session**,
  and three prior `driver-run` launches proceeded against it without complaint. `planning/` contains
  the vision directory: `driver-journal.jsonl`, `.pce/package-briefs/`, `package-outcomes/`,
  `supervision.md`, and preserved park evidence.
- Inference: the guard is new in this binary. The guard itself is defensible — composing package bases
  from a dirty source is not reproducible. The problem is its interaction with artifact location: the
  driver writes its journal, briefs, and outcomes into `planning/` inside the repository it requires
  to be clean, so the driver's own output is a source of the dirtiness it refuses to tolerate.
  Committing `planning/` defers the problem to the next journal append rather than solving it.
- Impact: blocks the run outright. Worse, every remedy the message suggests is destructive here:
  `git stash -u` and `git clean` remove untracked `planning/` and would delete the driver journal —
  the artifact the workflow designates as the admissible run proof. An operator following the error
  message literally destroys the run's evidence to satisfy a cleanliness check. The supervisor
  refused all three remedies and escalated instead.

## Recommendations

### Validate the vision directory before opening the freeze lock

- Addresses: "`graph freeze` reports a lock-file error when the vision directory itself is unresolvable".
- Change: stat `--vision-dir` first and, when it does not exist, fail naming the vision directory and
  its resolved absolute path, before any attempt to open the lock.
- Location: `pce graph freeze` argument handling, before lock acquisition.
- Trade-off: one extra stat per freeze; no change on the success path.
- Confidence: `high`. The correct and more useful error is available before the misleading one is
  produced.

### Carry overrule rationales into the redispatched brief

- Addresses: "A park-overrule clears driver state but conveys nothing to the next attempt".
- Change: when a package is redispatched after `package-park-overruled`, include the prior park reason
  and the overrule rationale verbatim in the brief, under a heading that marks the prior complaint as
  refuted and names the evidence. The data is already in the journal; only the brief generation needs
  to read it.
- Location: package brief generation, wherever the prior-attempt context is assembled.
- Trade-off: briefs grow for redispatched packages, and a wrong overrule rationale would now actively
  mislead a worker rather than merely being recorded. That is the correct trade: an overrule is
  already an authoritative supervisor claim, and one that cannot reach the worker is strictly worse
  than one that can be wrong.
- Confidence: `high`. Without this, the one-shot overrule cannot change worker behaviour at all, which
  appears to be its main intended purpose.

### State a dependency's resolvable location, not just its identity, in the brief

- Addresses: "A worker's brief names its dependencies but gives no address for their output"; also the
  cross-repository delivery finding in the archived first report.
- Change: for each entry under the brief's `Dependencies:` section, add the dependency's completed oid
  and how to reach it — the composed base for a same-repository dependency, or a materialized path or
  pushed ref for a cross-repository one. Where the dependent must update a declaration (a pinned rev,
  a lockfile) to consume it, say so explicitly, since the scope-boundary paragraph otherwise reads as
  forbidding exactly that edit.
- Location: package brief generation, `Dependencies` section; and the scope-boundary text, which should
  distinguish "do not implement another package's work" from "do not update your own declaration of it".
- Trade-off: the brief must know delivery locations, which for cross-repository dependencies do not
  currently exist — so this recommendation is downstream of implementing delivery at all.
- Confidence: `high` on the ambiguity being real and load-bearing; `medium` on the fix shape, since it
  depends on how cross-repository delivery is eventually solved.

### Exclude the vision directory from the dirty-source check, or state a non-destructive remedy

- Addresses: "The dirty-source guard refuses to run when the run's own state is what makes the source dirty".
- Change: exclude the vision directory (and anything under it) from the dirty-source determination,
  since the driver itself writes there. Independently, the abort message should not recommend
  `stash` or `remove` without qualification while the run's journal can live in the working tree —
  name the offending paths explicitly so an operator can see whether the journal is among them.
- Location: the dirty-source check in `driver-run` preflight, and its abort message text.
- Trade-off: excluding a directory from a cleanliness check narrows the guarantee slightly; the vision
  directory does not participate in base composition, so the reproducibility argument does not apply
  to it.
- Confidence: `high` on the interaction being a real blocker (observed, reproducible); `high` on
  listing offending paths being an improvement regardless of the exclusion decision.

## No-change decisions

- **`events.jsonl` gained no record for the human's freeze.** Only the plan-version-1
  `planning-artifact-approved` entry is present. This is correct: that bootstrap append belongs to
  the supervisor's `--mechanical` freeze path, and an ordinary human freeze is its own ratification.
  Recorded because its absence could otherwise read as a missing audit entry.
- **`authored_at_refs` unchanged between `graph.v1.json` and `graph.v2.json`.** Consistent with a
  repartition that adds a package rather than re-authoring against new refs. No change.
- **Two workspaces left open at the run-2 terminal boundary** (`w15H` for `TQ1`'s overruled attempt,
  `w16D` for `TQ2`'s parked attempt). The supervisor deliberately did not close them, since closure
  requires proof the run is retired and this run was paused pending a freeze. The rule behaved
  correctly; noting only that an open workspace whose run is *paused* is indistinguishable from an
  orphan without reading the journal, which the supervisor did.

## Suggested follow-up

- If the gate-versus-criterion pattern above holds across more runs, it is worth quantifying
  deliberately: count accepted gate findings against authored-criterion failures per package over
  several visions. That would turn a suggestive four-package observation into an actual argument
  about where proof effort belongs. Filed as an experiment, not a change.
