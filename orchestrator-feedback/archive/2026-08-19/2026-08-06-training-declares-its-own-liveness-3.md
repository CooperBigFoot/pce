# PCE workflow feedback: training declares its own liveness (continued 2)

- Date: `2026-08-07`
- Orchestrator: Claude Code, Opus 5, ultracode session
- Run: `palaestra/planning/2026-08-06-training-declares-its-own-liveness` (cross-repo: `palaestra` + `nostos`)
- Outcome: `in progress during milestone 3` — `m3-s1` merged; `m3-s2` planning dispatched at sequence 127

This continues `2026-08-06-training-declares-its-own-liveness.md` and
`2026-08-06-training-declares-its-own-liveness-2.md`. It covers the `m3-s1` merge, recovery of
status authority, and readiness for `m3-s2`.

## Executive summary

Five new PCE defects and one workflow-hardening opportunity surfaced. The highest-impact new defect
is that a successful JSON status snapshot grew to 40,825 bytes while the skill requires the entire
snapshot to be quoted verbatim and prohibits a duplicate status artifact. The agent transport
truncated it, a manual seven-chunk reconstruction was corrupted, and the run needed a human exception
(F9).

Three authority projections then disagreed or misreported target state. Status returned
`no-log-visible-candidate` while the approved graph contained `m3-s2` and readiness classified it
ready (F10). Machine-readable `gh --json` became invalid JSON when inherited force-color variables
added ANSI escapes (F11). Status reported PCE's hard-coded `v0.1.16` tag in both target repositories
rather than Palaestra's relevant `v0.1.89` tag (F12). `pce vision check` also concealed its stdin-only
input contract behind a misleading structural parse error (F13).

Separately, the orchestrator placed a mandatory status probe and its guarded merge in one script;
the probe failed and the merge still ran. This was an orchestrator process error, not a PCE binary
failure, but it supports one narrow skill hardening rule (F14).

Two blockers reproduced defects already filed in `2026-08-07-doctrine-sync-2.md`: the lack of a
legacy boundary for prose acceptance criteria and the Codex-incompatible installed verdict schema.
They are cross-referenced rather than counted as new findings. The initially claimed status exit 0
was withdrawn after an isolated measurement returned 1 and is not another F6 reproduction.

## Evidence reviewed

- `planning/2026-08-06-training-declares-its-own-liveness/events.jsonl`, especially sequences
  101, 110–127.
- `planning/2026-08-06-training-declares-its-own-liveness/vision.md` before and after the approved
  acceptance-criteria migration.
- `planning/2026-08-06-training-declares-its-own-liveness/milestone-3/steps.json`.
- PR #121, squash commit `3663410bd1ce2d48e0b9fa77e8301b38a7cd9877`, and tag `v0.1.89`.
- Harness capture
  `~/.claude/projects/-Users-nicolaslazaro-Desktop-work-palaestra/4aca67da-f69c-4857-8aa8-1f677567b4ae/tool-results/b8ymx51og.txt`.
- Installed `pce` behavior and source in `src/main.rs`, including `run_vision_check`,
  `observe_github`, and `RELEASE_TAG`.
- `2026-08-07-doctrine-sync-2.md`, findings 10 and 11, for prior reports of the acceptance-format
  and structured-output-schema defects.

## What worked

### The operator recorded the unauthorized merge instead of rewriting history

- Evidence: event 124 states that PR #121 was already merged without the required successful,
  separately observed status authorization. It preserves the two approving review rounds and the
  landed commit/tag rather than reverting them to conceal the breach.
- Effect: the audit trail distinguishes materially reviewed product bytes from workflow authority.
  No milestone merge or cleanup followed.

### Strict acceptance parsing produced an executable criterion floor after human ratification

- Evidence: after a human approved the exact SHA-256 of a one-for-one conversion of all eight
  criteria, `pce vision check < vision.md` and `pce status` exited 0. Event 125 records the semantic,
  non-verbatim changes to criteria 6 and 7 rather than claiming byte preservation.
- Effect: recovery preserved criterion order and strength and did not silently rewrite the ratified
  source. The already-filed defect is the absent legacy/migration boundary, not the strict target
  representation.

### Step-plan criticism prevented an expensive measurement that could not falsify criterion 5

- Evidence: `milestone-3/step-2/review-1.json` returned `BLOCK` with five executed findings. The
  first plan required seven 525-basin runs to normal completion although all required observations
  exist at the first heartbeat naming epoch 2; three amplified arms would emit about 17 records per
  second. Its criterion-5 comparison also measured control epochs on a 60-second heartbeat grid and
  amplified epochs on a 0.06-second grid, so grid alignment dominated the effect under test. The
  critic required exactly two epochs and the already-captured `process_elapsed_seconds`, which has
  the same approximately 10 ms resolution in both arms. It also replaced an unattainable per-batch
  contiguity assertion with recorded sampling-density measures, restored `uv.lock` before the
  version-only edit, and anchored fail-closed packet validation to the pinned `train.py` digest and
  recomputed arithmetic.
