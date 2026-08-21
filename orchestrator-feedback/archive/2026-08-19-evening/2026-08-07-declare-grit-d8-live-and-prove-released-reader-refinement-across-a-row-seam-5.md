# PCE workflow feedback: four authoring checks a graph author needs, each earned by a frozen defect

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph and /to-graph skills`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan versions 1 through 9
- Outcome: `blocked` — three of nine packages proven; the vision's central demand is under human ruling

## Executive summary

Nine plan versions produced three proven packages and eight package parks. Every park was a
worker correctly refusing. Four of them traced to a graph that passed `pce graph check` while
carrying a defect the check does not look for, and two of those defects were authored by this
supervisor.

Each defect yielded one check. The set is now four, and it is offered here as the substance of
this report because it is the only durable thing the wasted versions bought:

| # | check | the frozen defect that produced it |
|---|---|---|
| 1 | **Artifact provenance** — every artifact a criterion names, in its `command` *and in its `input`/`observation` prose*, must exist at the authored ref or be produced by a strictly upstream package | GD7's criterion required a transport that returns null bytes; GD8's bound its ranges to a downstream package's deliverable |
| 2 | **Act ownership** — before adding any package, confirm no frozen package's title or criteria already claim the act | GD9 duplicated the act GD2's frozen title claims; its worker was shown GD2 as out of bounds and correctly parked |
| 3 | **Reference resolution** — does the artifact resolve its own references from where it is about to be put? | a candidate-*prefix* design, viable on inspection, needed 255 GB of object copy because the manifest addresses its artifacts by relative key |
| 4 | **Consumer addressability** — can the consumer *name* the artifact at all, not merely resolve from it? | GD11 staged a content-addressed sibling correctly; the released reader derives `manifest.json` itself and can never name it |

Checks 3 and 4 look redundant and are not. The sibling **passed** 3 and **failed** 4.

## Evidence reviewed

- `planning/.../driver-journal.jsonl`, 132 events, plan versions 1-9
- `planning/.../graph.v1.json` … `graph.v9.json`, `graph.v6.criterion-revisions.json`
- `planning/.../package-outcomes/*` for eight parks
- `pourpoint` at `0f0bada`, `997fbe0`, `ba1fe09`, `66e3e0e`; `hfx` at `bca87d8`, `e5f51f7`
- the released wheel `pourpoint-0.3.0-cp39-abi3-macosx_11_0_arm64.whl`, installed to a scratch target
  and exercised directly

## What worked

### Parks as a refusal channel, eight for eight

- Evidence: eight `package-parked` records with typed `mis-specified` / `missing-dependency`
  outcomes. Every one was checked against repository evidence; every one was correct.
- Effect: not a single defective specification reached implementation. Workers refused, cleanly,
  with clean worktrees at their composed bases.

### The criterion-revision door, and then not needing it

- Evidence: `graph.v6.criterion-revisions.json`, 9 revisions, `ratified_by` the human. Later, at v9,
  an audit of all 19 v8 criteria found **none** mandated the retired mechanism, so the revision
  manifest was empty and two carried completions survived.
- Effect: the door made a stuck version chain movable in one direction, and the audit prevented an
  unnecessary use of it in the other. Both matter; the second is the one that preserved proof.

### Human-ratified worker-environment extension

- Evidence: `worker-environment-extended` at plan versions 8 and 9, additions-only, superset-checked.
- Effect: a granted authorization that had no path to the worker on 2026-08-19 morning reached it the
  same afternoon without abandoning a journal holding two proofs.

## Friction and failures

### 1. `pce graph check` validates structure, never satisfiability

- Severity: `high`
- Phase: `graph authoring`
- Observation: `graph.v3.json`, `graph.v4.json`, `graph.v5.json`, and `graph.v9.json` each returned
  `{"valid":true}` while carrying a defect that halted the next dispatch. The check verifies schema,
  criterion presence, edge kinds, acyclicity, and coverage. It does not relate a criterion's named
  artifacts to the packages that produce them, does not compare package titles for overlapping acts,
  does not ask whether an artifact resolves its references from its intended location, and does not
  ask whether a consumer can address it.
- Evidence: the four graphs and their check output, recorded in `supervision.md` parts 7, 10, 13, and
  the v9 authoring in part 34; the four parks that followed.
- Inference: the predicate is deliberately terminating and mechanical, which is right. The four checks
  above were run by hand each time after being learned the expensive way.
- Impact: four freeze-dispatch-park cycles across plan versions 3, 4, 5, and 9.

### 2. The descent obligation is the highest-yield step and the easiest to under-run

- Severity: `high`
- Phase: `graph authoring`
- Observation: `/to-graph` step 2 requires reading each repository at its ref for every claim a
  package depends on. Checks 3 and 4 are both instances of it. Both findings — relative artifact keys,
  and the fixed `manifest.json` name — took under five minutes to establish once asked, and each
  invalidated a design that had already been ruled on.
- Evidence: `hosting/grit-hfx-v0.3.0/manifest.json` at `bca87d8`, four relative artifact paths, zero
  absolute; `Engine(<dir>)` raising `DatasetError: required artifact "manifest.json" not found at
  <dir>/manifest.json` with zero network.
- Inference: the step is stated but its *questions* are not enumerated, so an author satisfies it by
  reading the code that a criterion names rather than by interrogating the artifact's placement and
  its consumers.
- Impact: one ruled design (candidate prefix) discarded before authoring, and one (sibling key)
  discarded after a package had already been built, frozen, dispatched, and completed.

### 3. A supervisor authoring criteria has no independent reviewer

- Severity: `medium`
- Phase: `graph authoring / freeze`
- Observation: the two defective criteria were drafted by the supervisor, passed the mechanical check,
  and were frozen by the human on the supervisor's presentation of that check. Neither was wrong in a
  way the check could see, and the human had no independent signal.
- Evidence: GD7 and GD8 criteria in `graph.v3.json` and `graph.v4.json`; the criterion-revision record
  that later retired all six.
- Inference: presenting "the check passes" is not evidence a criterion is satisfiable. The four checks
  narrow this but do not close it.
- Impact: two permanent entries in the version chain, retired only by an explicit human-ratified
  revision that did not exist in the binary when they were frozen.

## Recommendations

### Add the four checks to `/to-graph` step 6, as questions the author must answer in the presentation

- Addresses: findings 1, 2, 3
- Change: state them as four named questions and require the draft presentation to answer each with
  evidence, not assertion. Provenance and ownership are mechanical enough to lint; resolution and
  addressability need the author to read the consumer.
- Location: `skills/to-graph/SKILL.md` step 6, and the presentation contract in step 7.
- Trade-off: a longer presentation. Cheap against four freeze-dispatch-park cycles.
- Confidence: `high` — all four were learned from frozen defects in one run.

### Lint provenance and ownership in `pce graph check`

- Addresses: finding 1
- Change: extract repository paths from criterion commands and warn when one is neither resolvable at
  the authored ref nor produced by the package itself or a strictly upstream package; warn when a new
  package's criteria reference an artifact a predecessor package's criteria also reference.
- Location: the `pce graph check` path, reusing the `--repository` mappings it already takes.
- Trade-off: path extraction from shell commands is heuristic; warning level, not failure.
- Confidence: `medium` — catches the mechanical half. The GD7 defect lived in criterion *prose*, and a
  lint cannot reach it.

### Say in `/to-graph` step 2 what the descent must ask

- Addresses: finding 2
- Change: enumerate the questions rather than the obligation — where will this artifact live; what does
  it reference and does that resolve from there; who consumes it and can they name it; what does the
  consumer validate about location, scheme, or host.
- Location: `skills/to-graph/SKILL.md` step 2.
- Trade-off: none.
- Confidence: `high` — this run's last two blockers were both a missing answer to one of these.

## No-change decisions

- **Leaving the staged sibling in place.** Ruled by the human and correct: removal belongs to GD12,
  which owns proving the removal targeted only siblings. A hand removal would produce a mutation with
  no transcript and no criterion behind it, and leave GD12 nothing to verify.
- **The one-shot overrule.** Spent once in nine plan versions, on the single refutable complaint of
  eight. It never blocked a justified action.
- **The mechanical check's one-correction limit.** Correct. Every failure in this run came from what
  the predicate does not examine, never from needing more rounds of it.

## Suggested follow-up

- **A worked example of the four checks** in the skill, using this run: a criterion naming a
  null-returning transport, a package duplicating a frozen title, a manifest with relative keys, and a
  reader with a hard-coded artifact name. Each is small and each is real.
- **Whether `graph freeze` should require the author to attest the four checks.** Attestation is weak
  evidence and can become ceremony; recorded as a question rather than a recommendation.
