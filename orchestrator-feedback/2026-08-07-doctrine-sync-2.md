# PCE workflow feedback: doctrine sync (second half)

- Date: `2026-08-07`
- Orchestrator: Claude Code, Opus 5, ultracode session
- Run: `pyplate/planning/2026-08-04-doctrine-sync` (cross-repo: `pyplate` primary, `rustplate`)
- Outcome: `in progress` — milestones 1–4 delivered (13 PRs merged), milestone 5 at its first step

Continues `2026-08-05-doctrine-sync.md`, which covered Phase 0–2. That report's six findings are
not repeated here; only new ones are recorded.

## Executive summary

Milestones 1–4 shipped across two repositories and two GitHub owners. The adversarial gates were
the most valuable mechanism in the run and repeatedly caught defects that could only be found by
*executing* something — running the repository's formatter against a plan's pinned bytes, diffing
extracted regions, checking a return type against source, or constructing a test against the
pre-fix code to confirm it fails.

Four findings stand out:

1. **A `step-planner` revision emitted a changelog instead of the artifact — twice.** Both outputs
   were schema-valid. Both destroyed an approved specification. Recovery worked only because a gate
   artifact happened to quote what was destroyed.
2. **The verdict schema became invalid for the OpenAI structured-output dialect mid-run**, which
   killed every Codex structured dispatch while Claude gates continued to pass — because a gate with
   an empty `blocking_issues` array never exercises the item schema.
3. **`pce dispatch`'s `-o` convention writes into the worktree it is measuring**, which caused a
   legitimate `PLAN_INFEASIBLE` when a plan asserted on `git status --short`.
4. **Cross-repo runs break the `step-plan-writer` `--cwd` assumption**: a plan for a non-primary
   repository must be written under `VISION_DIR`, which lives in the primary repository.

## Evidence reviewed

- `pyplate/planning/2026-08-04-doctrine-sync/events.jsonl`
- Milestone 3, 4, 5 graphs, plans, and review artifacts
- Terminal output of `pce dispatch`, `pce status`, `pce ready`, `pce contract check/refresh`
- `~/.claude/skills/pce/schemas/verdict.schema.json` at three digests during the session

## What worked

### Gates that execute rather than read

- Evidence: the `m4-s2` plan critic reconstructed the plan's pinned `tests/test_init.py` in a scratch
  tree and found `PLACEHOLDER = "SHORT PROJECT" " DESCRIPTION"` fails `ruff format --check` **and**
  that the formatter joins the implicit concatenation, after which the test file contains the
  contiguous placeholder and its own central assertion fails. Verified as-pinned passing and
  post-format failing.
- Evidence: the `m5-s1` plan critic proved a defect with `grep -c '```python' plan.md` → `0`: the two
  files carrying the entire safety argument were prose, never pinned bytes.
- Evidence: the same critic checked `sync_doctrine()`'s return against source and found a pinned
  assertion comparing `SyncResult` to `SyncOutcome`.
- Effect: every one of these would have failed at execution. None was visible by reading.

### Critics reading source outside the artifact under review

- Evidence: the `m5` step critic listed the operator's work directory and found both `stopwatch`
  (approved) and `stopwatch-deprecated` (explicitly excluded), then blocked because the graph named
  targets as bare strings with no resolution rule. A prefix match would have retrofitted an excluded
  repository with a diff that looked correct.
- Effect: prevented an unrecoverable write to an out-of-scope repository.

### Executors that refuse instead of working around

- Evidence: `m5-s1` returned `BLOCK` / `PLAN_INFEASIBLE` twice — once because the worktree was not
  clean, once because the snapshot hit a FIFO — and left the tree untouched both times rather than
  deleting the obstacle. The `PLAN_INFEASIBLE` contract is doing real work.

### Moving the "verify your pinned bytes" instruction upstream

- Evidence: the first four steps of the run were each rejected on round 1 for pinning bytes that did
  not survive the repository's formatter. After the instruction was added to the *writer's* prompt
  rather than only the critic's, `m3-s2`, `m3-s3` and `m4-s3` were approved on round 1.
- Effect: a full plan/critic round saved per step. This currently lives in the orchestrator's prompt,
  not in the workflow.

