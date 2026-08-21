# PCE workflow feedback: the worker environment must be declared before it can be known

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 7, GD2 issuance 11
- Outcome: `blocked` — a granted human authorization cannot reach the worker that requested it

## Executive summary

A package parked asking for a human authorization. The human granted it. The driver then refused
to start, because the worker environment was declared as `{}` at the first launch on the previous
day and is immutable for the life of the journal.

**The worker environment must be declared before the first dispatch, but which environment a
package needs is only discoverable by dispatching it.**

This is not a bug in any single check — each mechanism is individually correct. `--worker-env` is
the only channel into a worker; the declaration is recorded once and enforced journal-wide, which
is what makes it auditable; and the refusal is loud and precise. Together they produce a state
where a legitimate, human-granted authorization has no path to the process that needs it, and the
only remedies available to the supervisor are to abandon proofs already earned or to defeat the
mechanism.

The parallel with the criterion-revision door is exact. Criteria were also immutable for good
reasons, and the same deadlock appeared (this run, report -1, findings 3 and 5). It was resolved
not by making criteria mutable but by adding an explicit, human-ratified revision record. The same
shape would resolve this.

## Evidence reviewed

- `planning/.../driver-journal.jsonl` — 92 events; `worker-environment-declared` count `1`;
  `recovery-configured` count `1`; final record `driver-aborted`
- `planning/.../package-outcomes/GD2/11.json`
- `planning/.../run.json` after the supervisor added `worker_env`
- `pce` at installed head `9d3d7a1`: `crates/core/src/package_driver.rs:834-846`,
  `src/main.rs:2145-2155`, `src/main.rs:2749-2757`
- `pourpoint` at `ba1fe09`: `scripts/released_wheel_proof.py:199-218`, `:417-427`, `:470-489`

## What worked

### The gate did exactly what the vision asked of it

- Evidence: `package-outcomes/GD2/11.json` →
  `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"GD2 human authorization for read-only bounded hosted COG access"}}`
- Effect: the worker hit `validate_live_environment` (`released_wheel_proof.py:418-420`), could not
  proceed without the explicit token, and reported that as a typed outcome naming **its own
  package** and a human input — rather than inventing a workaround or failing obscurely. This is the
  vision's "missing opt-in is failure, never a passing skip" working end to end, and it is the most
  precise fault shape observed in this run.

### The refusal named the exact conflict

- Evidence: `{"event":"driver-aborted","reason":"driver worker environment is already declared as {}, not {\"POURPOINT_LIVE_READ_AUTHORIZATION\", \"POURPOINT_RELEASE_WHEEL\"}"}`
- Effect: both sets printed, no guessing required. The diagnosis took one grep of the source to
  confirm rather than an investigation.

### There is no back door, and that is correct

- Evidence: `route_environment` (`src/main.rs:2749-2757`) builds the worker environment from
  `command.worker_environment` plus exactly `PATH`, `HOME`, `USER`. Setting a variable in the
  driver's own environment without declaring it does not reach the worker.
- Effect: the declaration cannot be bypassed, which is what makes it worth having. Recorded here as
  a property to preserve, not a gap to close.

## Friction and failures

### 1. A journal-scoped declaration cannot absorb a requirement discovered mid-run

- Severity: `high`
- Phase: `launch configuration / execution`
- Observation: this run declared `{}` at its first launch on 2026-08-18, when nothing indicated any
  worker environment was needed. On 2026-08-19, at plan version 7 and issuance 11, GD2 discovered it
  needs `POURPOINT_LIVE_READ_AUTHORIZATION` and `POURPOINT_RELEASE_WHEEL`. Relaunching with
  `--worker-env` for those two names aborts.
- Evidence: `package_driver.rs:834-842` performs the check in the pre-pass over **every** journal
  event, not the plan-version-scoped `active_events` slice, returning `ConflictingWorkerEnvironment`
  on any differing later declaration. Corroborated by this journal: across seven plan versions and
  nine launches, `worker-environment-declared` appears exactly once, as does the identically-treated
  `recovery-configured`.
- Inference: the declaration is deliberately once-per-journal so the worker environment is a single
  auditable fact. The cost is that it must be predicted before the first dispatch.
- Impact: the run is blocked with two packages proven (GD1, GD10) and a valid human authorization
  that cannot be delivered. The supervisor's only unaided options are to start a fresh journal —
  discarding both proofs, including two accepted gate findings and their witness/repair refs — or to
  defeat the mechanism, which it declined to do.

### 2. A plan-version advance resets less than it appears to