- Effect: the plan was blocked before the operator resource request. The review avoided seven
  unbounded GPU runs and prevented a measurement from passing or failing on sampling quantization
  rather than telemetry overhead.

## Friction and failures

### F9 — Complete status authority is too large for the orchestrator transport

- Severity: `high`
- Phase: `status authority / merge`
- Observation: the isolated successful status payload was 40,825 JSON bytes plus a newline. The
  harness returned only a preview and automatically persisted the full output. A seven-part manual
  reconstruction was not byte-faithful: chunk 6 contained duplicated/malformed material and chunk 7
  was repeated with a trailing fragment.
- Evidence: the automatic capture is 40,840 bytes including `STATUS_EXIT=0`; its JSON prefix parses
  as `pce.run-snapshot` v1. The JSON plus emitted newline has SHA-256
  `dbd327054dfa3e9754b3efc003f045cfa34c7a022ceba2f6fbfb3331493bbc4a`. `SKILL.md` § Status
  authority requires the complete emitted snapshot verbatim and prohibits a second progress file.
- Inference: although `recovery_digest` elides older entries, the complete snapshot still grows with
  dispatch, round, step, provenance, and evidence projections. The quotation rule assumes an
  unbounded transport.
- Impact: a successful command could not authorize the next action without a human exception.
  Repeating the probe only increases the snapshot.

### F10 — Status omits approved graph-only work from resume

- Severity: `medium`
- Phase: `recovery / readiness`
- Observation: the valid snapshot reports `{"state":"no-log-visible-candidate"}` and ends at merged
  `m3-s1`. The digest-matching approved step graph contains `m3-s2`; `pce ready` against that exact
  graph returns:

  ```json
  {"results":[{"classification":"ready","node":"m3-s2","repository":"palaestra"}]}
  ```

- Evidence: event 101 records approval digest
  `256435f52d5f2e020f35a17493f78aaf8ae35fccd3458ce5f01e141ceb695307`, which still matches the
  graph. The snapshot confirms `digest-matches` but does not include `m3-s2` because it had no log
  record until the later planning dispatch at sequence 127.
- Inference: status deliberately reports a log-visible candidate while ready reports graph
  readiness. The state name is technically accurate, but the split is operationally misleading
  because status is also described as the resume authority.
- Impact: status can be read as permitting an early milestone merge or indicating completion when
  approved work remains. `pce ready` prevented that outcome after human intervention.

### F11 — Machine-readable `gh --json` inherits presentation controls

- Severity: `medium`
- Phase: `readiness and repository observation`
- Observation: with `FORCE_COLOR=1` and `CLICOLOR_FORCE=1`, `pce ready` exits 1 even though
  `gh pr list --json …` exits 0. The stdout contains ANSI escapes and PCE reports
  `malformed successful gh JSON output` / `expected value at line 1 column 1`.
- Evidence: `src/main.rs:4580-4610` invokes `gh … --json` and passes `result.stdout` directly to
  `serde_json::from_slice`. Re-running with
  `FORCE_COLOR=0 CLICOLOR_FORCE=0 NO_COLOR=1` exits 0 and returns `m3-s2` ready byte-for-byte with
  the independent readiness measurement.
- Inference: a machine-protocol subprocess inherits caller presentation settings.
- Impact: readiness becomes environment-dependent and fails before returning a classification.

### F12 — Status probes a hard-coded unrelated tag in each target repository

- Severity: `medium`
- Phase: `status authority / release tracking`
- Observation: the snapshot reports `v0.1.16` for both Palaestra and Nostos. For Palaestra it points
  to `832c8696…`; the landed step is version `0.1.89` at `3663410…` with tag `v0.1.89`.
- Evidence: `src/main.rs:143` defines `const RELEASE_TAG: &str = "v0.1.16"`; target-repository
  observation consumes the constant at lines 4386 and 4544. The snapshot reproduces it under both
  repositories.
- Inference: PCE's own release identifier is projected as target-repository release state.
- Impact: status cannot verify the per-repository tag required by the tracked version policy and
  presents an unrelated old tag as authoritative state.

### F13 — `pce vision check` conceals its stdin-only input contract

- Severity: `low`
- Phase: `recovery / vision migration`
- Observation: invoking `pce vision check` from the vision directory without redirection produced
  `must contain exactly one ## Acceptance criteria section`, although the on-disk `vision.md` was
  valid. `pce vision check < vision.md` exited 0.
- Evidence: `src/main.rs:2385-2392` reads only `input: &mut dyn Read`; the command accepts no path.
  Usage shows only `pce vision check`, so neither the invocation nor error identifies stdin as the
  missing input.
- Inference: the parser correctly diagnoses the empty document, but the CLI omits the boundary
  context explaining why the document was empty.
- Impact: one unnecessary diagnosis round and a plausible false conclusion that the parser rejected
  a valid file.

### F14 — The merge-probe sequencing rule needs an explicit process boundary