## Friction and failures

### A `step-planner` revision emits a changelog instead of the revised artifact

- Severity: `high`
- Phase: `milestone/step planning`
- Observation: twice, a **revision** dispatch narrated its diff instead of emitting the artifact.
  At `m4-s1` the three-node graph was replaced by a single node titled
  `"Revised milestone-4 step graph"` whose summary described its own edit. At `m5-s1` all four node
  summaries were replaced with `"Revised to require ..."`, `"Now refuses to start unless ..."`,
  `"Now consumes ..."`, deleting the specification — including the 18 enumerated retrofit target
  names that round 1 had verified.
- Evidence: `milestone-4/steps.json` after the first revision; `milestone-5/review-2.json` finding
  `M5-B3`; `events.jsonl` deltas at `m4-s1` and `m5-s1`.
- Inference: initial authoring is reliable; **revision** dispatches are where this occurs. The agent
  appears to answer "what did you change?" rather than "emit the artifact".
- Impact: two destroyed approved artifacts, two planning rounds consumed, and at `m5-s1` it pushed
  the loop to its cap. Both were schema-valid: `graph.schema.json` constrains only that nodes carry
  `id`, `title`, `repo`, `depends_on`, `summary`, so changelog prose satisfies it. Structured output
  cannot detect the substitution because the defect is semantic and the schema is structural.
  Recovery was possible **only** because a gate artifact quoted the destroyed content — that is luck
  acting as a backup system.

### The verdict schema became invalid for the structured-output dialect mid-run

- Severity: `high`
- Phase: `execution`
- Observation: `~/.claude/skills/pce/schemas/verdict.schema.json` changed from a five-key
  `blocking_issues` item to a nine-key evidence-bearing item. Two objects declared `required` arrays
  that omitted a key present in `properties` — the outer item omitted `execution_ref`, and nested
  `replacement_execution` omitted the same. The OpenAI structured-output dialect requires `required`
  to enumerate **every** key in `properties`, so the API returned
  `400 invalid_json_schema ... Missing 'execution_ref'` and **no child spawned**.
- Evidence: `pce dispatch codex ... --role step-executor` exiting 1 with that payload at digest
  `5fdf0a24...`; the same file later valid at digest `2ebf226e...`.
- Inference: the schema is valid ordinary JSON Schema but not valid for the strict dialect Codex
  uses for `response_format`.
- Impact: every `step-executor` dispatch was dead on arrival. **The dangerous part is the asymmetry**:
  Claude gate dispatches are validated by `pce` *after* the child writes, and a gate returning an
  empty `blocking_issues` array never exercises the item schema — so gates kept passing. The
  `m5-s1` plan critic approved minutes before the first executor failure. A reviewer that actually
  *found* something would have failed validation instead. The breakage is invisible until a gate has
  a finding.

### `pce dispatch -o` writes into the worktree it is measuring

- Severity: `medium`
- Phase: `execution`
- Observation: `SKILL.md` places the executor's structured result at
  `-o <worktree-abs>/.codex-result.json`. Plans legitimately assert on `git status --short`, and the
  `m5-s1` plan required an empty status before editing. The harness artifact — plus a
  `pytest-of-<user>/` directory left by the orchestrator's own pre-dispatch `pce contract check` —
  made the tree dirty, and the executor correctly returned `PLAN_INFEASIBLE`.
- Evidence: `events.jsonl` delta at `m4-s3`; `git status --short --ignored` in that worktree showing
  `?? .codex-result.json` and `?? pytest-of-nicolaslazaro/`; the `m3-s3` PR reviewer independently
  flagged `.codex-result.json` as leaking into the tree.
- Inference: the harness pollutes the state it asks the child to satisfy.
- Impact: one wasted execution round. Worked around by writing `-o` to `/private/tmp`, which the
  sandboxed child can also write.

### Cross-repo runs break the `step-plan-writer` working-directory assumption

- Severity: `medium`
- Phase: `step planning`
- Observation: `step-plan-writer` for `m3-s1` (repository `rustplate`) was dispatched with
  `--cwd <rustplate>` and `--sandbox workspace-write`. Its output path, `plan.md`, lives under
  `VISION_DIR` in the **primary** repository, outside the child's writable workspace. The child
  reported the path read-only, changed nothing, removed its temp tree, and refused.
