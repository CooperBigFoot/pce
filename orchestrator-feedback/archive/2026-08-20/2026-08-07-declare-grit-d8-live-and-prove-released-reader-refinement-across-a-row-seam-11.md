# PCE workflow feedback: the remedy `recovery-parked` prescribes does not work

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan versions 18 and 19
- Outcome: `blocked` — GD4 cannot be dispatched by any plan version; the run is stopped behind it
- Severity: `high` — the documented escape from an exhausted recovery ladder is inert, and five
  packages sit behind the one that needs it

## Executive summary

GD4 exhausted its recovery ladder on three identical failures caused by a harness defect (report 9),
and the driver parked it with an explicit remedy:

```
recovery-parked  "recovery spending exhausted after 3 attributable failures;
                  re-author as plan version n+1"
```

That remedy was performed. Plan version 19 was frozen and the driver relaunched. **GD4 was recomposed
and immediately re-parked with the same message, without a single dispatch.**

The cause is a disagreement inside the driver about which events count. Replay selects a recovery rung
from `active_events` — the window that begins at the last `PlanVersionAdvanced`. The snapshot that
reports and gates dispatch counts over the **entire** journal. For GD4 the two disagree completely:

```
GD4 charged failures — full journal: 3  |  active window since the v19 advance: 0
```

Zero charged failures in the active window, and yet `dispatches_remaining: 0, next_rung: replan`.
A package that exhausts its ladder can therefore never be recovered, because the only prescribed
remedy is filtered out by the counter that enforces the prescription.

## Evidence reviewed

- the journal, whose last four events are the whole experiment:
  ```
  plan-version-advanced   18 -> 19   carried 14 completions
  package-base-composed   GD4 / hfx       5603645f…
  package-base-composed   GD4 / pourpoint cb3b8595…
  recovery-parked         GD4  "recovery spending exhausted after 3 attributable failures;
                                re-author as plan version n+1"
  ```
  GD4 reached `package-base-composed`, so it was `Pending` after the advance — the advance did move it
  out of its parked state. It was re-parked before any worker was dispatched.
- `pce package driver-status --graph …/graph.v19.json`:
  ```
  outcome blocked   ready []
  GD4  {dispatches_remaining: 0, local_patch_remaining: 0, next_rung: replan, retry_remaining: 0}
  GD5  {dispatches_remaining: 2, local_patch_remaining: 1, next_rung: retry,  retry_remaining: 1}
  ```
- counted directly from the journal: `GD4 charged failures — full journal: 3 | active window: 0`,
  where charged events are `worker-failed`, `package-failed`, `package-hardening-invalidated` per
  `charged_failure_count`.
- `crates/core/src/package_driver.rs:1067-1069` — `active_events` begins at the last
  `PlanVersionAdvanced`
- `crates/core/src/package_driver.rs:1543` and `:1747` — rung selection uses
  `charged_failure_count(&active_events[..=event_index], package)`
- `crates/core/src/package_driver.rs:1809` — the snapshot's recovery map uses
  `charged_failure_count(events, package.id().as_str())`, over the full journal
- `crates/core/src/package_recovery.rs:155-177` — `recovery_budget` maps a charged count to a rung;
  `charged > dispatch_budget` yields `Replan`

## What worked

### The plan-version advance did what it should on every other axis

- Evidence: all fourteen completions carried; GD4 recomposed onto the advanced hfx ref
  `5603645f91f80873e3d1cb9c236feb303def949e`; `pce graph check --file graph.v19.json` valid.
- Effect: the mechanical freeze itself is sound. The defect is isolated to the recovery counter, which
  makes it cheap to fix without touching anything else.

### The park message names its own remedy

- Evidence: `"re-author as plan version n+1"`.
- Effect: it told me exactly what to try, which is how the defect became observable rather than
  mysterious. A silent park would have left me guessing.

## Friction and failures

### 1. Two counters disagree, and the enforcing one ignores plan versions

- Severity: `high`
- Phase: `driver execution`
- Observation: replay counts charged failures within the post-advance window; the snapshot counts them
  across the whole journal. GD4 is 0 by the first and 3 by the second.
