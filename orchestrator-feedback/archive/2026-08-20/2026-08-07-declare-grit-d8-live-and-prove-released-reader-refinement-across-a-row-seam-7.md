# PCE workflow feedback: a proven repair that no edge delivers is invisible to every check

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan versions 12-14
- Outcome: `blocked` — GD16 parked at issuance 25; resolved by authoring plan version 14

## Executive summary

Package GD10 completed, proving a repair to `scripts/verify_released_wheel_evidence.py`. Package
GD16's criteria run that exact file. GD16 did not declare an edge on GD10, so GD10's commit is not in
GD16's composed base, so GD16's third criterion fails with the very `TypeError` GD10 repaired — at a
plan version where GD10 is recorded `complete`.

Nothing detected this. `pce graph check` passed. `--strict` produced 30 warnings and none of them was
this one. The driver composed the base exactly as declared and dispatched. The worker parked, correctly,
after doing no work, and the driver exited `Blocked` with `ready: []`.

This is report 3's defect class — *a change proven against its own criterion, unproven against
everything in its blast radius* — but a distinct shape worth its own entry. Report 3's shape is a
package that **breaks** something outside its criteria. This shape is a package that **fixes**
something and the fix never arrives. Same class, opposite sign, and only one of the two is even
theoretically visible to a linter today.

## Evidence reviewed

