# PCE workflow feedback: `assembly-failed` recorded while every assembly criterion passed

- Date: `2026-08-21`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 21
- Outcome: `blocked` — all 19 packages complete, all 67 assembly criteria exit 0, terminal verdict `assembly-failed`
- Severity: `high` — the terminal verdict contradicts the evidence in the same journal, and it is the
  verdict that gates promotion

## Executive summary

The final assembly of plan version 21 executed **67 assembly criteria, every one exiting 0**. Every
criterion the graph and the carried amendments call for was executed — the counts match package by
package with no gaps. The driver then wrote:

```
{"event":"assembly-failed","reason":"one or more effective criteria failed against the composed assembly"}
```

`driver-status` reports `outcome: blocked`, `packages not complete: NONE`, `assembly.state: failed`.

No criterion failed. Nothing in the journal after the final composition records a failure of any kind.
A run that may be genuinely proven is held blocked by a verdict its own evidence contradicts.

## Evidence reviewed

- distinct exit statuses across the final assembly block:
  ```
  {"code": 0, "kind": "exited"} -> 67
  ```
  One distinct value. No signalled executions, no non-zero codes.
- every event in the block after the last two `assembly-repository-composed` records:
  ```
  assembly-repository-composed  2
  assembly-criterion-executed  67
  assembly-failed               1
  ```
  Nothing else. No `package-hardening-invalidated`, no `repair-credit-stale`, no failure of any kind.
- expected-versus-executed, computed from `graph.v21.json` criteria plus the `carried_amendments` of
  the last `plan-version-advanced`, per package:
  ```
  GD1 5/5  GD2 3/3  GD3 3/3  GD4 5/5  GD5 3/3  GD6 6/6  GD10 3/3  GD11 4/4
  GD12 3/3 GD13 5/5 GD14 2/2 GD15 3/3 GD16 4/4 GD17 3/3 GD18 3/3 GD19 3/3
  GD20 4/4 GD21 2/2 GD22 3/3          totals expect 67, ran 67
  ```
- `pce package driver-status --graph graph.v21.json`:
  ```
  outcome: blocked   ready: []
  packages not complete: NONE
  assembly: {"state":"failed","reason":"one or more effective criteria failed against the composed assembly"}
  ```
- the immediately preceding assembly, at plan version 20, **did** fail legitimately: 67 criteria,
  7 failures, all `accepted trace line numbers do not reference the localization pair`

## What worked

### The assembly itself

- Evidence: 67/67 green, including the four suites (GD10, GD16, GD18, GD21) that failed in the prior
  assembly, and GD4's two authority criteria that failed in the two before that.
- Effect: the repairs landed. Whatever the verdict is reading, it is not reading these.

### The journal records enough to detect the contradiction

- Evidence: the per-package expected-versus-executed reconciliation above is computable entirely from
  the frozen graph and the journal.
- Effect: the defect is provable rather than merely suspected, from durable artifacts.

## Friction and failures

### 1. The terminal verdict contradicts the journal it was written into

- Severity: `high`
- Phase: `assembly`
- Observation: `assembly-failed` with a reason naming failed criteria, in a block containing no failed
  criterion.
- Inference, marked as such: the most likely reading is that assembly failure state is **carried from a
  previous attempt** rather than recomputed for this one. The plan-version-20 assembly failed
  legitimately with 7 failures; the plan advanced to 21, GD22 completed, and the new assembly ran
  clean. If the failed flag is sticky across the advance — as the recovery evidence was found to be
  *non*-sticky in report 14, the mirror-image defect — the verdict would be exactly what was written.
  I have not read the source to confirm this and am not asserting it.
- Impact: `driver-status` returns `blocked`, so sections 8-10 of the work-graph role are unreachable and
  the run cannot be promoted or declared finished. A proven assembly is indistinguishable from a failed
  one to every consumer of the verdict.

### 2. The reason string carries no discriminating detail

- Severity: `medium`
- Phase: `assembly`
- Observation: *"one or more effective criteria failed against the composed assembly"* names no
  criterion, package, or command.
- Impact: with a true failure the supervisor must scan the block to find it, which is tolerable. With a
  false one there is nothing to scan, and the only way to establish that is the reconciliation above.
  Naming the criteria the verdict is based on would have made this self-evident and would make a real
  failure faster to act on.

## Recommendations

### Recompute assembly state per attempt rather than carrying it

- Addresses: finding 1
- Change: derive `assembly-failed` / `assembly-completed` from the criterion executions recorded after
  the current attempt's `assembly-repository-composed` events only, and reset any prior assembly state
  at `plan-version-advanced`.
- Location: the assembly-verdict path in `package_driver.rs`, and the `PlanVersionAdvanced` handler,
  which already sets `assembly = DriverAssemblyState::Pending` — worth checking why that does not
  clear this.
- Trade-off: none apparent.
- Confidence: `medium` — the recommendation follows from the observation; the mechanism is inferred.

### Name the failing criteria in the verdict

- Addresses: finding 2
- Change: include the failing `(package, criterion)` pairs in the `assembly-failed` reason or as a
  field.
- Location: the `assembly-failed` event construction.
- Trade-off: a longer event; bounded by the number of failures.
- Confidence: `high`

## No-change decisions

- **Blocking on a failed assembly.** Correct. The problem is the verdict being wrong, not the run
  stopping when it is right.
- **Re-running every criterion against the composed whole.** Correct and repeatedly valuable — it
  caught three real cross-package defects in this run that no package gate could see.

## Suggested follow-up

- **Confirm the sticky-state reading.** If `plan-version-advanced` already sets assembly state to
  `Pending`, the cause is elsewhere and this report's inference is wrong; the observation stands either
  way.
- **This run is a ready-made regression case.** Journal, frozen graph and the exact reconciliation are
  all on disk: 19 packages complete, 67/67 assembly criteria green, verdict `failed`.