- Evidence: the child's own final message; `events.jsonl` delta at `m3-s1`.
- Inference: `SKILL.md` says the writer runs "at the primary root" but also `-C <repo-abs>`, which
  reads naturally as the step's repository. Only cross-repo runs expose the conflict.
- Impact: one wasted dispatch and one planning round.

### Artifact index collides between roles in the same step directory

- Severity: `medium`
- Phase: `execution / review`
- Observation: round indices key on `(node, role)`, so `pr-reviewer` at a step computes `n = 1` and
  would write `review-1.json` — the filename `step-plan-critic` already owns in the same `step-<s>/`
  directory. Following the rule literally overwrites an approved plan review.
- Evidence: `SKILL.md` "Routing, caps, and adaptation"; the run wrote `pr-review-<n>.json` instead
  and recorded the improvisation in a delta at `m1-s1`.
- Impact: destroyed audit history and corrupted stuck-detection input if followed as written.

### `dependency-inconclusive` recurs for every new milestone

- Severity: `medium` (previously reported as `high`; now confirmed to recur predictably)
- Observation: each new milestone's nodes classify `dependency-inconclusive` until its integration
  branch exists on the remote, because `git fetch` of a never-pushed ref exits 128.
- Evidence: observed at milestone 1, 2, 3, 4 and 5; each resolved by creating and pushing the
  integration branch.
- Impact: a mandatory manual step per milestone that the workflow does not name.

### `pce contract bootstrap` leaves the tracked contract uncommitted

- Severity: `low`
- Observation: `bootstrap` wrote `rustplate/.pce/repository-contract.json` but left it untracked.
  A fresh worktree therefore has no contract, and `pce contract check` in a step worktree fails.
- Evidence: `git ls-files .pce/repository-contract.json` empty in `rustplate` after bootstrap;
  fixed by committing it before m3 began.

## Recommendations

### Reject a graph node whose summary describes an edit

- Addresses: the changelog-instead-of-artifact pathology
- Change: add to the graph-critic obligation that any node whose `summary` narrates a change,
  revision, review, or difference from an earlier version is a **critical** blocking finding; and
  instruct revision dispatches to **re-author from an authoritative source** rather than edit in
  place.
- Location: `SKILL.md` Phase 1/Phase 2 critic obligations, and the revision-dispatch guidance.
- Trade-off: none. Both occurrences were caught by a critic, but only after destroying the artifact.
- Confidence: `high` — two occurrences, same role, same trigger.

### Validate installed schemas against the structured-output dialect at startup

- Addresses: the mid-run schema breakage
- Change: at startup, verify every installed schema satisfies the dialect Codex requires — in
  particular that every object with `properties` declares a `required` array enumerating all of
  them — and fail loudly with the offending path if not. A five-line recursive check.
- Location: the startup schema verification already described in `SKILL.md` step 3.
- Trade-off: rejects schemas that are valid ordinary JSON Schema but unusable for structured output.
  That is the intent.
- Confidence: `high` — the failure mode is silent for gates and total for executors.

### Move the executor's `-o` artifact out of the worktree

- Addresses: `-o` polluting the tree it measures
- Change: place the structured result outside the worktree (for example under the session temp
  directory), or add its filename to a harness-owned ignore mechanism.
- Location: `SKILL.md` Phase 3 step 3, which currently pins `-o <worktree-abs>/.codex-result.json`.
- Trade-off: the result is no longer adjacent to the work it describes.
- Confidence: `high` — cost one execution round, and a PR reviewer independently flagged it.

### State that `step-plan-writer` runs at the primary root, always

- Addresses: the cross-repo `--cwd` trap
- Change: make explicit that the writer's `--cwd` is the **primary** repository root regardless of
  the step's repository, because `plan.md` is written under `VISION_DIR`; the step's repository is
  read at its named ref.
- Location: `SKILL.md` Phase 3 step 1.
- Trade-off: none; this is a documentation fix.
- Confidence: `high`

### Give PR reviews their own artifact index

