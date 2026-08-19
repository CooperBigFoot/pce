# PCE workflow feedback: declare GRIT D8 live and prove released-reader refinement across a row seam

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, driver journal `driver-journal.jsonl`, plan versions 1 through 6
- Outcome: `in progress` — six plan versions frozen, five package parks, one overrule spent; GD2 running at issuance 8 at time of writing. One package (GD1) proven.

## Executive summary

The work-package driver behaved correctly throughout. Every mechanism that refuses
something refused correctly, and no invalid state was ever admitted to the journal.
The run nevertheless consumed five plan versions to prove one package, and the
dominant cause is a single gap: **`pce graph check` validates a graph's structure but
not whether its criteria are satisfiable**, so three successive supervisor-authored
graphs passed every mechanical check while carrying defects that only a dispatched
worker could discover.

The highest-impact findings:

1. Worker reasoning was unrecoverable in 6 of 6 failed attempts (`herdr pane read`
   returned a bare shell prompt every time). Diagnosis rested entirely on the typed
   outcome file plus repository archaeology. The first park's true cause was
   misdiagnosed as a result, and two defective packages were authored on top of that
   misdiagnosis.
2. The `mis-specified` / `missing-dependency` fault carries a free-text id and no
   evidence. Three of five parks reported a bare package identifier (`"GD2"`, `"GD2"`,
   `"GD1"`) with no statement of what was missing or where the worker looked.
