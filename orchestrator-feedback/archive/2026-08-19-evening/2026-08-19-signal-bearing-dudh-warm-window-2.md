# PCE workflow feedback: signal-bearing Dudh warm window (2)

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `bluesmith/planning/2026-07-29-signal-bearing-dudh-warm-window`, plan version 5, journal at 262 events
- Outcome: `blocked` — the driver process aborts with a non-zero exit and no journal event, twice, on the same condition

Filed as a second report because the blocking defect appeared after
`2026-08-19-signal-bearing-dudh-warm-window.md` was written and is unrelated to its findings.

## Executive summary

The driver **aborts with exit status 1** while crediting a gate repair that was carried as an amendment
across a plan version in which the owning package forfeited its completion. It reproduces exactly. The
package is left in `judging` forever, the journal records nothing about the abort, and the only
explanation exists in the terminal's stderr.

This is the highest-severity finding of the run: it is reached by an ordinary, legal sequence — revise
a package that carries a multi-repository gate amendment — and it terminates the orchestrator rather
than parking the package or recording a typed event.

## Evidence reviewed

- `planning/…/driver-journal.jsonl` (262 events; the abort leaves no record in it)
- tmux pane stderr of two driver processes, dead at `12:29:29` and `12:32:28` local
- `pce package driver-status --graph graph.v5.json --journal driver-journal.jsonl`
- `git cat-file -t` in both source repositories
- `graph.v4.json`, `graph.v5.json`, and the v4 revision record

## Friction and failures

### Driver aborts on a credited repair that is not a fast-forward of a re-composed package lineage

- Severity: `high`
- Phase: `execution / gate judging`
- Observation: after every gate on W7 finishes, the driver exits status 1 with

  ```
  Error: credited repair 7c56462d25f09a63f329fed0a27274b582caf388 for package-gate-20-1 finding 0
  is not a fast-forward of package W7 lineage b4e77741b511d1c4e0392bceff4a6369fa98eef1
  ```

  Reproduced twice, on two separate driver processes, after `package-gate-26-1` and again after
  `package-gate-26-2`. Both gates themselves succeeded: each filed one finding, decision `accepted`,
  witness exiting 1 and repair exiting 0.
- Evidence:
  - journal tail ends `gate-finished | W7 | package-gate-26-2` with **no** subsequent event;
  - `driver-status` reports `W7 {"issuance": 26, "state": "judging"}` and `outcome: running` while no
    driver process exists;
  - `git cat-file -t 7c56462d…` => `commit` in **stopwatch**, `could not get object info` in
    **bluesmith**.
- Inference (clearly separated from the observation): gate `package-gate-20-1` ran under plan version 4
  and merged repairs into **both** repositories (`package-repair-merged` for stopwatch and bluesmith).
  Plan version 5 added two criteria to W7, so W7 was revised, forfeited its completion, and was
  re-composed from the authored refs — giving it a new lineage `b4e77741…`. The **amendment** produced
  by that gate carried forward (amendments are criteria and carry), but the **repair commits did not**,
  because they belonged to the forfeited attempt's lineage. The driver then tries to credit the
  remembered repair against the new lineage and finds it is not a fast-forward. The orchestrator cannot
  prove this chain from the artifacts alone; it is the reading consistent with every observation above.
- Impact: **terminal.** W7 cannot complete, so W4, W5, W6 and the assembly cannot proceed. Recovery is
  untouched (`dispatches_remaining: 1, local_patch_remaining: 1`) because the failure is not attributed
  to the package at all. Two paid `r7g.2xlarge` instances were provisioned and torn down across the
  attempts, and the second sat idle while this was diagnosed.

### The abort is invisible to the run proof

- Severity: `high`
- Phase: `execution / state persistence`
- Observation: the append-only journal — the artifact the method designates as the admissible run proof
  — contains no record that the driver aborted. Its last event is a normal `gate-finished`. A reader of
  the journal alone would conclude the run is mid-judging and healthy.
- Evidence: `driver-status` derives `outcome: running` and `W7 state: judging` from the journal after
  both aborts, with no process alive.
- Inference: the abort path is an `Error:` return out of `main` rather than an appended typed event.
- Impact: a supervisor that trusted the journal, or a resumed session reconstructing state from disk as
  the work-graph skill requires, would see a live run that does not exist. The evidence needed to
  diagnose it lives only in a terminal scrollback buffer, which is not durable. This is also why the
  orchestrator briefly mis-reported the crash as non-reproducing: a `capture-pane` grep returned the
  **previous** process's stderr, and nothing in the journal was available to contradict it.

## Recommendations

### Append a typed event before any driver abort

- Addresses: "the abort is invisible to the run proof"
- Change: every non-zero exit from `driver-run` should first append a typed event carrying the same
  diagnostic string — e.g. `driver-aborted { reason }` — so the journal explains why the process stopped.
- Location: the `driver-run` command's error path in `src/main.rs`.
- Trade-off: adds an event kind that consumers must tolerate; `driver-status` must decide whether an
  aborted run reads as `blocked`.
- Confidence: `high`

### Do not credit a repair whose lineage the package no longer descends from

- Addresses: "driver aborts on a credited repair that is not a fast-forward"
- Change: when a carried amendment's credited repair is not reachable from the current package lineage,
  treat the amendment as **unsatisfied-and-recoverable** rather than fatal — the criterion still has to
  pass on its own merits, which is exactly what happened here (the worker recreated the test and
  `gate:package-gate-20-1:finding:0` passed at issuance 26). The remembered repair oid is stale
  bookkeeping, not proof, once the lineage is rebuilt.
- Location: the repair-crediting path that emits `credited repair … is not a fast-forward of package …
  lineage …`.
- Trade-off: a stale credit is dropped silently unless it is logged; pair with the typed-event
  recommendation above so the drop is visible.
- Confidence: `medium` — the correct disposition (drop, re-verify, or park) is a design decision, but
  aborting the whole driver is clearly not it.

### Decide what a forfeited package does with its gate amendments

- Addresses: the root of both findings
- Change: make the rule explicit in one place — when a package is revised and forfeits its completion,
  its gate amendments carry as criteria while their repairs do not. Either carry both, or carry neither,
  or document that the next worker must reconstruct the repair in **every** repository the amendment
  names.
- Location: the plan-advance/carry logic and its documentation; the `/to-graph` and `/work-graph` skill
  text that describes what carries.
- Trade-off: carrying repairs would mean composing a revised package on a forfeited attempt's commits,
  which may be wrong for other reasons; this recommendation asks for an explicit decision, not a
  specific one.
- Confidence: `high` that the ambiguity is real; `experimental` as to which resolution is right.

## No-change decisions

- **The gates themselves.** `package-gate-26-1` and `-26-2` both behaved correctly: each filed one
  accepted finding with a failing witness and a passing repair. Nothing about the gate mechanism
  contributed to the abort.
- **Recovery budgeting.** W7's budget was never charged for the abort, which is right — the failure is
  the driver's, not the package's.

## Suggested follow-up

- Worth checking whether the same condition can arise without a plan revision, e.g. when a package's
  lineage is rebuilt for any other reason while an amendment is outstanding. If so, the abort is
  reachable in ordinary single-plan-version runs too.
