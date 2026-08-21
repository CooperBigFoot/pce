# PCE workflow feedback: 2026-08-07-close-the-seven-basin-coverage-gap (plan version 11)

- Date: `2026-08-20`
- Orchestrator: Claude Code (Opus 5), `/work-graph` skill, session `5f9f76fa`
- Run: `planning/2026-08-07-close-the-seven-basin-coverage-gap` in `/Users/nicolaslazaro/Desktop/work/hfx`, `graph.v11.json`
- Outcome: blocked (SB12 complete; SB9 parked)

Continues `archive/2026-08-20/2026-08-20-close-the-seven-basin-coverage-gap.md`, filed earlier today
and since archived. Findings there are not repeated: the join-attribution defect (confirmed at
`src/main.rs:2714-2747` and briefed), criterion-prose/command divergence, external evidence
mutability, and the missing `issuance` on `package-join-conflicted`.

## Executive summary

One new high-severity finding. A plan version authored specifically to discharge SB9's park did
discharge the underlying blocker — SB12 completed first try and SB7's frozen criterion now passes at
both campaign roots — and SB9 nevertheless re-parked **with zero dispatches**, carrying a park record
whose stated reason describes three attributable failures that did not occur under this plan version
and whose evidence list is empty.

Also recorded: the previous report's high-severity join-attribution finding produced exactly the
predicted misdirection cost, and the fix for it is confirmed briefed. Two gate behaviours worked
notably well and are recorded as such.

## Evidence reviewed

- `planning/2026-08-07-close-the-seven-basin-coverage-gap/driver-journal.jsonl` (553803 bytes;
  events for issuance 29 and the SB9 re-park)
- `pce package driver-status --graph graph.v11.json --journal driver-journal.jsonl`
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/graph.v11.json` (sha256
  `5b7834104bba66db413cffe18c3ec003ce0b8fbdb4fd9f96159714e16bfb24f6`)
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/supervision.md`
- All `plan-version-advanced` records in the journal (advances 1->2 through 10->11)

## What worked

### The gate found the exposure that the package's own fix created

- Evidence: two paired proofs on SB12's gate `package-gate-29-1`, both accepted —
  `test_orientation_evidence_check_duplicate_basin.py` (witness `36b819a2` exit 1,
  `AssertionError: duplicate basin evidence was accepted`; repair `054765a1` exit 0,
  `refused=duplicate excluded basin record: 2020003440`) and
  `test_orientation_evidence_check_multiple_pairs.py` (witness exit 1, `verdict with multiple
  orientation pairs was accepted`; repair exit 0).
- Effect: SB12's act was to stop a check asserting pair identity against a hardcoded fixture and
  make it derive from the record instead. That removed an accidental guard — the fixture had
  constrained the roster as a side effect of pinning values — and opened two record-ambiguity holes.
  The gate found both and closed them deliberately, without reintroducing the fixture as a
  definition of correctness. This is a gate catching a regression that is invisible from the
  package's own criteria, which all passed at exit 0 before the gate ran.

### A criterion that runs the same frozen command against two roots produced non-vacuous proof

- Evidence: SB12's third criterion executed SB7's frozen command unchanged against
  `$HFX_CAMPAIGN_EVIDENCE` and `$HFX_CAMPAIGN_EVIDENCE_V1` in one execution. Recorded stdout shows
  all six basins settling to exactly one endpoint at each root, with all twelve pairs differing
  between the two runs, and the two out-of-scope basins reported as out of scope rather than
  counted as settled.
- Effect: the pass cannot be vacuous — neither root's numbers can satisfy the other's record. Worth
  noting as an authoring pattern: proving a reconciliation against two independent datasets in one
  criterion is cheap and forecloses the "it passes because it now accepts everything" failure that
  loosening a check invites.

## Friction and failures

### A package re-parked after a plan advance with zero dispatches, an empty attempts list, and a reason naming failures that never happened

- Severity: high
- Phase: recovery, plan advance
- Observation: after `plan-version-advanced` 10 -> 11, SB9 recomposed, hit the expected join
  conflict on `adapters/tdx-hydro/build_adapter.py`, and `recovery-parked` immediately. No worker was
  dispatched and no criterion executed under plan version 11. `driver-status` reports
  `dispatches_remaining: 0, retry_remaining: 0, local_patch_remaining: 0, next_rung: replan`.
- Evidence: the plan-version-11 park record in full is
  `{"event":"recovery-parked","package":"SB9","reason":"recovery spending exhausted after 3
  attributable failures; re-author as plan version n+1","attempts":[{"rung":"replan",
  "issuance":null,"what":"park for human re-authoring as plan version n+1","evidence":[]}]}`.
  The plan-version-10 park for the same package carried three `attempts` entries, each with a rung,
  an issuance, and captured criterion evidence.
- **Root cause, located in source after this report's first draft called it undiagnosable.** The
  count and the evidence in that record come from two functions with different plan-version
  scoping, sitting adjacent in the same file:
  - `crates/core/src/package_driver.rs:2093` — `charged_failure_count` filters `WorkerFailed`,
    `PackageFailed` and `PackageHardeningInvalidated` over **every event handed to it**, with no
    plan-version scoping.
  - `crates/core/src/package_driver.rs:2122` — `latest_criterion_failure_evidence`, defined
    immediately below, opens with `let events = events_for_active_plan(events);`.

  `park_if_recovery_exhausted` (`src/main.rs:2301-2324`) reads the **whole journal** via
  `read_driver_journal` and passes it to both. So `charged` counts SB9's three plan-version-10
  failures and reports "3 attributable failures", while `attempts` — built from the plan-scoped
  evidence — is correctly empty for plan version 11. The record contradicts itself because its two
  halves disagree about which plan version they are describing.

  Note the same function is called with a plan-scoped slice elsewhere
  (`package_driver.rs:1543` and `:1747` pass `&active_events[..=event_index]`), so the unscoped
  call is an inconsistency within the codebase, not a deliberate global policy.