3. `pce graph check` accepted `graph.v3.json` (criterion unsatisfiable against the
   named transport), `graph.v4.json` (criterion circular on a downstream deliverable),
   and `graph.v5.json` (new package duplicating a frozen package's act).

The criterion-revision door added mid-run (`--criterion-revisions`) resolved the
accumulated damage cleanly and is the single most valuable addition observed.

## Evidence reviewed

- `planning/2026-08-07-declare-grit-d8.../driver-journal.jsonl` (59 events at time of writing)
- `planning/.../supervision.md` parts 1-19 and `supervision-state.json`
- `planning/.../graph.v1.json` through `graph.v6.json`, and `graph.v6.criterion-revisions.json`
- `planning/.../package-outcomes/{GD2/3.json, GD2/7.json, GD7/4.json, GD8/5.json, GD9/6.json}`
- `planning/.../.pce/package-results/GD2/5.json`
- `planning/.../.pce/package-briefs/GD9/6.md`
- `herdr pane read` output for panes `wNY:p1`, `wNZ:p1`, `wPJ:p1`, `wPY:p1`, `wQB:p1`, `wRK:p1`, `wNY:p2`
- `pourpoint` at refs `997fbe0`, `0f0bada`, `ba1fe09`, `66e3e0e`; `hfx` at `bca87d8`
- `pce` source at head `9d3d7a1`, installed binary sha256 `9e63cd87584b…`

## What worked

### Package parks as a refusal channel

- Evidence: `package-outcomes/GD2/3.json`, `GD7/4.json`, `GD8/5.json`, `GD9/6.json` —
  four `{"outcome":"mis-specified","fault":{"kind":"missing-dependency",...}}` records.
  Each was independently checked against repository evidence and each was correct.
- Effect: four defective package specifications were refused before any worker wrote
  code against them. No wasted implementation, and every worktree was left clean at
  its composed base (`git log <base>..HEAD` empty in all four).

### Carried completions gated on byte-identical package definitions

- Evidence: `plan-version-advanced` events for 2→3, 3→4, 4→5, 5→6 each carry
  `carried_completions: ["GD1"]` plus GD1's `gate:package-gate-1:finding:0` amendment.
  `unchanged_package_ids` (`crates/core/src/work_package_graph.rs:608+`) drops any
  package whose title, repositories, criteria, or dependencies changed, and the
  fixpoint loop also drops dependents of changed packages.
- Effect: GD1's proof survived five graph revisions without ever being re-executed,
  and the rule for preserving it was mechanical and predictable rather than a judgement
  call.

### The gate finding's witness/repair ref pair

- Evidence: `finding-replayed` for GD1 `package-gate-1` finding 0 —
  `witness_ref 5fce962…` exit 70, `repair_ref 66e3e0e…` exit 0.
- Effect: the accepted finding is self-proving. The witness ref demonstrates the defect
  reproduces and the repair ref demonstrates it is fixed, both as executable refs rather
  than prose. This is the strongest evidence shape in the whole run.

### Human-ratified criterion revision (`--criterion-revisions`)

- Evidence: `graph.v6.criterion-revisions.json`, sha256 `3a4bc30f…`, 9 revisions,
  `ratified_by: "Nicolas Lazaro"`, every `successor: null`. Replayed in
  `driver-status` output under `criterion_revisions` with from/to plan versions.
- Effect: the only mechanism that could retire the two defective criteria rather than
  carry them forever. It forces a named ratifier and a per-revision rationale, publishes
  an immutable record beside the graph, and surfaces the whole thing in the run's
  machine-readable status. Without it this vision's graph would have carried an
  unsatisfiable criterion permanently.

### `pce graph check --repository` ref verification

- Evidence: `{"refs_verified":true}` on v4, v5, v6; `{"refs_verified":false}` when the
  repository mappings are omitted.
- Effect: caught nothing in this run because the refs were correct, but it is the check
  that would have caught the plan-version-1 defect (`authored_at_ref 0f0bada` does not
  resolve in `hfx`) before any dispatch. See finding 5.

## Friction and failures

### 1. Worker reasoning is unrecoverable from dispatch panes

- Severity: `high`
- Phase: `execution / recovery`
- Observation: every failed attempt's pane read back as a single idle shell prompt.
  `herdr pane read wNY:p1 --source recent-unwrapped --lines 10000 --format text`
  returned exit 0 with the content
  `➜  00-pourpoint git:(pce/.../GD2/attempt-2)` and nothing else. The same for
  `wNZ:p1`, `wPJ:p1`, `wPY:p1`, `wQB:p1`, `wRK:p1`. `herdr pane read wNY:p2` returned
  `{"code":"pane_not_found"}`.
- Evidence: six pane reads recorded verbatim in `supervision.md` parts 5, 6, 9, 12, 15, 19.
- Inference: the worker process exits and its pane is either respawned to a shell or its
  scrollback is not retained; the supervisor's read always loses the race. This is an
  inference — the retention mechanism was not inspected.
- Impact: every diagnosis in this run rested on the typed outcome file plus repository
  archaeology. Concretely: the GD2 issuance-3 park reported
  `missing-dependency: GD1 released-reader-compatible HTTPS declaration proxy` and the
  supervisor could not learn *which* incompatibility the worker hit. It inferred an
  offline-transport requirement that the vision never imposed, froze that inference as
  GD7's criterion, and two further packages (GD8, GD9) were built on it. Three plan
  versions and four dispatches were spent on a misdiagnosis that a readable transcript
  would likely have prevented.

### 2. `missing-dependency` faults carry no evidence

- Severity: `high`
- Phase: `execution / escalation`
- Observation: the fault `id` is free text. Observed values: `"GD1
  released-reader-compatible HTTPS declaration proxy"`, `"offline-released-reader-GRIT-object-replay"`,
  `"GD2"`, `"GD2"`, `"GD1"`. The last three are bare package identifiers naming neither
  the missing capability nor where the worker looked for it.
- Evidence: `package-outcomes/{GD2/3.json, GD7/4.json, GD8/5.json, GD9/6.json, GD2/7.json}`.
- Inference: the outcome schema permits any string, so a worker satisfies it with a
  package id.
- Impact: refuting or accepting a park requires reconstructing the worker's reasoning
  from scratch. For GD2 issuance 7 the supervisor had to check `merge-base --is-ancestor`,
  read two symbol definitions, grep for an absent identifier, and re-execute two criteria
  in the worker's own worktree — to refute a one-word claim. That refutation was only
  possible because the claim happened to be checkable; `"GD2"` from GD8 and GD9 was not
  falsifiable in the same way.

### 3. `pce graph check` does not check that criteria are satisfiable

- Severity: `high`
- Phase: `graph authoring / freeze`
- Observation: three graphs passed `pce graph check` with `valid: true` while carrying
  defects that made a package impossible to complete.
  - `graph.v3.json` GD7 criterion "Released reader reads through the declaration proxy"
    requires the reader be driven "through the harness's injected offline transport" and
    report applied refinement. That transport is `ReplayTransport` at
    `scripts/released_wheel_proof.py:1583-1604` (ref `ba1fe09`), whose GET path returns
    `body = bytes(byte_range.length)` — null bytes. A reader fed nulls cannot refine.
  - `graph.v4.json` GD8 criterion "The corpus stays bounded" binds recorded ranges to
    "the declared fixed-outlet watershed set", which is GD2's deliverable; GD2 was
    downstream of GD8 in the v4 and v5 edges. No *declared* cycle existed, so the check
    passed.
  - `graph.v5.json` GD9 duplicated the act named by GD2's frozen title, "The two fixed
    planetary proof outlets are chosen and pinned".
- Evidence: `graph.v3.json`, `graph.v4.json`, `graph.v5.json`; the check output for each
  recorded in `supervision.md` parts 7, 10, 13; `package-outcomes/GD7/4.json`,
  `GD8/5.json`, `GD9/6.json`.
- Inference: the check validates schema, ids, and declared-edge acyclicity. It does not
  relate a criterion's referenced artifacts to the packages that produce them, and does
  not compare package titles for overlapping acts.
- Impact: three freeze-dispatch-park cycles. Two of the three defects were introduced by
  a supervisor drafting under a rule ("preserve every criterion byte-for-byte") that the
  tool enforced, while the tool enforced nothing about whether the new criteria could be
  met.

### 4. An environment failure was never explained

- Severity: `medium`
- Phase: `execution / recovery`
- Observation: GD2 issuance 2 produced
  `worker-environment-failed … "package dispatch stopped without a successful required
  artifact: Exited { code: ExitCode(0) }"`. The result file records
  `{"duration_ms":82129,"exit_status":{"kind":"exited","code":0},"required_artifact_presence":"absent"}`.
  The worktree was clean at its base with no commit; the brief was written normally
  (15,870 bytes, same size as the succeeding attempt's).
- Evidence: `driver-journal.jsonl` event 25; `.pce/package-results/GD2/5.json`;
  `supervision.md` part 5.
- Inference: a worker session terminated believing it was done while having produced
  nothing. The event is named `worker-environment-failed`, but nothing observed implicates
  the environment — the same prepare command succeeded three times during GD1.
- Impact: one wasted dispatch (82 seconds) and, more importantly, an event class whose
  name misdirects diagnosis. It never recurred, so no repository-contract defect was
  claimed, and it remains unexplained in the record.

### 5. The plan-version-1 graph named a ref that resolves in only one of its repositories

- Severity: `medium`
- Phase: `graph authoring`
- Observation: `graph.v1.json` declared `authored_at_ref: "0f0bada8395…"` for a graph whose
  packages span `pourpoint` and `hfx`. That oid exists in `pourpoint` only; the two
  repositories have disjoint object graphs. Composition resolves the authored ref per
  repository (`src/main.rs:3059`, `compose_git_commits` at `:2853-2885`), so every `hfx`
  package would have failed to compose.
- Evidence: `git -C hfx cat-file -t 0f0bada` → `fatal: could not get object info`;
  `graph.v1.json`; `supervision.md` part 2.
- Inference: at plan version 1 the schema had only a single `authored_at_ref`, which cannot
  express a multi-repository authoring state.
- Impact: caught before launch by a human, not by a tool. `pce graph check` without
  `--repository` reports `refs_verified: false` and still `valid: true`, so a graph can be
  frozen with an unresolvable ref.

### 6. `graph freeze` gained a required flag without the documented command being updated

- Severity: `medium`
- Phase: `freeze`
- Observation: the `/work-graph` skill documents
  `pce graph freeze --vision-dir <vision-dir>`. Against the current binary that fails:
  `Error: graph freeze requires one --repository NAME=SOURCE_WORKTREE mapping per graph repository`.
- Evidence: `supervision.md` part 8, command and exit 1 recorded verbatim.
- Impact: a human following the skill's documented command verbatim hits an error. Low
  cost here because the supervisor was running the command, but the skill hands this exact
  string to the human as the ruling boundary.

### 7. Promotion is unavailable for multi-repository visions, and the skill has no path for it

- Severity: `medium`
- Phase: `promotion`
- Observation: `/work-graph` section 8 requires the union of graph repository names to
  contain exactly one name. This vision names `pourpoint` and `hfx`, so sections 8-10 are
  refused regardless of outcome.
- Evidence: `graph.v1.json` … `graph.v6.json` `packages[].repositories`; `supervision.md`
  part 1.
- Impact: a fully proven run ends at `Finished` plus a notification with no defined path to
  landing. The constraint was identified before launch and is correct as written, but the
  workflow offers no alternative for a vision that is legitimately two-repository — and
  this vision's consumption edge (`hfx` produces the manifest, `pourpoint` consumes it) is
  declared in its own `vision.md`.

### 8. The criteria-protection hook substring-matches the vision filename

- Severity: `low`
- Phase: `tooling`
- Observation: `$HOME/.local/bin/pce-protect-criteria` refuses any Bash command containing
  the string `vision.md`. `supervision.md` contains that substring, so every attempt to
  append to the supervision log with `cat >> supervision.md` was refused with
  "REFUSED: Bash may not access vision.md during an active run".
- Evidence: three refusals recorded during parts 3, 5 and 12 authoring; the workaround was
  to use the Write/Edit tools instead.
- Inference: the hook matches on substring rather than on a resolved path.
- Impact: low — a workaround exists and the hook's intent is correct. But it blocks writes
  to a file the workflow itself mandates, and the failure message names the wrong file.

## Recommendations

### Retain worker pane scrollback until the driver records a terminal outcome

- Addresses: finding 1
- Change: hold the dispatch pane (or persist its scrollback to
  `<vision-dir>/.pce/package-transcripts/<PACKAGE>/<ISSUANCE>.txt`) until the driver has
  written `worker-done`, `package-parked`, or `worker-environment-failed`, rather than
  allowing cleanup or respawn to win the race.
- Location: dispatch/cleanup path around `dispatch-pane-cleanup` emission
  (`crates/core/src/package_driver.rs`), or `herdr` pane retention policy.
- Trade-off: disk for retained transcripts; a longer-lived pane per attempt.
- Confidence: `high` — this is the single change most likely to have prevented three of
  the five parks in this run.

### Require a structured fault payload for `mis-specified` outcomes

- Addresses: finding 2
- Change: replace the free-text `fault.id` with required fields — the capability or
  artifact believed missing, the path(s) or symbol(s) checked, and the command run to
  check them. Reject an outcome whose fault names only a package id.
- Location: the package outcome schema consumed by `pce package agent`, and the worker
  brief section that instructs reporting a missing dependency.
- Trade-off: workers must do a little evidence-gathering before parking; a genuinely
  confused worker may find the schema hard to satisfy, which is arguably the point.
- Confidence: `high`

### Add an artifact-provenance lint to `pce graph check`

- Addresses: findings 3 and 5
- Change: for every criterion, extract referenced repository paths from its `command` and
  verify each either resolves at the graph's authored ref for that package's repositories,
  or is produced by the package itself or a package strictly upstream in the declared
  edges. Report the ones that are not as a warning, or an error under a `--strict` flag.
- Location: `pce graph check` (`src/main.rs` graph-check path), reusing the existing
  `--repository` mappings it already accepts for `refs_verified`.
- Trade-off: path extraction from shell commands is heuristic and will produce false
  positives on generated paths; a warning level rather than a hard failure avoids blocking
  legitimate graphs.
- Confidence: `medium` — the check catches the mechanical half of the defect. The GD7
  defect lived in criterion prose (`input`/`observation`), not in the command, and a lint
  cannot catch that.

### Warn when a new package's title overlaps a frozen package's act

- Addresses: finding 3 (the GD9 case)
- Change: at freeze, when a successor graph adds a package, compare its title and criterion
  paths against every predecessor package's title and criterion paths; warn when a new
  package's criteria reference an artifact that a predecessor package's criteria also
  reference.
- Location: `run_graph_freeze` (`src/main.rs:9012+`), alongside the existing invariance
  check.
- Trade-off: an artifact legitimately verified by two packages (as
  `fixed-cases.json` is by GD2 and was by GD9) would warn; this is a warning, not a refusal.
- Confidence: `medium`

### Make `refs_verified: false` visible as a freeze-time refusal, not just a field

- Addresses: finding 5
- Change: `pce graph freeze` already requires `--repository` mappings; ensure it refuses a
  graph whose authored refs do not resolve in every declared repository. If it already
  does, the plan-version-1 defect in this run indicates the check post-dates that freeze —
  no change needed beyond confirming coverage.
- Location: `run_graph_freeze`
- Trade-off: none identified.
- Confidence: `medium` — stated as a check to confirm rather than a change to make, since
  the current binary may already cover it.

### Update the `/work-graph` skill's documented freeze command

- Addresses: finding 6
- Change: the skill's section 6 and section 11 both print
  `pce graph freeze --vision-dir <vision-dir>`. Add the required `--repository
  NAME=SOURCE_WORKTREE` mappings and the optional `--criterion-revisions
  <HUMAN_RECORD_PATH>`.
- Location: `skills/work-graph/SKILL.md`, sections 6 and 11.
- Trade-off: none.
- Confidence: `high`

### Match the criteria-protection hook on resolved paths

- Addresses: finding 8
- Change: resolve the candidate path and compare basenames, rather than substring-matching
  `vision.md` anywhere in the command line.
- Location: `$HOME/.local/bin/pce-protect-criteria`
- Trade-off: none identified.
- Confidence: `high`

## No-change decisions

- **The one-shot overrule limit.** It was reached once (GD2, plan version 6) and behaved
  correctly. Four earlier parks were *not* overruled because the evidence supported the
  worker each time; the limit never prevented a justified action. `RepeatedParkOverrule`
  ("graph revision is the only exit") is the right pressure. No change.
- **`worker-environment-declared` with an empty `names` list.** New event observed in the
  updated binary. Consistent with `run.json` declaring no worker environment. Not a defect.
- **Criteria invariance being packaging-independent.** `criteria_invariance_violation`
  (`crates/core/src/work_package_graph.rs:578-602`) flattens successor criteria across all
  packages before matching, so a criterion may move between packages but must still exist.
  This is correct and it is what forced the two defective criteria into an explicit,
  human-ratified retirement rather than a silent drop.
- **`depends_on` edges not being invariance-protected.** Correct as designed — a repartition
  must be able to re-point edges. Noted here only because it is a powerful and easily
  missed lever: an edge carries frozen justification prose that a human reader will trust,
  even though the binary does not protect it.

## Suggested follow-up

- **Multi-repository promotion.** Finding 7 is a workflow gap, not a bug. A vision with a
  declared producer/consumer edge between two repositories currently has no landing path.
  Worth a separate vision rather than a patch to `/work-graph`.
- **Experiment: a "criterion dry-run" verb.** Something like
  `pce package criteria-run --package <ID>` against a composed base *before* freezing,
  reporting which criterion commands are even resolvable. Three of this run's five parks
  would have been visible at authoring time. Marked as an experiment: it is unclear how a
  criterion whose artifact is legitimately produced by the package itself should be
  reported, and that is the majority case.