- Evidence: the two call sites above, and the empirical count from this journal.
- Inference, distinguished from the observation: I did not instrument which call site emitted this
  particular `recovery-parked`. What is established by observation is that the *effective* budget
  ignored the advance — `dispatches_remaining: 0` with zero charged failures in the active window.
  The full-journal call site at `:1809` is the only one that produces that number.
- Impact: an exhausted package is permanently undispatchable. No freeze, no criterion revision, no
  definition change reaches it, because none of them clears a full-journal count.

### 2. The park's prescription is unreachable, so an orchestrator burns a plan version discovering it

- Severity: `medium`
- Phase: `recovery`
- Observation: I froze plan version 19 specifically to satisfy the message, on a human ruling that
  scoped it to a genuine ref advance so that the version would be honest rather than invented.
- Impact: a frozen, immutable plan version now exists whose only purpose was to attempt a remedy that
  cannot work. It is not wasted — the hfx ref advance was real and correct — but the run paid a freeze
  and a relaunch for information the driver could have refused to give.

### 3. `package-park-overruled` is not an alternative door

- Severity: `low`
- Phase: `recovery`
- Observation: the obvious human escape is an overrule, but it requires the package to be in
  `disputed_parks`, which only `PackageParked` populates (`:1569`). A `recovery-parked` package is not
  disputed, and `:1162` clears `disputed_parks` at every advance regardless.
- Evidence: `:1345-1370`, which returns `NoDisputedPark` otherwise.
- Impact: there is no human ruling that reaches this state either. I checked before proposing one to
  the human, which is the only reason a doomed overrule was not attempted as well.

## Recommendations

### Count charged failures over the active window in the snapshot too

- Addresses: findings 1 and 2
- Change: at `:1809`, pass the post-advance window rather than `events`, matching `:1543` and `:1747`.
- Location: `crates/core/src/package_driver.rs:1805-1815`.
- Trade-off: a package that genuinely deserves its exhaustion gets a fresh ladder at the next advance.
  That is the intended semantics — the message says so — and a plan version is a human-gated act, not
  something a package can grant itself.
- Confidence: `high` — the two call sites differ by exactly this argument, and the empirical numbers
  match the full-journal count precisely.

### Add a regression test for the prescription

- Addresses: finding 1
- Change: a test that exhausts a package, advances the plan version, and asserts the package is
  dispatchable again. `charged_failure_count` already has focused tests at `:2392` and `:2410`; none
  covers the count as the snapshot uses it.
- Location: the `package_driver` test module.
- Confidence: `high`

### Refuse to prescribe a remedy the driver will not honour

- Addresses: finding 2
- Change: if the remedy stays unreachable for some states, the park message should say what actually
  clears it rather than naming a plan version.
- Location: the `recovery-parked` reason string.
- Trade-off: none. This is a fallback if the first recommendation is rejected.
- Confidence: `medium`

## No-change decisions

- **The ladder itself.** Three attributable failures before parking is a reasonable budget. It behaved
  correctly here — the three failures were real, and report 9 explains why they were unavoidable.
- **Resetting on plan-version advance rather than on definition change.** The window semantics at
  `:1067` are the right rule: a human-gated re-authoring is exactly the moment a package deserves a
  fresh ladder. It simply needs to be applied where it is enforced.
- **`recovery-parked` as a distinct event from `package-parked`.** Correct; they mean different
  things. The consequence for overrules (finding 3) is worth revisiting only if the first
  recommendation is not adopted.

## Suggested follow-up

- **Verify with the run that is already waiting.** GD4 is stopped, frozen at plan version 19, with a
  fixed `PCE_WORKTREE_N` binary installed and three frozen criteria that should now pass untouched.
  Applying the first recommendation lets a single relaunch confirm this report and report 9 together,
  at no additional setup cost.
- **Check whether any earlier run has a package stuck this way.** The defect is old enough to predate
  this vision; a package quietly undispatchable would look like an ordinary blocked graph.