- Inference, still labelled as such: this also explains why a plan advance does not clear an
  exhaustion park, since the charged count it is measured against never resets. It is consistent
  with the counter-case — SB3 was `recovery-parked` under plan version 1 and dispatched at issuance
  8 after the 1 -> 2 advance, composing cleanly, where SB9's recomposition hits a join conflict —
  but I traced the dispatch path only as far as the `Failed`-state park sweep at
  `src/main.rs:5442-5452` and am not asserting the full state machine.
- Impact: the plan version did its job — the blocker is gone, SB12 complete, SB7's criterion passing
  at both roots — and the package it was authored for still cannot run. Clearing it costs either the
  package's one-shot overrule or a further plan version, on a vision that has already spent eleven.

### The join-attribution defect cost exactly what the previous report predicted

- Severity: high (already filed; recorded here only as outcome evidence)
- Observation: SB9's issuances 26, 27 and 28 each failed with the driver-supplied reason
  `conflicted join broke parent criteria: SB7:Every recorded real-basin ambiguity is settled`, and
  the ladder exhausted without any worker addressing the actual defect.
- Evidence: the plan-version-10 park record's three `attempts` entries all carry the identical
  captured failure `FAIL: 2020003440 orientation pair changed: (147096, 148472) != (665258,
  666634)`, and the counter-test at SB7's own ref `db4fb9c3` reproduces it with no join present.
  The maintainer confirmed the mechanism at `src/main.rs:2714-2747`.
- Impact: three worker issuances spent on a non-existent merge defect. The actual repair, once a
  package was authorised to make it, was completed by one worker on its first attempt with no
  recovery rung spent.

## Recommendations

### Scope `charged_failure_count` to the active plan version at the park site

- Addresses: the root cause above — and, if the inference holds, the failure of a plan advance to
  clear an exhaustion park
- Change: in `park_if_recovery_exhausted` (`src/main.rs:2301`), pass `events_for_active_plan(&events)`
  to `charged_failure_count` as the adjacent `latest_criterion_failure_evidence` already does; or
  scope inside `charged_failure_count` itself and drop the now-redundant slicing at its
  plan-scoped call sites. The first is the smaller change; the second removes the inconsistency
  permanently.
- Location: `src/main.rs:2301-2324`, `crates/core/src/package_driver.rs:2093`
- Trade-off: failures genuinely accumulated across plan versions stop being charged, which is the
  intended semantics of "re-author as plan version n+1" but should be confirmed against the
  recovery model before changing.
- Confidence: high on the diagnosis, medium on which of the two shapes is wanted.

### Refuse an overrule with the exit that actually applies

- Addresses: a secondary observation from this run
- Change: `pce package driver-overrule` refused with `park overrule refused; graph revision is the
  only exit after a repeated dispute / package SB9 has no specification-dispute park to overrule`.
  The package's park was an exhaustion park, and the message names "a repeated dispute" that never
  occurred, sending the reader to look for a dispute history that does not exist. Name the park
  kind that was found and the exit that applies to it.
- Location: overrule admissibility check.
- Trade-off: none identified.
- Confidence: high

### Derive the park reason from the attempts it carries

- Addresses: "A package re-parked after a plan advance with zero dispatches..."
- Change: compose the `recovery-parked` reason from the `attempts` list rather than from a constant.
  With zero attempts carrying evidence, the reason should say what actually happened — that the
  package could not be dispatched — instead of asserting three attributable failures. A park record
  whose reason contradicts its own evidence list is worse than an unhelpful one, because it sends
  the reader looking for failures that never happened.
- Location: `recovery-parked` event construction.
- Trade-off: none identified; the data needed is already in the record.
- Confidence: high

### Record the ladder state at a plan advance

- Addresses: the same finding, on the undiagnosable half
- Change: include per-package recovery state in the `plan-version-advanced` record — which parks
  were discharged, which ladders reset, and which were carried. The advance currently records
  `carried_completions` and `carried_amendments` only, which makes a failure-to-reset invisible
  until a package refuses to dispatch, and undiagnosable afterwards.
- Location: `plan-version-advanced` event payload.
- Trade-off: a larger advance record, once per plan version.
- Confidence: high — this run could not distinguish "never reset" from "reset then consumed", and
  that distinction is the whole diagnosis.

## No-change decisions

- **Spending SB9's one-shot overrule to clear the park.** An overrule restores the ladder — observed
  directly at issuance 12 — so it would work. It is being escalated rather than taken: what is false
  here is the driver's park record rather than a worker's specification complaint, which is outside
  the sanctioned case; and if the cause is a failure to reset, an overrule buys one dispatch and
  spends the one-shot on the package most likely to need it again. Recorded as the orchestrator's
  reasoning, not as a recommendation against overrules generally.
- **The two gate-authored refusals in SB12.** They add repository content the maintainer did not
  review directly. That is the gate working as designed, and both are backed by paired proofs.

## Suggested follow-up

- The counter-case in this vision (SB3 reset at the 1 -> 2 advance; SB9 did not at 10 -> 11) is a
  ready-made pair for reproducing the reset behaviour against a real journal, if the ladder-state
  recording above is implemented and someone wants a regression test with known-good and
  known-bad instances.
