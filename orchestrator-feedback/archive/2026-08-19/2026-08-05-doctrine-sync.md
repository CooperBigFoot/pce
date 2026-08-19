# PCE workflow feedback: doctrine sync

- Date: `2026-08-05`
- Orchestrator: Claude Code, Opus 5, ultracode session
- Run: `pyplate/planning/2026-08-04-doctrine-sync` (cross-repo: `pyplate` primary, `rustplate`)
- Outcome: `in progress` — Phase 0/1/2 complete and approved, Phase 3 dispatching at time of writing

## Executive summary

Phases 0–2 completed. The milestone graph converged in 2 planner/critic rounds and both step
graphs in 2 rounds each. The adversarial gates did real work: they deleted a false ordering edge,
refuted a planner threat model against source, and caught a hazard the orchestrator had propagated
from Phase 0.

Two findings blocked the run outright and each required a human decision to clear:

1. **A fresh run cannot establish its primary repository contract when `bootstrap` does not
   support the stack.** The known Rust-only limitation
   (`2026-08-01-bootstrap-is-rust-only.md`) composes with the contract-lifecycle authorization
   rules to leave *no legal path* for a CI-less Python primary repo. `pce status` then fails
   `resolve_primary_repository` and no phase can run.
2. **Every step node classified `dependency-inconclusive` on a fresh run**, because no run branch
   had ever been pushed. `pce ready` emits no reason field, so diagnosis required cross-referencing
   `pce status`. Confirmed by remedy: pushing the integration branches reclassified the nodes to
   `ready`.

## Evidence reviewed

- `pyplate/planning/2026-08-04-doctrine-sync/events.jsonl` (29 records at time of writing)
- `milestones.json`, `review-1.json`, `review-2.json`
- `milestone-1/steps.json`, `milestone-1/review-1.json`, `milestone-1/review-2.json`
- `milestone-2/steps.json`, `milestone-2/review-1.json`, `milestone-2/review-2.json`
- `orientation-pyplate.json`, `orientation-rustplate.json`
- Terminal output of `pce contract bootstrap`, `pce contract check`, `pce status`, `pce ready`
- `pce/orchestrator-feedback/2026-08-01-bootstrap-is-rust-only.md`

## What worked

### Adversarial graph critics with the burden of proof on edge presence

- Evidence: `milestones.json` round 1 contained `m4 -> m3` justified as "fleet operation across
  both template families requires rustplate's merged consumer". `review-1.json` finding E1
  (critical) refuted it at source: m4 consumes the per-repository check/sync primitive, which is
  m1's, and m3 exposes no interface m4's fan-out consumes. It also observed the first clause
  ("neither referenced tree contains fleet application code") is non-probative because it "would
  equally justify an edge from m4 to any node".
- Effect: a false ordering edge was deleted and the genuine `m4 -> m1` edge it was standing in for
  was added. This directly increases available concurrency.

### Critics checking the planner's cited facts rather than its conclusions

- Evidence: `milestone-1/review-1.json` finding B2 refuted the planner's stated preservation
  hazard — `init.sh` ends with `rm -- "$0"`, which removes only `init.sh` itself and "deletes no
  sibling file", so it cannot delete a sync command shipped at another path. The critic then named
  the real hazard (init renames `src/mypackage` and re-runs `uv sync`).
- Effect: prevented a step whose entire justification was a misread of one line of shell.

### Critics reading source outside the artifact under review

- Evidence: `milestone-1/review-1.json` finding B1 read `/Users/.../metis/AGENTS.md` and found no
  delimiter or stamp of any kind, only the literal heading `## 2. Design Doctrine`. The step's
  acceptance condition ("metis's older marker must remain readable") was therefore unsatisfiable.
- Effect: corrected a step acceptance criterion that would have failed at execution time. Note the
  critic reached outside the declared blast radius to do this; that was useful here, and the
  workflow neither authorizes nor forbids it explicitly.

### Appendable hazard propagation into later dispatches

- Evidence: the orchestrator propagated the measured hazard "sandboxed runners have no network, so
  a fresh tree must be seeded from a warm `.uv-cache`" into the step-planner prompts.
  `milestone-2/review-1.json` finding B1 then caught that the proposed regression test runs
  `init.sh` — which ends in `uv sync` — inside a fresh copy whose `.uv-cache` is empty, and demanded
  the step say how it avoids an offline sync.