- journal `package-base-composed`: `GD16 / pourpoint / bc540df0 / dependencies [("GD15","7b68223f")]`
- `git merge-base --is-ancestor 43c7a3d6 bc540df0` → **not an ancestor**
- `git merge-base --is-ancestor 43c7a3d6 7b68223f` → not an ancestor (GD15's tip lacks it too)
- blob comparison of the call site across refs:
  - `43c7a3d:scripts/verify_released_wheel_evidence.py:210` passes `completed_reads_before_worker`
  - `7b68223` and `bc540df` at `:209` do not
- executed at GD16's composed base with live inputs stripped, as its criterion strips them:
  `verify_released_wheel_evidence.py --self-test` → `ERROR[70]: TypeError: _build_evidence() missing
  1 required positional argument: 'completed_reads_before_worker'`
- `pce graph check --file graph.v13.json --strict` → 30 warnings, 11 `artifact-provenance` +
  19 `act-ownership-artifact`, **zero** concerning lineage delivery

## What worked

### The park was correct and cheap

The worker read its dependencies, ran its criterion command, found a red baseline it did not create,
and parked with a `{missing, checked, command}` fault naming both failing sites precisely. Its
worktree was untouched — `git status --short` and `git diff --stat` both empty. The new
missing-dependency fault schema made the complaint reproducible in one paste. That is the boundary
discipline working exactly as intended, and it is what made the diagnosis a ten-minute job.

### The journal recorded the composition faithfully

`package-base-composed` names the base oid and every dependency oid. The whole diagnosis is a single
`merge-base --is-ancestor` against that record. Without it this would have been archaeology.

## Friction and failures

### 1. `graph check` validates structure, never lineage delivery

- Severity: `high`
- Phase: `graph authoring`
- Observation: a package whose criteria execute file `F`, where a completed package's proven repair to
  `F` is absent from its declared closure, passes every check at every strictness level.
- Evidence: v13 passed `check`; `--strict` refused on 30 warnings, all classified, none this one.
- Inference: the checks that exist ask *does this artifact exist at the authored ref* and *does another
  package reference the same path*. Neither asks *will the repair this package needs actually be in its
  tree when it runs*. That question is answerable — the graph declares the edges, the journal records
  the proven oids, and the criterion commands name the files.
- Impact: the failure surfaces as a mid-run park after a dispatch, an issuance, a composed base and a
  worker's full context load. It is discoverable at authoring time for the cost of a set intersection.

### 2. The audit that finds it is supervisor discipline, not tooling

- Severity: `medium`
- Phase: `graph authoring`
- Observation: the human's ruling required auditing every other package for the same omission. I did it
  in ~20 lines of Python over the graph and the journal: transitive closure per package, changed-file
  set per completed package, intersect against the files each package's criteria execute.
- Evidence: 15 packages swept; 4 raw flags; 3 discarded as backward-in-time (a package cannot carry a
  successor's repair); 1 genuine, exactly the one already known.
- Inference: the sweep is mechanical and its one interesting subtlety — reverse-order flags are not
  defects — is a two-line filter on package order.
- Impact: it runs only when a human thinks to order it. It did not run at v12, v13, or any earlier
  version, and GD15 shipped without GD10's repair for the same reason GD16 did.

### 3. Nothing re-checks a completed package's repair against later trees

- Severity: `medium`
- Phase: `driver execution`
- Observation: GD10 is `complete` and carried through plan versions 11, 12, 13 and 14. Its repair is
  absent from GD15's tree and from GD16's tree. Both are or were dispatchable.
- Inference: carry-forward preserves the *completion*, not the *delivery*. A package can be
  simultaneously proven and, from the perspective of every tree that needs it, not present.
- Impact: the run's proof surface reads stronger than it is. Six packages complete at v13, one of whose
  proven repairs reached only one of the three later packages that run the file it repaired.

## Recommendations

### Add a lineage-delivery check to `pce graph check`

- Addresses: findings 1 and 2
- Change: for each package `P` and each package `Q` ordered before `P` that a journal records as
  complete, warn when `Q`'s proven commit range touches a file that `P`'s criterion commands name and
  `Q` is not in `P`'s transitive `depends_on`. Suppress when `Q` follows `P` in the graph's order.
- Location: `pce graph check`, taking an optional `--journal` so the proven oids are available.
- Trade-off: needs the journal, so it is a mid-run check rather than a v1-authoring check. That is
  where the defect lives anyway — it cannot exist before something has completed.
- Confidence: `high` — implemented ad hoc against this graph, one true positive and three
  order-filterable false positives out of 15 packages.

### Extract the file set from criterion commands, not from prose

- Addresses: finding 1
- Change: the paths are already literal in the `command` strings. A regex over `\S+\.(py|sh|rs|toml)`
  recovered them exactly for all 15 packages here.
- Location: same check.
- Trade-off: misses files reached indirectly (an import, a subprocess). Under-approximates, so it does
  not produce false refusals.
- Confidence: `high`

### Say what a carried completion does and does not guarantee

- Addresses: finding 3
- Change: `plan-version-advanced` reports `carried_completions`. It would be more honest as
  "this package's proof is retained" rather than anything implying its work is present downstream.
  Consider emitting, per carried package, the set of later packages whose declared closure omits it
  while their criteria name its files.
- Location: the carry-forward path in `package_driver.rs`, and the driver's status rendering.
- Trade-off: more journal volume at each plan-version boundary.
- Confidence: `medium` — the diagnosis is solid, the right surface for it less so.

## No-change decisions

- **Composing the base strictly from declared edges.** Correct, and it is what made this legible. A
  driver that helpfully merged in unrelated completed packages would have hidden the missing edge and
  produced a tree no one declared.
- **The worker parking rather than reaching for the fix.** Correct. It could have seen GD10 complete in
  the graph and cherry-picked. It did not, and the boundary held.
- **Criteria invariance refusing removal.** Not implicated here. The resolution was additive — one new
  package, two new edges, zero criterion bytes changed — which is the door working as designed.

## Suggested follow-up

- **Run the proposed check against this journal at v13.** It should emit exactly one warning,
  `GD16 -> GD10`, which would confirm both the check and its false-positive filter against a case whose
  answer is now known.
- **Keep the two shapes separate in whatever ledger tracks this.** *Broke a path no criterion of its
  own executes* (GD15) and *proved a repair no edge delivers* (GD10) share a cause but need different
  countermeasures: a whole-suite criterion for the first, a lineage check for the second.
