# PCE workflow feedback: nothing carries within a package across attempts, so a multi-deliverable package must be won in one attempt

- Date: `2026-08-20`
- Orchestrator: `Claude Code / work-graph skill, session b2d7df0c`
- Run: `bluesmith planning/2026-07-29-signal-bearing-dudh-warm-window`, plan versions 12 to 13
- Outcome: `resolved by splitting the package; the recovery ladder behaved correctly throughout`

## Executive summary

W6 carried four independent deliverables. Three attempts each proved a different subset, none proved
all four, and the ladder exhausted. Every individual criterion was demonstrably achievable — two of
them twice. The package failed not because the work was hard but because attempts start fresh from the
composed base, so partial progress within a package is discarded, and a package of N independent
deliverables demands that one attempt land all N simultaneously.

Splitting W6 into three packages cleared it immediately: the first two completed on their first
attempt, in parallel.

## Evidence reviewed

- `driver-journal.jsonl`, all `criterion-executed` records for W6 issuances 105, 106 and 107
- `recovery-parked` record for W6
- `graph.v12.json` and `graph.v13.json`

## What worked

### The recovery ladder charged and stopped correctly

- Evidence: `recovery-parked W6 — recovery spending exhausted after 3 attributable failures; re-author
  as plan version n+1`. Three attempts, no more, and a park that names the remedy.
- Effect: the run stopped at the right moment instead of burning attempts on a packaging fault, and the
  park text pointed directly at re-authoring rather than at retrying.

## Friction and failures

### Attempts do not accumulate within a package

- Severity: `high`
- Phase: `execution`
- Observation: the per-attempt exit matrix:

  ```
  issuance 105   certify=0   resolve-window=0   bluesmith-tests=101   stopwatch-tests=0
  issuance 106   certify=0   resolve-window=0   bluesmith-tests=101   stopwatch-tests=0
  issuance 107   certify=1   resolve-window=2   bluesmith-tests=0     stopwatch-tests=0
  ```

- Evidence: attempt 107 repaired an inherited test-isolation defect, turning the workspace suites green
  for the first time, and simultaneously lost the two criteria that had passed twice. The
  `resolve-window` failure shows the loss was total rather than a regression:

  ```
  iteration_benchmark.py: error: argument command: invalid choice: 'resolve-window'
    (choose from 'calibrate','verify-window','verify-probe','screen-one','screen','report','gc','certify')
  ```

  The verb was absent, not broken. Attempt 107 began from the composed base and never wrote it.
- Inference: each attempt is independent, so a worker facing four deliverables and finite effort will
  rationally spend it on whichever it judges most urgent — and the previous attempt's work is gone.
- Impact: three attempts and roughly ninety minutes, plus a paid instance idling throughout, to reach a
  park whose cause was the shape of the package rather than the difficulty of the work.

### The park text names the remedy but not the diagnosis

- Severity: `low`
- Phase: `recovery`
- Observation: `re-author as plan version n+1` is correct advice that does not say what to change.
- Evidence: the diagnosis required the supervisor to assemble the per-attempt matrix above by hand from
  `criterion-executed` records; it is not surfaced anywhere.
- Inference: the driver has every fact needed to observe "different criteria passed on different
  attempts, and no attempt passed all", which is a strong and cheap signal that a package should split.
- Impact: the supervisor reached the right split, but only after manual reconstruction.

## Recommendations

### Report per-criterion outcomes across attempts when a package parks

- Addresses: finding 2
- Change: include, in the `recovery-parked` record, a matrix of criterion outcomes by attempt. When a
  criterion has passed in some attempt but the package never completed, say so explicitly — that is the
  signature of a package that should be split rather than retried.
- Location: recovery ladder park reporting
- Trade-off: a larger park record.
- Confidence: `high`

### State in the brief that attempts do not accumulate

- Addresses: finding 1
- Change: tell the worker plainly that its attempt starts from the composed base and that nothing from a
  previous attempt of the same package survives, so every criterion must be satisfied in this attempt.
  A worker that knows this may sequence its effort differently than one that assumes prior progress is
  banked.
- Location: package brief generation
- Trade-off: none.
- Confidence: `high`

### Consider warning at authoring time

- Addresses: finding 1
- Change: `pce graph check` could warn when a package carries several criteria with no shared
  deliverable — here, certification, a new CLI verb, and two test suites — since such packages are the
  ones that exhibit this failure.
- Location: graph check warnings
- Trade-off: heuristic, and will sometimes be wrong; a warning rather than a refusal.
- Confidence: `experimental`

## No-change decisions

The workers are not at fault and no change to worker conduct is proposed. Each attempt did coherent
work; the discarding of it between attempts is the mechanism at issue.

## Suggested follow-up

The split is a useful precedent worth confirming: v13 divided W6 into three packages with every
criterion carried byte-identically, needing no revision manifest because a split relocates criteria
rather than editing them. The first two completed on their first attempt, running in parallel because
they no longer shared a package.