- Addresses: the index collision
- Change: either name PR-review artifacts distinctly (`pr-review-<n>.json`), or continue the index
  across roles within a step directory. State which.
- Location: `SKILL.md` "Routing, caps, and adaptation".
- Trade-off: a second naming convention to remember.
- Confidence: `high`

### Require pinned bytes to be verified at authoring time

- Addresses: the round-1 rejection pattern
- Change: add to the `step-plan-writer` obligation that any pinned file content must be
  reconstructed in a scratch directory and run through the repository's own formatter and linter
  before the plan is emitted, and that a pinned test whose correctness depends on a string not
  appearing in its own source must be checked **post-format**.
- Location: `SKILL.md` Phase 3 step 1.
- Trade-off: longer authoring dispatches.
- Confidence: `high` — measured: four steps needed two rounds before this instruction, three were
  approved on round 1 after it.

### Require a test that has been observed failing

- Addresses: the `m4-s4` importlib blind spot
- Change: when a step exists to fix a defect, require the plan to state how it verified the new test
  **fails against the unfixed code**, and require the critic to reproduce that.
- Location: `SKILL.md` Phase 3 step 1 and the plan-critic obligations.
- Trade-off: none for defect-fixing steps; not applicable to feature steps.
- Confidence: `high` — a packaging defect shipped past a plan critic and a PR reviewer because
  `tests/test_inventory_repositories.py` exercised the entry point through `importlib`, which
  resolves the package correctly, while `uv run python scripts/inventory_repositories.py` failed
  with `ModuleNotFoundError`. Both gates truthfully reported "the CLI contract is tested".

### Create the milestone integration branch as part of milestone startup

- Addresses: recurring `dependency-inconclusive`
- Change: name integration-branch creation and push as an explicit step at the start of each
  milestone, or have `pce ready` report the absent-ref cause in its `results` entries.
- Location: `SKILL.md` Phase 2, and `pce ready` output.
- Confidence: `high` for the problem; `medium` for which remedy.

### Commit the contract in `pce contract bootstrap`

- Addresses: the untracked contract
- Change: either commit `.pce/repository-contract.json` as part of bootstrap, or state in `SKILL.md`
  that the orchestrator must commit it before any worktree-based step in that repository.
- Location: `pce contract bootstrap`; `SKILL.md` Phase 0.
- Confidence: `medium` — committing on the operator's behalf may be unwanted; documenting is safe.

## No-change decisions

- **Round caps of 3.** `m5-s1` exhausted its cap and escalated correctly; the operator authorized one
  further round. The cap did its job — it stopped a loop and surfaced a decision. No change.
- **`PLAN_INFEASIBLE` as an executor contract.** It fired three times, correctly each time, and twice
  prevented an executor from deleting something to make a plan pass. Keep exactly as is.
- **`repository-analyst` producing a verdict-shaped artifact.** Still awkward — orientation's correct
  answer was "not yet built", which is neither `APPROVE` nor `BLOCK` — but inventing a second schema
  for one role is likely worse than the workaround. Observation only, unchanged from the prior report.

## Suggested follow-up

- **A run cannot verify work in repositories it holds no contract for.** Milestone 5 writes to 22
  repositories with no `.pce/repository-contract.json`, so there are no gates to run and no version
  policy to read. Every other milestone proved itself with five measured gates before merging; there,
  verification reduces to "the instruction file changed as intended and nothing else did". The run
  compensated by making a pre-retrofit snapshot mandatory and having both waves refuse without it,
  but this is a structural gap worth designing for rather than improvising per vision.
- **Both templates' `init.sh` are macOS-only.** Every substitution uses BSD `sed -i ''`, which GNU sed
  misparses, so a Linux colleague running `init.sh` does not get a correctly initialized project.
  Inherited, outside this vision's scope, recorded as a key finding at `m3-s3`.

---

## Finding 9 — `gh pr create --fill` in a detached worktree (tool defect surfaced by this run)

Not a `pce` defect, recorded because it is the class of bug the sandbox structurally hides.

`scripts/controlled_retrofit.py` published with `gh pr create --head <branch> --base main --fill`,
`cwd` = the detached temporary worktree. `--fill` resolves `origin/main...<branch>`; that branch has
no local ref there, so `gh` exits 1 — **after** the push succeeded.