- Effect: the hazard channel worked end to end, from Phase 0 measurement to a Phase 2 gate finding.

### `pce ready` as sole readiness authority

- Evidence: after the integration-branch push, m1 returned `ready | m1-s1`, `waiting | m1-s2`,
  `waiting | m1-s3` — matching the approved graph's `m1-s2 -> m1-s1` and `m1-s3 -> m1-s1` edges.
- Effect: no orchestrator-side dependency bookkeeping was needed.

## Friction and failures

### A fresh run has no legal path to its primary repository contract when `bootstrap` cannot derive gates

- Severity: `high`
- Phase: `orientation / contract lifecycle`
- Observation: `pce contract bootstrap --repo-root <pyplate> --repository pyplate --node m1-s1`
  exited with `cannot bootstrap CI-less repository without Cargo.toml at default-branch HEAD`.
  `SKILL.md` authorizes a hand-authored `repository-contract` append only when
  `.pce/repository-contract.json` is *present* at default-branch HEAD and the log has no current
  record, and states "Never construct or append a repository-contract payload by hand in any other
  case." With the tracked file absent and bootstrap unavailable, both paths were closed. `pce status`
  then failed with `expected exactly one primary repository containing both log ... and vision ...,
  found 0`, because the only contract established was `rustplate`, whose root prefixes neither path.
- Evidence: terminal output of both commands; `events.jsonl` sequence 1 (rustplate contract) and
  sequence 2 (`escalation-open`, key `pyplate-contract-bootstrap-unsupported`).
- Inference: the Rust-only derivation limitation is already documented as out of scope, but its
  *composition* with the lifecycle authorization rule is what produces a hard stop. The rule is
  written as if bootstrap always succeeds for a repository lacking the tracked file.
- Impact: the run could not start. Cleared only by a human authoring and committing
  `.pce/repository-contract.json` to `pyplate` `main` (commit `20fb590`), outside the run.

### The event log must already exist before any verb can write to it

- Severity: `medium`
- Phase: `startup`
- Observation: `pce contract bootstrap --file <LOG_PATH> ...` failed with
  `failed to open event log <LOG_PATH>` / `No such file or directory (os error 2)` on a fresh run.
  `pce --help` lists no verb that creates a log. The orchestrator improvised `touch`.
- Evidence: terminal output of the first `pce contract bootstrap` invocation; `pce --help` usage block.
- Inference: every writing verb opens the log for append without creating it.
- Impact: an undocumented manual step on every fresh run. It also opens a window in which the log
  exists but is empty — a state `SKILL.md` says must "stop loudly" — so an interruption between
  `touch` and the first successful append leaves a run that cannot start *or* resume. In this run
  that window was survived only because the immediately following bootstrap failed on a different
  repository, leaving the empty file in place until a later append succeeded.

### The tracked contract file shape is undocumented and differs from the log payload shape

- Severity: `medium`
- Phase: `orientation / contract lifecycle`
- Observation: `SKILL.md` specifies the `repository-contract` **log payload** in full (flat
  `stated.format`, `stated.lint`, ..., plus `branch_convention` and `pull_request_convention` as
  single strings, plus `workflow_map`). The **tracked file** written by `bootstrap` uses a different
  shape: nested `stated.gates{}`, `stated.branches{}`, `stated.pull_requests{}`, `stated.workflows[]`,
  and no `observations`, `repository`, `repo_root`, or `evidence`.
- Evidence: `rustplate/.pce/repository-contract.json` as written by `pce contract bootstrap`, compared
  against `events.jsonl` sequence 1 payload and the `SKILL.md` "Event log contract" section.
- Inference: the two shapes are deliberately different, but only one is documented.
- Impact: hand-authoring the tracked file for `pyplate` required reverse-engineering the shape from
  bootstrap's output in the *other* repository. An orchestrator without a second, bootstrappable repo
  in the same run would have had no reference at all.

### Gate children are never shown the verdict schema they are validated against