- Severity: `medium`
- Phase: `step merge`
- Observation: the orchestrator put `pce status` and `gh pr merge` in one script. Status failed, but
  the merge still executed. The orchestrator also initially read `$?` after a pipe to `head`, then
  withdrew the resulting false exit-code claim after an isolated measurement.
- Evidence: event 124 records the failed isolated status reproduction, already-merged PR #121,
  commit `3663410`, and the adopted separate-invocation remedy.
- Inference: this is an orchestrator control-flow error. The skill requires status immediately before
  merge, but it does not explicitly prohibit placing the check and action in one compound command.
- Impact: the workflow's merge authority was bypassed accidentally even though the delivered bytes
  had passed two review rounds.

## Reproductions of previously filed defects

### Legacy prose visions cannot cross the executable-criteria boundary

`pce status` rejected this pre-existing ratified vision until a human approved a semantic conversion
to the sole fenced JSON object. This reproduces `2026-08-07-doctrine-sync-2.md` finding 10. It is not
counted as a new finding here.

### The installed verdict schema was invalid for Codex structured outputs

Events 111–115 record HTTP 400 `invalid_json_schema`, an empty worktree, and a human repair adding
`execution_ref` to `required` at both object levels. The same provider-specific schema defect is
already reported in `2026-08-07-doctrine-sync-2.md` finding 11. This run adds a second reproduction
and the fact that reinstalling the then-committed schema would restore the blocker.

## Recommendations

### R10 — Separate compact decision authority from full recovery evidence

- Addresses: F9
- Change: make default JSON status a bounded snapshot containing schema identity, holds, candidate,
  merge/removal decision, repository observations, and digests. Move verbose dispatch history,
  evidence, and recovery projections behind `pce status --full` or filtered retrieval commands.
  Require the compact authority, not the full audit projection, to be quoted before actions.
- Location: `RunSnapshot` projection and `skills/pce/SKILL.md` § Status authority.
- Trade-off: detailed recovery requires a second query; the compact snapshot remains verifiable by
  digest.
- Confidence: `high`

### R11 — Reconcile status resume with approved graph provenance

- Addresses: F10
- Change: either accept the exact approved graph on `pce status --graph` or return an explicit state
  such as `readiness-query-required` whenever digest-matching approved graphs contain nodes absent
  from the log projection. Do not let the rendering imply that no work remains.
- Location: run-state resume derivation and status CLI.
- Trade-off: status gains a graph input or a less decisive resume state.
- Confidence: `high`

### R12 — Sanitize presentation variables for machine-output subprocesses

- Addresses: F11
- Change: for `gh --json`, set `NO_COLOR=1` and remove or force off `FORCE_COLOR` and
  `CLICOLOR_FORCE`. Add a regression with both force variables set.
- Location: the `observe_github` process envelope in `src/main.rs`.
- Trade-off: none for a machine-only call.
- Confidence: `high`

### R13 — Derive target tags from the repository contract

- Addresses: F12
- Change: remove global `RELEASE_TAG` from target-repository observation. Derive the expected tag
  from the repository's version and tracked `version_policy`, or omit tag state when the policy is
  `NONE`.
- Location: repository observation in `src/main.rs` and the repository-contract domain.
- Trade-off: version extraction may need to become an explicit per-repository contract field.
- Confidence: `high` that the hard-coded tag is wrong; `medium` on extraction mechanics.

### R14 — Make the `vision check` input boundary explicit

- Addresses: F13
- Change: accept `--file <VISION_PATH>`, or change usage/help and the empty-stdin error to state that
  the complete vision must be supplied on stdin.
- Location: `pce vision check` parsing and `run_vision_check` error context.
- Trade-off: a file option adds one CLI form; a documentation-only fix retains shell-redirection
  dependence.
- Confidence: `high`

### R15 — Require the status probe and guarded action to be separate invocations

- Addresses: F14
- Change: state in `SKILL.md` that a mandatory status probe must be its own tool invocation; the
  guarded merge/removal must not appear in the same shell, script, pipeline, or compound command.
  The action may be issued only after the complete successful output has been observed.
- Location: `skills/pce/SKILL.md` § Status authority and Phase 3 stage 7.
- Trade-off: one additional tool boundary per guarded action.
- Confidence: `high`

## No-change decisions

- **The withdrawn exit-code claim is not another F6 reproduction.** Isolated status exited 1. The
  apparent 0 came from reading `$?` after a pipe to `head`, not from PCE detaching status.
- **The acceptance migration was not silent.** A human approved exact bytes, all eight criteria were
  retained in order, and semantic wording changes were recorded.
- **`m3-s1` executor count 3 does not block `m3-s2`.** Budgets are per node.
- **F14 is not attributed to the PCE binary.** The concrete change belongs in the skill.

## Suggested follow-up

- Fix R12 and add its forced-color regression independently; it is small and directly reproduced.
- Design R10 before the next long run because snapshot growth is monotonic.
- Address R11 before status is used as a completion/resume signal on another graph with unstarted
  nodes.
- Remove or redesign the hard-coded release-tag observation before status is used as release proof.
