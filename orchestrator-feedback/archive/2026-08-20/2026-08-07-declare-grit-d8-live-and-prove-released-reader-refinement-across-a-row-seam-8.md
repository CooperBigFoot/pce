# PCE workflow feedback: a gate repair broke another package's proven property

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan versions 16-18
- Outcome: `blocked` — GD2 parked at issuance 35; resolved by authoring plan version 18

## Executive summary

GD16 proved a property of the whole harness: no self-test section reads the ambient
`POURPOINT_LIVE_READ_AUTHORIZATION` or `POURPOINT_RELEASE_WHEEL`. It completed. GD18 proved the
whole suite green with those variables unset. It completed.

Three packages later, **GD19's gate repair added a self-test section that reads both variables**, and
nothing noticed. Not the gate that authored it, not GD19's own criteria, not the carried completions
of GD16 and GD18, not `graph check`. The regression surfaced only when GD2 — six attempts and two
plan versions downstream — ran the suite as a precondition of its live work and parked.

The novel part is where the breakage came from. This is not a worker writing careless code: the
offending commit is `411667fa fix: authenticate unresolved worker status`, produced by **the gate
itself** as a hardening repair, and the gate's reproof re-ran only the criteria of the package it was
hardening. A mechanism whose whole purpose is to make a package stronger silently made a different,
already-complete package's guarantee false.

## Evidence reviewed

- `package-parked GD2 issuance 35`, reason naming
  `scripts/released_wheel_proof.py:_run_with_environment_probe` and
  `scripts/released_wheel_proof.py:self_test_unresolved_status_requires_protocol_marker`
- reproduced at GD2's composed base `ac27489a`:
  ```
  $ env -u POURPOINT_LIVE_READ_AUTHORIZATION -u POURPOINT_RELEASE_WHEEL \
      python3 scripts/released_wheel_proof.py self-test
  ERROR[10]: self-test read ambient live inputs:
    {"unresolved-status-authenticated": ["POURPOINT_LIVE_READ_AUTHORIZATION","POURPOINT_RELEASE_WHEEL"]}
  exit 10
  ```
  and identically with both variables set — the probe detects the *read*, not the value
- `git show 411667fa --stat` → `scripts/released_wheel_proof.py | 6 +++++-`, authored by the gate
- the added section at `:1160-1180` calls `run_worker(...)`, which resolves the sanitized environment
  before the section's `subprocess.run` stub can take effect
- `package-completed GD16`, `package-completed GD18` — both carried through plan versions 17 and 18

## What worked

### The probe GD16 built is exactly right

`_run_with_environment_probe` names the offending section and the variables it read, in one line, as
structured data. Diagnosis took a single command. A property that reports *which section broke it* is
worth far more than a boolean.

### GD2 caught it as a precondition

GD2's own criteria do not run the harness self-test. Its worker checked the suite anyway, because its
live run depends on the harness being sound, and parked with a `{missing, checked, command}` fault
naming both source sites. Boundary discipline found a regression that no criterion in the graph was
positioned to find.

## Friction and failures

### 1. Gate repairs are never checked against other packages' proven properties

- Severity: `high`
- Phase: `driver execution`
- Observation: the gate for GD19 authored a commit touching a file six packages share, then ran
  `gate-reproof-executed` for GD19's three criteria and its own amendment. No property proven by any
  other package was re-checked.
- Evidence: `package-repair-merged GD19 … repair_ref 411667fa`, followed by four
  `gate-reproof-executed` records, all scoped to GD19.
- Inference: the gate is a per-package hardening loop with a per-package proof obligation. When the
  file it hardens is shared, its blast radius is the whole graph and its proof is one package wide.
- Impact: a complete package's guarantee became false while the journal continued to report it
  complete. The run carried GD16 and GD18 as proven through two further plan versions in that state.

### 2. Carried completions are never re-proven until assembly

- Severity: `high`
- Phase: `driver execution`
- Observation: `plan-version-advanced` carries completions forward by comparing package *definitions*
  — title, repositories, criteria, depends_on. It does not re-run anything.
- Evidence: GD16 and GD18 carried unchanged across 16 → 17 → 18 while the property they proved was
  false at every tree built after `411667fa`.
- Inference: a carried completion asserts *this package was proven at its own tree*, which stays true
  forever. Readers — including this supervisor — take it to mean *this package's guarantee holds
  now*, which is a different claim and was false here.