- Severity: `medium`
- Phase: `orientation` (first occurrence; applies to every gate)
- Observation: the first `repository-analyst` dispatch produced a substantively correct result but
  wrote `blocking_issues` as an array of strings. `pce dispatch` rejected the artifact after the child
  had exited, with `violates schema keyword/location 'type' at instance '/blocking_issues/0' ... is not
  of type "object"`, and the whole dispatch was lost.
- Evidence: terminal output of the first `pce dispatch gate ... --role repository-analyst`; the child's
  own final message correctly identified the artifact as not yet built.
- Inference: `--output-schema` is applied as post-hoc validation. The gate caller tail passes only the
  output *path* via `--append-system-prompt`; the schema text reaches the child only if the caller
  chooses to inline it. `SKILL.md` does instruct the orchestrator to "quote its JSON verbatim into
  Claude prompts", so this is orchestrator error — but it is error the binary is positioned to make
  impossible, since it already holds the schema bytes.
- Impact: one wasted gate dispatch and one retry. Every subsequent gate in this run inlined the schema
  verbatim and none failed validation again.

### Every step node classifies `dependency-inconclusive` until a run branch is pushed, with no diagnostic

- Severity: `high`
- Phase: `step planning -> execution handoff`
- Observation: with both step graphs approved, `pce ready --graph <steps.json>` returned
  `dependency-inconclusive` for all four nodes (`m1-s1`, `m1-s2`, `m1-s3`, `m2-s1`) and `ready` for
  none, so Phase 3 could dispatch nothing. `SKILL.md` forbids treating the classification as `waiting`
  or `ready`, so the run stopped. The `results` objects carry only `classification`, `node`, and
  `repository` — no reason — so diagnosis required reading `pce status`, which showed
  `git.availability: unreachable` with failure
  `git ... fetch --no-tags origin refs/heads/pce/doctrine-sync/milestone-1 exited exit status: 128;
  ... couldn't find remote ref`, alongside `github: reachable, zero-exact-matches` and
  `merge_status: inconclusive`.
- Evidence: `pce ready` output before and after remedy; the `steps` array of the `pce status` snapshot;
  `events.jsonl` sequences 27 and 29 (`escalation-open` / `escalation-close`,
  key `step-readiness-dependency-inconclusive`).
- Inference: `git fetch` of a never-created remote ref exits 128, which maps to *unreachable* rather
  than to an absent-branch observation. `SKILL.md`'s startup section anticipates this shape only for
  exit 1 ("git exit 1 for a missing branch is an absent branch observation"). A branch that has never
  been pushed is indistinguishable from an unreachable remote, and merge status stays inconclusive.
  Note the affected nodes included `m1-s1` and `m2-s1`, which have **empty** `depends_on` — so the
  classification did not depend on any actual dependency edge.
- Impact: a fresh run is hard-blocked at the Phase 2/3 boundary. Creating the integration branch
  locally was tested first and did **not** resolve it. Pushing
  `pce/doctrine-sync/milestone-1` and `pce/doctrine-sync/milestone-2` to `origin` reclassified
  `m1-s1` and `m2-s1` to `ready` immediately, confirming the cause.

## Recommendations

### Treat a never-pushed integration ref as an absent-branch observation

- Addresses: "Every step node classifies `dependency-inconclusive` until a run branch is pushed"
- Change: when `git fetch` of an integration or step ref fails specifically with
  `couldn't find remote ref` (exit 128), record it as an *absent branch* observation rather than
  *unreachable*, matching the existing exit-1 handling. An absent run branch on a fresh run is the
  expected state, not an unobservable one.
- Location: the fetch-observation logic behind `pce status`'s `steps[].git`, and the startup
  discrimination paragraph in `SKILL.md` that currently names only exit 1.
- Trade-off: a genuinely unreachable remote that happens to produce the same message would be read as
  absent. The message is specific enough to distinguish, and `github.availability` remains a second
  signal.
- Confidence: `high` — the remedy was tested and reversed the classification.

### Emit a reason alongside every non-`ready` classification

- Addresses: same finding
- Change: add a `reason` string to each element of `pce ready`'s `results`, naming the unresolved
  observation (for example `merge status inconclusive: integration ref not present on origin`).
- Location: the `results` object emitted by `pce ready`.
- Trade-off: a wider output contract for consumers to tolerate.
- Confidence: `high` — diagnosis here required a second command and a manual cross-reference, and the
  orchestrator was explicitly forbidden from guessing.