Result: `hdx` ended with a pushed branch (`2a791ae2fdc6`, content byte-identical to pinned digests)
and no pull request. Three step-plan critic rounds and 19 sandboxed executor dispatches never
reached this line, because reaching it requires network plus GitHub auth.

**Implication for the dispatch model:** gate/executor sandboxes cannot exercise any publication path.
A plan can be adversarially approved three times and still contain an unexecutable publication step.
Worth considering whether the contract should mark publication paths as sandbox-unverifiable so the
critic is told not to treat their approval as evidence of executability.

## Finding 10 — `pce status` cannot parse a prose acceptance-criteria section

`pce status --file … --vision-dir …` now fails with:

```
failed to parse ratified acceptance criteria from …/vision.md
  acceptance-criteria section must contain exactly one fenced `json` object and no other content
```

This vision's `## Acceptance criteria` section is a numbered Markdown list, which is what the
skill's own vision template produces. Either the template or the parser is behind. Filed rather than
worked around: reshaping a mid-run vision to satisfy a changing binary contract would rewrite
ratified criteria, which is worse than losing status derivation. The event log is unaffected.

---

## Finding 11 — an adversarially approved plan failed at runtime; reading is not execution

m5-s3 passed three critic rounds and was APPROVED with `self_sufficiency: PASS`. It then halted at
its first preflight with zero publications.

The critics were not careless. They verified the plan's internal consistency, recomputed every
pinned digest, and swept for shell defects. What none of them did — because nothing required it —
was *run the pinned controller* and observe how it scopes its own refusals.

The final round did. That critic built a sandbox fleet under `/private/tmp` with a local bare origin
and executed the unmodified controller against a one-name manifest. It settled in minutes what three
rounds of reading had not: which directory each invocation reads, which zips can mismatch, and what
exact refusal a repeat attempt produces.

**Suggested contract change:** for a step whose `--planning-act` is `irreversible`, require the
critic to execute the pinned tooling against a synthetic fixture and report the observed output, not
only to reason about it. An approval that rests entirely on reading should not be able to gate an
irreversible act. The distinction is already encoded in `--planning-act`; the gate obligation could
follow from it.

## Finding 12 — authority scope must match the unit of work

The controller's `resolve_approved()` iterates `request.approved_names`, not `request.targets`. So a
one-target manifest still carried all seventeen approved names, and `require_fresh_snapshot()`
re-validated all seventeen stable triples on *every* invocation.

The plan had carefully bounded the operator's quiescence obligation to one repository at a time. The
code kept a wave-wide gate. Both were internally coherent; the mismatch was invisible in the plan
text and invisible in the code alone — it existed only in their composition.

Consequence: drift in any repository refused the *next* target, including after earlier irreversible
pushes. The wave that halted at preflight could equally have halted at target 12 with eleven
unrecoverable publications behind it.

**Generalizable rule worth stating in the doctrine:** when a plan narrows an obligation per unit of
work, something must mechanically verify the tooling's authority is narrowed the same way. A prose
claim of per-target scope is not evidence of per-target scope.

## Finding 13 — irreversible waves need a human, and the executor model cannot supply one

The approved m5-s3 requires a per-target confirmation typed at a controlling terminal. That is the
right design for seventeen direct-to-main publications under active concurrent orchestrators.

It also means neither the orchestrator nor any sandboxed executor can run the step. The orchestrator
shell has no controlling TTY (`read < /dev/tty` returns `Device not configured`), so execution
transferred to the operator via a written handoff.

This is a legitimate terminal state for a step, but the skill has no vocabulary for it. There is
`dispatch`, `delta`, `escalation-*`, `key-finding`, `planning-artifact-approved` — nothing that says
*this approved step is executable only by a human, and here is the handoff*. It was recorded as a
`delta`, which understates it: a reader of the event log cannot distinguish "work continues" from
"work has left the orchestrator entirely."

**Suggested addition:** an `operator-handoff` event kind, or a documented convention, carrying the
approved artifact digest and the handoff path. Related to finding 9 — the sandbox cannot exercise
publication paths, and here it cannot exercise the human gate either.
