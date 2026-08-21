# PCE workflow feedback: the recovery ladder loses the information that makes it work

- Date: `2026-08-21`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan versions 20 and 21
- Outcome: `blocked` — GD22 recovery-parked after spending its last rung as a degraded retry
- Severity: `high` — one defect makes a rung useless by construction, the other silently converts the
  only effective rung into the useless one

## Executive summary

Two independent defects, both in the recovery ladder, both about evidence rather than budget.

**The retry rung cannot repair a missing-artifact failure**, because it re-dispatches the same brief
that just failed. Observed three times in two days:

```
GD6   46 retry       -> 4 amendments exit 2      47 local-patch -> green
GD6   48 fresh       -> 4 amendments exit 2      49 retry -> identical    50 local-patch -> green
GD22  51 fresh       -> pin criterion exit 2     52 retry -> identical    53 local-patch -> green
```

In all three, `exit 2` is "the script this criterion names does not exist in this attempt's tree".
Only `local-patch` ever fixes it, because only `local-patch` appends the recorded failure evidence
naming the missing file.

**A plan-version advance clears that evidence while preserving the rung**, so the one rung that works
degrades into the one that does not:

```
recovery-rung-attempted  {package: GD22, issuance: 54, rung: "local-patch", evidence: []}
```

GD22 had exactly one dispatch remaining. It was spent on a local-patch rung with an empty evidence
list, failed for the same reason issuances 51 and 52 failed, and the package is now parked.

## Evidence reviewed

- the three cycles above, from `recovery-rung-attempted` and `criterion-executed` records
- `recovery-rung-attempted` at issuance 54: `rung: local-patch`, `evidence entries: 0`, immediately
  after `plan-version-advanced 20 -> 21`
- the same event at issuance 53, one plan version earlier, carrying a populated evidence array naming
  `python3 hosting/grit-hfx-v0.3.0/test-authority-pin-matches-tree.py`
- issuance 54's failures:
  ```
  Retained evidence without a completed worker raster read is refused   exit 0
  The authority pin equals the recorded file it pins                    exit 2
  gate:package-gate-53-2:finding:0                                      exit 2
  ```
  both exit-2 stderrs are `can't open file …`
- `recovery-parked GD22` — "recovery spending exhausted after 3 attributable failures in this journal
  recovery epoch; supply an attributed --recovery-reset record to open a fresh ladder"

## What worked

### The rung ladder's shape is right

- Evidence: in all three cycles the local-patch rung, when it carried evidence, repaired the failure
  on its first attempt.
- Effect: the mechanism works. Both defects below are about what reaches it, not about its design.

### The park message names the working door

- Evidence: it directs the human to `--recovery-reset`, which is the instrument that actually clears
  this state, unlike the plan-version remedy of report 11.

## Friction and failures

### 1. The retry rung is structurally incapable of repairing a missing artifact

- Severity: `high`
- Phase: `recovery`
- Observation: three cycles, three identical outcomes. The retry brief is described in the journal as
  *"same package and same brief with a fresh worker"*.
- Inference: for a failure whose cause is "the tree does not contain the file the criterion names", a
  fresh worker with an unchanged brief has no new information and reproduces the failure exactly.
- Impact: one wasted dispatch per cycle, and in a two-rung budget it is half the ladder. For GD22 it
  was the difference between repairing and parking.

### 2. A plan-version advance strips the evidence but keeps the rung

- Severity: `high`
- Phase: `recovery`
- Observation: `evidence: []` on a `local-patch` rung dispatched immediately after an advance; the same
  package's local-patch rung one version earlier carried a populated array.
- Inference: recovery *position* survives the advance while the recorded criterion failures do not, so
  the rung is dispatched without the thing that distinguishes it from a retry.
- Impact: silently converts the only effective rung into the ineffective one, at exactly the moment a
  package is most likely to be on its last dispatch — a plan version is usually minted *because*
  something went wrong. GD22 was parked by this.

### 3. A rejected attempt's gate amendment is inherited by its replacement

- Severity: `medium`
- Phase: `graph authoring`
- Observation: `plan-version-advanced 20 -> 21` carried `gate:package-gate-53-2:finding:0` for GD22.
  That gate ran on an attempt the human explicitly rejected; the criterion revision's own rationale
  says *"that work is rejected and is not carried"*.
- Evidence: the amendment's command is `python3 tests/verify_released_wheel_trace_read_order.py`, a
  script that existed only in the rejected attempt's tree.
- Inference: the documented rule is that gate-earned amendments carry as criteria while repair commits
  do not. Applied to a *rejected* attempt, that inherits an obligation from work the human refused,
  and requires the replacement to recreate a test it never designed.
- Impact: GD22's fresh attempt must satisfy a criterion inherited from the rejected implementation, on
  top of its two authored ones. It is not obviously wrong — the finding may be real regardless of who
  found it — but nothing records the decision either way.

## Recommendations

### Carry the recorded criterion failures across a plan-version advance

- Addresses: finding 2
- Change: preserve the evidence associated with a package's outstanding recovery position when the
  plan advances, or re-derive it from the last `package-failed` for that package in the journal. The
  data is still there; only the association is lost.
- Location: the carry-forward path in `package_driver.rs`, alongside `carried_amendments`.
- Trade-off: evidence from a prior plan version may reference criteria that changed. Filtering to
  criteria that still exist is straightforward and strictly better than dispatching an empty array.
- Confidence: `high`

### Skip the retry rung when the recorded failure is a missing artifact

- Addresses: finding 1
- Change: when every recorded failure is a non-existent-file execution failure, go straight to
  local-patch. `exit 2` with `can't open file` is a narrow, reliable signal.
- Location: the rung selection in `package_recovery.rs`.
- Trade-off: a heuristic on exit status and stderr. Conservative matching keeps the false-positive risk
  low, and the cost of being wrong is one dispatch — the same cost the current behaviour pays every
  time.
- Confidence: `medium`

### Decide explicitly whether a rejected attempt's amendments carry

- Addresses: finding 3
- Change: when a criterion revision's record states that the prior attempt's work is not carried, offer
  a way to say the same of its gate amendments — or state in the doctrine that amendments always carry
  regardless, so the inheritance is a choice rather than a side effect.
- Location: the criterion-revision record schema and the carry-forward path.
- Confidence: `medium` — the right answer is genuinely unclear; the absence of a recorded decision is
  the defect.

## No-change decisions

- **Charging the failures.** All three of GD22's charges were real criterion failures. Nothing here
  argues the accounting was wrong, only that two of the three dispatches were spent on a rung that
  could not repair the cause.
- **The recovery-epoch model.** Correct, and better than the plan-version scoping proposed in report
  11. Finding 2 is about evidence crossing the boundary, not about the boundary.

## Suggested follow-up

- **Count the cost in this run.** Of GD6's and GD22's nine dispatches across issuances 46-54, three
  were retry rungs that reproduced the prior failure byte-for-byte and one was a local-patch rung
  stripped of its evidence. Four of nine dispatches carried no new information into the worker.