- Severity: `medium`
- Phase: `graph revision`
- Observation: `PlanVersionAdvanced` clears `disputed_parks` and `consumed_overrules`
  (`package_driver.rs:1000-1002`) and re-scopes package state, so a new plan version reads as "a new
  specification for this run". It does **not** reset the worker environment or the recovery limits,
  which are fixed at first launch.
- Evidence: this journal — seven plan versions, one `recovery-configured`, one
  `worker-environment-declared`.
- Inference: the split is intentional (spending limits and environment are run-level; specification
  is plan-level).
- Impact: a supervisor reasonably expects a new plan version to be able to carry new launch
  configuration, since it can carry new packages, new edges, and — since today — ratified criterion
  revisions. Nothing signals the distinction until a launch aborts.

### 3. `tmux respawn-pane` needs absolute paths, and the failure is a bare status code

- Severity: `low`
- Phase: `launch`
- Observation: the first relaunch attempt used a bare `env …` prefix and a bare `pce`. The pane died
  with status **127** and no message, because `respawn-pane` executes the command vector directly
  with no shell and therefore no `PATH` resolution of the intended kind.
- Evidence: `tmux capture-pane` → `Pane is dead (status 127, …)`. Corrected with `/usr/bin/env` and
  `/Users/nicolaslazaro/.local/bin/pce`.
- Impact: minor and self-inflicted, but the skill's section 3 prescribes `respawn-pane` without
  mentioning that every binary in the vector must be an absolute path. Worth one sentence.

## Recommendations

### Allow the worker-environment declaration to be extended at a plan-version boundary, with an explicit human record

- Addresses: findings 1 and 2
- Change: mirror the criterion-revision door. Refuse a differing declaration by default, as now; but
  accept one when the freeze (or the launch) is accompanied by an explicit human record naming the
  added variables and a rationale, published beside the graph as
  `graph.v{N}.worker-environment.json` and replayed in `driver-status` exactly as
  `criterion_revisions` now is. Additions only — never silent replacement of an existing name's
  meaning.
- Location: `crates/core/src/package_driver.rs:834-842` for the acceptance rule;
  `run_graph_freeze` or `driver-run` for the record; `run_render.rs` for surfacing it.
- Trade-off: a second human-ratified record type to maintain. The audit property is preserved —
  arguably strengthened, since the addition carries a stated reason rather than being decided
  silently on day one.
- Confidence: `high` — this is the same problem the criterion-revision door already solved, and the
  same solution shape.

### State the once-per-journal facts in the skill's launch configuration section

- Addresses: finding 2
- Change: say explicitly that `worker_env` and the recovery limits are declared once for the life of
  the journal and cannot be changed by a plan-version advance, and that `worker_env` should
  therefore be considered at first launch even when no package appears to need it. In this vision the
  requirement was discoverable in advance: `scripts/released_wheel_proof.py:199-202` names both
  variables, and `vision.md` states network checks are explicit and loud.
- Location: `skills/work-graph/SKILL.md` section 2.
- Trade-off: none.
- Confidence: `high` — this alone would have prevented this block, since a supervisor reading the
  harness before launch would have declared both names on day one.

### Note the absolute-path requirement for relaunch

- Addresses: finding 3
- Change: one sentence in section 3 — `respawn-pane` runs the vector with no shell, so every binary
  must be given as an absolute path.
- Location: `skills/work-graph/SKILL.md` section 3.
- Trade-off: none.
- Confidence: `high`

## No-change decisions

- **`route_environment` passing only declared names plus `PATH`/`HOME`/`USER`.** Correct and worth
  preserving. It is what makes the declaration meaningful rather than advisory, and it is why the
  supervisor could not and did not work around the block.
- **The abort itself.** Refusing to start on a conflicting declaration is right; a driver that
  silently accepted a changed worker environment mid-journal would invalidate every prior dispatch's
  reproducibility.
- **The authorization token being a literal string in the environment.**
  `I_AUTHORIZE_READ_ONLY_BOUNDED_NETWORK_V1` (`released_wheel_proof.py:200`) is legible, greppable,
  and states its own scope. It made the boundary obvious enough for the supervisor to decline to set
  it unilaterally. No change.

## Suggested follow-up

- **A ruling on this run specifically.** Two packages are proven in the current journal and a granted
  authorization cannot reach the third. Whether to archive the journal and restart, or wait for the
  declaration door, is a human decision with a real cost either way; it should not be inferred from
  this report.
- **Audit other visions for the same latent block.** Any run whose packages will eventually need an
  environment variable, and which declared `{}` at first launch, is in the same position and will not
  discover it until the relevant package is dispatched. A cheap check is to grep each vision's
  criteria and harness scripts for `os.environ` / `env.get` reads and compare against its
  `worker-environment-declared` record.
