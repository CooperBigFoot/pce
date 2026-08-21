# PCE workflow feedback: a gate repair proven against its own criterion, unproven against its blast radius

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 6, GD1 gate repair `66e3e0e`, discovered by GD2 issuance 9
- Outcome: `blocked, resolved by plan version 7` — a new upstream package (GD10) now owns the repair

## Executive summary

An accepted gate repair changed a function signature and broke a sibling script that no
criterion in its package exercises. The package's proof is honest: every criterion it names
passes, the witness ref reproduces the finding, the repair ref fixes it. The proof is also
incomplete in a way the workflow cannot currently see.

**The defect class, stated for the ledger: a change proven against its own criterion,
unproven against everything else in its blast radius.**

This is distinct from every finding in the two earlier reports for this run. It is not a graph
defect — `pce graph check` is right, the edges are right, the criteria are sound. It is not a
worker defect — the worker that found it reported the most precise fault of the entire run. It
is a gap between what a repair changes and what the repairing package's criteria observe.

Note specifically: **the gate-reproof fix that landed today would not have caught this one.**
Re-proving a gate finding re-runs the criterion the finding names. The blast radius here lies
entirely outside GD1's criteria set, so no amount of re-proof against `--section
worker-network-reads` would ever touch `verify_released_wheel_evidence.py`. That is what makes
this a new entry rather than a duplicate.

## Evidence reviewed

- `planning/.../driver-journal.jsonl` — `finding-replayed` for GD1 `package-gate-1` finding 0;
  `package-parked` GD2 issuance 9
- `planning/.../package-outcomes/GD2/9.json`
- `pourpoint` at `ba1fe09` (GD2's composed base), `66e3e0e` (GD1's repair ref), `0f0bada`
- `scripts/released_wheel_proof.py:2081-2085`, `scripts/verify_released_wheel_evidence.py:209-212`
- `git log -S"completed_reads_before_worker" --all -- scripts/released_wheel_proof.py`
- `planning/.../graph.v6.json` — GD1's four criteria

## What worked

### The worker's fault report named the package, the kind, and the artefact

- Evidence: `package-outcomes/GD2/9.json` →
  `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"GD1 buildability: released-wheel verifier self-test regression"}}`
- Effect: this is the most specific fault in the run — every earlier one was a bare package id
  (`"GD1"`, `"GD2"`) or a capability phrase. It was reproducible from the fault text alone in one
  command, and the attribution to GD1 turned out to be exactly right. It is the shape that
  recommendation 2 of the first report for this run asks the schema to require.

### The gate's witness/repair ref pair remains the strongest evidence in the run

- Evidence: `finding-replayed` GD1 `package-gate-1` finding 0 — `witness_ref 5fce962…` exit 70,
  `repair_ref 66e3e0e…` exit 0.
- Effect: it is precisely because the repair ref is recorded that the regression could be
  attributed to a specific commit in one `git log -S` invocation. The mechanism that let the
  defect through is also the mechanism that made it diagnosable in minutes.

## Friction and failures

### 1. A gate repair's blast radius is never observed

- Severity: `high`
- Phase: `execution / gate repair`
- Observation: GD1's accepted repair `66e3e0e` ("fix: exclude preflight traffic from worker read
  gate") added a required parameter `completed_reads_before_worker` to `_build_evidence`
  (`scripts/released_wheel_proof.py:2081-2085`). The sibling script
  `scripts/verify_released_wheel_evidence.py:209-212` still calls it with ten positional
  arguments where eleven are required. At GD2's composed base:

      python3 scripts/verify_released_wheel_evidence.py --self-test
      -> ERROR[70]: TypeError: _build_evidence() missing 1 required positional argument:
                    'completed_reads_before_worker'

- Evidence: the two file:line ranges above at ref `ba1fe09`;
  `git log --oneline -S"completed_reads_before_worker" --all -- scripts/released_wheel_proof.py`
  → `66e3e0e`; the journal's `finding-replayed` record naming `66e3e0e` as GD1's `repair_ref`.
- Inference: GD1's four authored criteria and its one accepted amendment all invoke
  `released_wheel_proof.py self-test --section …`. None invokes
  `verify_released_wheel_evidence.py --self-test`. The repair was proven against the criterion it
  was written for and against nothing else.
- Impact: GD1 was marked `complete` and its proof carried across five plan advances while its
  deliverable carried a break that halts the very next package. Three dispatches of GD2
  (issuances 7, 8, 9) and one spent overrule preceded the discovery, and the run needed a seventh
  plan version to fix it.

### 2. Package completion says nothing about what the package broke

- Severity: `high`
- Phase: `package completion`
- Observation: `package-completed GD1` is emitted when GD1's own criteria pass. Nothing evaluates
  whether the tree GD1 hands downstream still satisfies anything else.
- Evidence: `driver-journal.jsonl` `package-completed` for GD1, followed by
  `package-repair-merged` merging `66e3e0e`; GD2's later park.
- Inference: completion is scoped to the package's criteria by design, which is correct as a
  proof boundary but leaves the composed tree unchecked.
- Impact: the defect surfaces only when a downstream package is dispatched — in this run, after
  five plan versions and roughly six hours.

### 3. The repair was authored by a worker whose brief scopes it to one package

- Severity: `medium`
- Phase: `gate repair`
- Observation: the worker producing a gate repair operates under the same scope boundary as any
  package worker — other packages are out of bounds. The verifier it broke is a sibling script in
  the same repository, exercised by GD2, GD3, and GD4 criteria.
- Evidence: brief scope-boundary wording, `\.pce/package-briefs/GD9/6.md:181-182` (the identical
  section appears in every brief).
- Inference: the boundary is about *packages*, not *files*. A repair may freely edit a shared
  script; nothing tells the worker which other criteria consume it.
- Impact: the worker had no way to know its change had a blast radius, and no criterion required
  it to look.

## Recommendations

### Re-run every previously-passed criterion in the run against a package's final tree before completing it

- Addresses: findings 1 and 2
- Change: before emitting `package-completed`, execute the criteria of every already-complete
  package against the completing package's tree. A regression fails the completing package, not
  the earlier one. This is a "no package completes by breaking a proof already earned" rule.
- Location: the completion path in `crates/core/src/package_driver.rs` that emits
  `package-completed`, reusing the existing criteria-execution machinery.
- Trade-off: real cost — every completion re-runs the accumulated criteria set, which grows with
  the run. For this vision the set is small (GD1's four self-test sections, seconds each). For a
  large graph it may need bounding to criteria whose commands touch files the package changed.
- Confidence: `medium` — the rule is right; the scoping needs thought before it becomes required.

### Have the gate repair declare and prove its blast radius

- Addresses: findings 1 and 3
- Change: when a repair changes a shared artefact, require the repair to name the other entry
  points into the artefact it changed and show them still green. Minimally: for a repair touching
  a script under `scripts/`, run every `--self-test` entry point in that directory.
- Location: gate repair brief and the `finding-replayed` acceptance path.
- Trade-off: heuristic and repository-shaped; a project without a `--self-test` convention gets
  nothing from it.
- Confidence: `experimental` — recorded as an experiment, not a required change. The general form
  ("prove the blast radius") is sound; the mechanism for detecting it is not obvious.

### Say in the brief which other packages' criteria consume the files this package may touch

- Addresses: finding 3
- Change: the brief already lists every other package's criteria. Add a derived line: for each
  file path appearing in this package's own criteria, note which other packages' criteria also
  reference it. The graph already contains everything needed.
- Location: `compose_package_worker_brief` (`crates/core/src/package_worker.rs`).
- Trade-off: brief grows slightly; the mapping is purely syntactic on criterion commands and will
  miss shared code not named in any command — as it would have here, since
  `verify_released_wheel_evidence.py` *is* named by GD2/GD3/GD4 criteria and so would have been
  caught.
- Confidence: `high` — cheap, derived from data already in the brief, and would have surfaced
  this exact case to GD1's repair worker.

## No-change decisions

- **The one-shot overrule refusing a second use for GD2.** GD2's overrule was spent in plan
  version 6 on an earlier, genuinely refutable park. It was unavailable for the issuance-9 park —
  correctly, and irrelevantly, since that complaint was true and overruling it would have
  laundered a real regression into the proof record. The limit did not cause harm.
- **`consumed_overrules.clear()` on plan advance** (`crates/core/src/package_driver.rs:1000-1002`).
  Correct: a new plan version is a new specification, so prior disputes about the old one should
  not carry. Noted only so the reset is not mistaken for a bug.
- **Scoping `package-completed` to the package's own criteria.** Correct as a proof boundary. The
  recommendation above adds a check alongside it rather than changing what completion means.

## Suggested follow-up

- **Audit the other repairs merged in this run.** `package-repair-merged` for GD1 is the only one
  so far, but the same class applies to any future repair. A cheap retrospective check is to run
  every `--self-test` entry point in `scripts/` at each `package-repair-merged` oid.
- **Cross-reference with the gate-reproof work.** The fix landed today re-proves a gate finding
  against its own criterion. It does not and cannot address this class, because the blast radius
  lies outside the criteria set being re-proved. Worth noting explicitly in that work's own record
  so the two are not conflated.