- Impact: the gap between the two readings is the whole defect. Assembly would eventually catch it,
  after every package in the graph had been dispatched.

### 3. This is the third instance of one class in a single run

- Severity: `medium`
- Phase: `graph authoring` and `driver execution`
- Observation: GD15 broke a self-test path no criterion of its own ran (report 3). GD10 proved a
  repair no edge delivered (report 7). GD19's gate broke a property another package proved (this
  report). One class — *a proof scoped to what the actor names* — three mechanisms.
- Impact: each instance cost a park, a ruling, a plan version, and in two cases a live run of
  20-30 minutes. The class is the dominant cost of this run, well ahead of any actual defect in the
  work being proven.

## Recommendations

### Re-run the criteria of every package that shares a changed file, after a gate repair

- Addresses: findings 1 and 2
- Change: when `package-repair-merged` touches path `P`, re-run the criteria of every completed
  package whose criterion commands reference `P`, against the repaired tree. Emit the results as
  typed events; a failure demotes that package from complete rather than silently carrying it.
- Location: the gate-repair path in `package_driver.rs`, next to the existing `gate-reproof-executed`
  emission.
- Trade-off: more criterion executions per gate. In this graph the affected set is small and the
  commands are seconds-long self-tests; the alternative cost was a 28-minute live park two plan
  versions later.
- Confidence: `high` — the affected set is computable from the same criterion-command paths report 7
  already proposed extracting.

### Distinguish "proven at its tree" from "proven at head" in the journal

- Addresses: finding 2
- Change: let `carried_completions` carry the oid the proof was established at, and emit a warning
  when a later composed base contains changes to a file that package's criteria name. That is the
  read-only version of the recommendation above, and it is cheap.
- Location: the carry-forward path, and `driver-status` rendering.
- Confidence: `medium` — it reports rather than prevents, but it would have made the false state
  visible at plan version 17 instead of at GD2's fifteenth attempt.

### Give the gate the shared-file question explicitly

- Addresses: finding 1
- Change: the gate agent's brief should state which of the files it may touch are referenced by other
  packages' criteria, and require it to run those criteria before proposing a repair.
- Location: gate brief construction.
- Trade-off: a longer brief and a slower gate.
- Confidence: `medium` — the gate did good work here; it was simply never told the file was shared.

## No-change decisions

- **The gate authoring repairs at all.** It has found and fixed something in every package it has
  gated in this run, including this one. The problem is the scope of its proof obligation, not its
  existence.
- **GD16's probe reporting section names.** Keep exactly as is. It is the reason this took one command
  to diagnose instead of a bisect.
- **Criteria invariance.** Not implicated. The resolution is additive again — one new package, one new
  edge, zero criterion bytes changed.

## Suggested follow-up

- **~~Check whether other completed packages are currently false at head.~~ Done — answered below.**

## Appendix: the casualty audit, run

Every criterion of every completed package, executed against GD2's composed base `ac27489a` with the
driver's declared live inputs present, as the driver would run them:

```
GD1  authorization                     PASS      GD16 hermetic-under-live-inputs   PASS
GD1  bounds                            PASS      GD16 ambient-input-isolation      FAIL(10)
GD1  candidate-diagnostics             PASS      GD17 seed-rejection-continues     PASS
GD1  seed-probe-predicate              PASS      GD18 whole suite, vars unset      FAIL
GD10 verifier self-test                PASS      GD19 unresolved-candidate-recorded PASS
GD10 worker-network-reads              PASS      GD19 accepted-seed-is-ranked      PASS
GD14 transport-user-agent              PASS      GD20 unresolvable-seed-named      PASS
GD14 preflight-published-authority     PASS      GD20 seal resume                  PASS
GD15 live-hosted-source                PASS      GD20 distant-seed declaration     PASS
```

**Exactly two carried completions are false at head, and both for the single cause in this report:**
GD16's `ambient-input-isolation` and GD18's whole-suite command. Nine packages and their remaining
criteria are sound. That bounds the damage, and it confirms the repair scope — fixing the one section
`411667fa` added restores both.

It also makes the first recommendation concrete: this audit is twenty-one command invocations that
each take seconds, computable entirely from the criterion commands already in the graph. The driver
could run it after every `package-repair-merged` that touches a shared path, and would have reported
the regression at plan version 17 instead of leaving it for a fifteenth live attempt to discover.
- **Count the class.** Three instances, three mechanisms, one run. If a fourth appears, the shape is
  worth a design change rather than another countermeasure.
