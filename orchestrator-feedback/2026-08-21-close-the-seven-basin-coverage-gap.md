# PCE workflow feedback: 2026-08-07-close-the-seven-basin-coverage-gap (SB3 spawn)

- Date: `2026-08-21`
- Orchestrator: Claude Code (Opus 5), `/work-graph` skill, session `5f9f76fa`
- Run: `planning/2026-08-07-close-the-seven-basin-coverage-gap` in `/Users/nicolaslazaro/Desktop/work/hfx`, `graph.v11.json`
- Binary: `f83d45486737b1425a011e8fb03914b3f096d0291c53985ef2ba82af7a9cdf0e`
- Outcome: blocked — SB3 cannot spawn a worker

## Executive summary

One high-severity blocking defect, reported immediately because it stops the run and no local
repair is admissible. **The driver refuses to spawn a worker unless a previously recorded join
conflict still reproduces.** SB3's recorded conflict cannot reproduce, because the package that
owned reconciling the conflicted file completed in the interim and its merge is now an ancestor.
The driver treats "the conflict I remembered is gone" as a spawn failure rather than as a clean
composition.

Two consecutive dispatches (issuances 33 and 34) failed identically, so this is deterministic and
not a transient environment fault.

Also recorded: the epoch-scoped recovery reset shipped in `273f5c34` works exactly as specified —
SB3's reset opened a fresh ladder and the package dispatched.

## Evidence reviewed

- `planning/2026-08-07-close-the-seven-basin-coverage-gap/driver-journal.jsonl`
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/sb3-recovery-reset.json`
- `pce package driver-status --graph graph.v11.json --journal driver-journal.jsonl`
- All `package-join-conflicted` records for SB3 in the journal

## What worked

### The epoch-scoped recovery reset

- Evidence: `recovery-spending-reset` SB3 (`reset_by: "Nicolas Lazaro"`, `record_sha256:
  ddb4141e187da3815e4926b248c38f0f728e661efa529afef33286e20f32b2c1`) followed immediately by
  `worker-dispatched` SB3 issuance 33. The prior park message —
  `recovery spending exhausted after 9 attributable failures in this journal recovery epoch;
  supply an attributed --recovery-reset record to open a fresh ladder` — named the count, the
  scope, and the exit.
- Effect: this is the direct fix for the defect filed yesterday, where a park claimed three
  failures while carrying an empty evidence list. The new message is accurate and actionable, and
  the reset mechanism discharged a nine-failure ladder that no plan version could have cleared.
  Recording the spend authorization inside the reset rationale also puts the money decision in the
  journal rather than in chat, which is a better durable record than anything the orchestrator was
  doing before.

## Friction and failures

### A worker cannot spawn when a recorded join conflict has since been legitimately resolved

- Severity: high (blocking)
- Phase: execution, dispatch
- Observation: SB3 dispatches and then immediately fails to spawn, twice:
  `worker-spawn-failed` SB3 issuance 33 and issuance 34, `scope: "dispatch-environment"`, reason
  verbatim:
  `worker spawn failed: conflicted join for repository hfx did not reproduce: expected
  ["adapters/tdx-hydro/build_adapter.py"], observed []; stdout: Already up to date.; stderr:`
- Evidence: SB3's only two `package-join-conflicted` records are at bases `c796b39b2a77` and
  `622386437406`, both recorded before SB9 completed. SB9's whole act was reconciling
  `adapters/tdx-hydro/build_adapter.py` across SB1, SB7, SB8, SB11 and SB12; it completed at
  hardened ref `26cb4978abad453b61c3e61d7975280cca920a3f`. With that reconciliation in the tree,
  SB7 is an ancestor and the merge reports `Already up to date.` — there is no longer anything to
  conflict.
- Inference, separated: the spawn path appears to verify a remembered conflict as a precondition,
  and to treat non-reproduction as a fault. The observed state is strictly better than the
  remembered one: a clean merge is the successful case, not a failure.
- Impact: the run is stopped at its last substantive package with no admissible local repair. The
  failure is environment-scoped rather than charged to the package ladder, so it will not park —
  it will simply never spawn. Manufacturing a conflict to satisfy the check is not an option any
  honest supervisor would take, and no flag disables the precondition.

## Recommendations

### Treat a vanished conflict as a clean compose, not a spawn failure

- Addresses: the finding above
- Change: when the recorded conflicted paths do not reproduce and the merge is clean, recompose and
  dispatch normally, journaling that the previously recorded conflict no longer occurs and naming
  the completion that resolved it. Reserve the spawn failure for the case where the observed
  conflict set is *different and non-empty*, which is the genuine "the world moved under us"
  signal. A superset/subset comparison would also distinguish "partially resolved" from "resolved".
- Location: worker spawn precondition that compares expected against observed conflicted paths.
- Trade-off: the driver loses a consistency assertion it currently makes; the replacement assertion
  is weaker but is the one that matches reality after an upstream package merges the contested file.
- Confidence: high — the state is deterministic, reproduced twice, and the resolving completion is
  identifiable in the journal.

## No-change decisions

- **Retrying further.** Two identical dispatches establish determinism; a third would spend
  environment-failure budget to re-learn the same fact.
- **Any local repair.** Nothing in the working tree is wrong. The conflict's absence is correct and
  is the product of SB9 doing its job.

## Suggested follow-up

- This is the third distinct way this vision has been blocked by state recorded at one moment being
  asserted at a later one — after the criterion fixture pinned to a campaign's numbers and the park
  reason counting failures from a previous plan version. Whatever else changes, a general sweep for
  "recorded expectation asserted against a changed world" in the driver would likely find more.