### Give the gate child the schema the binary will validate against

- Addresses: "Gate children are never shown the verdict schema"
- Change: have `pce dispatch gate` append the `--output-schema` file's contents to the child's system
  prompt automatically, rather than relying on the caller to inline it.
- Location: gate caller-tail construction in `pce dispatch gate`.
- Trade-off: a larger system prompt per gate, and one more thing the anchored route owns.
- Confidence: `medium` — `SKILL.md` already requires the caller to do this, so the change removes a
  failure mode rather than fixing a specification gap.

### Add a log-creating startup verb, or create the log on first append

- Addresses: "The event log must already exist"
- Change: either create the parent file on first append in the writing verbs, or add an explicit
  `pce log init --file <LOG_PATH>`. The second is preferable because it keeps "an existing empty log
  stops loudly" meaningful.
- Location: `pce --help` usage block and the log-append path.
- Trade-off: one more verb, and `SKILL.md`'s startup section would need to name it.
- Confidence: `high` for the problem; `medium` for which of the two remedies is right.

### Document the tracked contract file shape next to the log payload shape

- Addresses: "The tracked contract file shape is undocumented"
- Change: add the tracked `.pce/repository-contract.json` shape to `SKILL.md` beside the existing
  `repository-contract` payload block, stating explicitly that the two differ and how fields map.
- Location: `SKILL.md`, "Event log contract" section.
- Trade-off: none beyond document length.
- Confidence: `high`

### Narrow the hand-append prohibition to cases where bootstrap can actually run

- Addresses: "A fresh run has no legal path to its primary repository contract"
- Change: extend the authorized hand-append case to also cover "bootstrap exited non-zero for an
  unsupported stack and no tracked file exists", requiring the same evidence discipline (every
  `observations` value sourced from a real gate execution). This is a `SKILL.md` change only and does
  not depend on fixing bootstrap's stack coverage.
- Location: `SKILL.md`, Phase 0 contract-lifecycle paragraph.
- Trade-off: widens the one place the orchestrator may author a contract payload directly. The
  evidence requirement is what keeps it honest, and the alternative today is a hard stop.
- Confidence: `medium` — the alternative remedy is to fix stack coverage in `bootstrap`, which
  `2026-08-01-bootstrap-is-rust-only.md` already scopes as separate work. These are complementary,
  not competing.

## No-change decisions

- **Gate children reading source outside the declared blast radius.** `milestone-1/review-1.json`
  read `metis/AGENTS.md`, a repository outside the run's declared repos. It produced the single most
  valuable finding of Phase 2. The vision names `metis` explicitly as the drift test case, so this was
  reasonable. Not proposing a rule in either direction on the evidence of one occurrence.
- **Round caps of 3.** Both graph loops and both step-graph loops converged in 2 rounds. No cap was
  approached, so this run provides no evidence about whether 3 is the right number.
- **`repository-analyst` producing a verdict-shaped artifact.** Orientation is not a gate, and
  `APPROVE`/`BLOCK` fits it awkwardly — the correct orientation answer here was "the artifact does not
  exist yet", which is neither. The orchestrator worked around it by instructing the child to treat
  not-yet-built as `APPROVE`. This is friction, but inventing a second schema for one role is likely
  worse than the workaround. Recording it as an observation only.

## Suggested follow-up

- The sandbox interaction with `uv` cost substantial diagnosis time this run and is not a `pce` defect,
  but `pce`-spawned gate children are where it surfaces: gate children cannot write `~/.cache/uv`
  (`Operation not permitted`) and have no network (DNS failure). Repositories must therefore keep an
  in-workspace cache **and** pre-seed fresh worktrees from a warm one. Consider whether
  `pce contract check` should detect and report this class of failure distinctly from a genuine red
  gate, since the two are indistinguishable in its current output (`stated gate command ... exited with
  status 2`).
- `uv` older than 0.12 panics under a macOS sandbox inside `system-configuration` 0.6.1
  (`Attempted to create a NULL object`, upstream `astral-sh/uv#18629`), which fails every `uv build`
  variant. Any future Python stack support in `bootstrap` will meet this.
