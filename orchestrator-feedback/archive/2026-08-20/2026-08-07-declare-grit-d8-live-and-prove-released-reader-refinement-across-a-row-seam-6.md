# PCE workflow feedback: `graph check --strict` cannot pass a graph with a shared harness

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph and /to-graph skills`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 13 draft
- Outcome: `advisory` — the draft is valid; `--strict` refuses it on 30 warnings, none of which is a defect in the draft

## Executive summary

`pce graph check --strict` landed today implementing two of the four authoring checks this run
produced. Run against the v13 draft it refuses with 30 warnings. All 30 were classified against the
graph and the repositories, and **none identifies a defect**:

- **11 `artifact-provenance`**, every one on an *already-frozen* package, every one reporting the
  same thing: *"a producer cannot be proven because the graph has no output declarations."* These are
  packages referencing artifacts they themselves produce. Two of the packages (GD11, GD13) are
  **complete**, so the artifacts demonstrably exist — just not at the authored ref.
- **19 `act-ownership-artifact`**, every one on the two new packages, every one reporting that they
  reference `scripts/released_wheel_proof.py` or `scripts/verify_released_wheel_evidence.py` — files
  that six predecessor packages also reference, because the harness is one file and every package
  that repairs it must name it.

The check is correct about the facts and wrong about the conclusion, in both cases for the same
underlying reason: **the graph has no vocabulary for "this package produces this artifact", so the
check cannot distinguish producing from merely referencing.**

## Evidence reviewed

- `pce graph check --file graph.json --strict --repository …` output, 30 warnings, exit 1
- the same check without `--strict`: `{"packages":14,"plan_version":13,"refs_verified":true,"valid":true}`
- `graph.v12.json` and the v13 draft
- the journal: GD11 and GD13 both `package-completed`

## What worked

### The check implements the right two questions

Both checks it encodes were learned in this run from frozen defects (report `-5`): provenance caught
nothing here only because the graph is now correct on that axis, and ownership is genuinely the
question that would have caught GD9's collision. Landing them as a tool rather than as supervisor
discipline is the right move.

### `--strict` as a separate flag

Warnings appear in the default output and only `--strict` refuses. That is the right default: this
graph is legitimately un-strict-able today, and a hard failure by default would have blocked a valid
draft.

## Friction and failures

### 1. Provenance cannot be satisfied because the schema has no output declarations

- Severity: `high`
- Phase: `graph authoring`
- Observation: 11 warnings, all reading *"artifact does not exist at the authored ref; a producer
  cannot be proven because the graph has no output declarations"*, with `strict_upstream_references: []`.
  The message names its own cause.
- Evidence: warnings on GD2 (`prepublication/fixed-cases.json`, `prepublication/horizontal-boundary`),
  GD3 (`prepublication`, `negative-flow-direction`), GD11 (`verify-candidate-staging.py` ×3), GD13
  (`verify-canonical-publication.py` ×4). GD11 and GD13 are `package-completed`; their artifacts exist.
- Inference: any package whose criteria verify its own deliverable warns, permanently, and no
  authoring change can clear it. A package that produces nothing new is the only kind that can pass.
- Impact: `--strict` is unreachable for this graph and for any graph of this shape. The signal is
  also unusable as a warning, because the true-positive case (an artifact produced *downstream*) is
  indistinguishable from the majority case (produced by the package itself).

### 2. Act-ownership fires on every package that repairs a shared file

- Severity: `medium`
- Phase: `graph authoring`
- Observation: 19 warnings, all on the two new packages, all of the form *"new package references the
  exact canonical artifact already referenced by a predecessor package"*, naming GD1, GD2, GD3, GD10,
  GD14, GD15 as predecessors.
- Evidence: `scripts/released_wheel_proof.py` is referenced by GD1 (4 criteria), GD10, GD14, GD15, and
  now GD16 and GD17; `scripts/verify_released_wheel_evidence.py` by GD2, GD3, GD10, GD16.
- Inference: the harness is a single file. Every repair package in this vision — GD10, GD14, GD15,
  and now GD16 and GD17 — necessarily names it. GD10, GD14 and GD15 all completed cleanly, so the
  pattern the check flags has three counter-examples already in this journal.
- Impact: 19 warnings of noise. The real ownership collision this check exists to catch — GD9
  duplicating the act named by GD2's frozen *title* — would be one row lost among them. A check whose
  true positives are outnumbered ten to one by structural false positives will be skimmed.

### 3. Warning volume scales with criteria, not with packages

- Severity: `low`
- Phase: `graph authoring`
- Observation: GD16 draws 11 ownership warnings for 3 criteria; GD17 draws 8 for 2. The same
  package/predecessor/path triple repeats — GD16↔GD1 on `released_wheel_proof.py` appears twice,
  GD16↔GD10 twice, GD16↔GD14 twice, GD16↔GD15 twice.
- Evidence: the census — `artifact-provenance: 11`, `act-ownership-artifact: 19`, from 5 new criteria.
- Impact: duplicates inflate the count and make the refusal message ("refused 30 authoring warnings")
  read as far worse than the 2 distinct findings it represents.

## Recommendations

### Add output declarations to the graph schema, and key provenance off them

- Addresses: findings 1 and 2
- Change: let a package declare the artifacts it produces (`"produces": ["docs/evidence/…", "scripts/…"]`).
  Provenance then warns only when an artifact is neither present at the authored ref nor declared by
  the package itself or a strictly upstream package. Ownership warns only when two packages declare
  they *produce* the same artifact — which is the actual collision — rather than when they reference it.
- Location: `work-package-graph.schema.json`, and the provenance/ownership checks in `pce graph check`.
- Trade-off: a new required-ish field, and authors must think about outputs. That thinking is the
  point of the check.
- Confidence: `high` — the tool's own message names the missing input.

### Deduplicate warnings before counting

- Addresses: finding 3
- Change: emit one warning per distinct (kind, package, predecessor, path) rather than per criterion.
- Location: the warning collector in `pce graph check`.
- Trade-off: none.
- Confidence: `high`

### Until outputs exist, treat ownership as title-based rather than artifact-based

- Addresses: finding 2
- Change: the ownership check that would have caught GD9 compared a new package's *act* against
  frozen package *titles* and criteria semantics, not shared file paths. An artifact-path proxy for
  that question produces mostly noise in a repo with a shared harness.
- Location: the act-ownership check.
- Trade-off: title comparison is fuzzy and cannot be fully mechanical; it may need to stay a human
  question in the presentation rather than a lint.
- Confidence: `medium` — flagged as the honest limit of what a lint can do here.

## No-change decisions

- **Warnings in the default output.** Correct. They are informative even when not actionable, and
  they made this classification possible in one command.
- **`--strict` refusing rather than warning.** Correct as a flag. The problem is what it counts, not
  that it refuses.
- **The four authoring checks themselves.** All four remain right; two are now partly mechanised and
  two (reference resolution, consumer addressability) remain human questions. Nothing here argues
  against any of them.

## Suggested follow-up

- **Re-run `--strict` on this vision once output declarations exist.** It should then drop from 30
  warnings to zero without any change to the graph, which would confirm the diagnosis.
- **A true-positive corpus.** GD9's collision and GD7/GD8's provenance defects are all still in this
  vision's frozen history. They would make a small regression suite proving the checks catch what
  they were built for.
