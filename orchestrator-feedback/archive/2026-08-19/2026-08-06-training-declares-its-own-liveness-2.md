# PCE workflow feedback: training declares its own liveness (continued)

- Date: `2026-08-07`
- Orchestrator: Claude Code, Opus 5, fresh ultracode session
- Run: `palaestra/planning/2026-08-06-training-declares-its-own-liveness` (cross-repo: `palaestra` + `nostos`)
- Outcome: `in progress` — milestones 1 and 2 merged; continues into 3 and 4

Continuation of `2026-08-06-training-declares-its-own-liveness.md`, which covered Phase 0 and the
contract-measurement Seatbelt blocker (F1–F4 there). This report covers milestone 1 step 2 through
milestone 2, and supersedes nothing in the first.

## Executive summary

Four new findings, three of them structural. The verdict schema was edited three times while the run
was in flight, discarding two correct reviews (F5). `pce dispatch gate` reports a schema-violating
artifact and still exits 0, so an orchestrator routing on exit status accepts it (F6). The
contract-measurement Seatbelt profile and the executor's own sandbox differ in ways no agent can
observe from inside, which shipped a falsely-green test and then made the fix unverifiable by the
executor (F7). And `pce ready` cannot classify a milestone whose merged step set exceeds its
approved graph — which is exactly what runtime graph adaptation produces, making the delta mechanism
and the readiness verb mutually incompatible (F8).

F8 is the one worth acting on first: it has no workaround inside the documented workflow, and it
silently removes the run's only readiness authority.

## Evidence reviewed

- `planning/2026-08-06-training-declares-its-own-liveness/events.jsonl` (dispatch, dispatch-completion, key-finding, delta, escalation records)
- Verdict artifacts under `milestone-1/step-2/` and `milestone-2/`, including two quarantined `*.schema-violating.json`
- `~/.claude/skills/pce/schemas/verdict.schema.json` at three distinct mtimes during the run
- PRs 115–120 in `CooperBigFoot/palaestra`; `pce contract check` output in three worktrees
- `strings` on the installed `pce` binary and on `torch/lib/libomp.dylib`

## What worked

### Mandatory re-validation of gate artifacts

- Evidence: dispatch-completion sequence 31 records `artifact_outcome: schema-violating` with `exit_status {kind: exited, code: 0}`.
- Effect: the skill's instruction to re-validate the artifact after every gate return is the only thing that caught it. Without it the run would have accepted an `APPROVE` whose JSON omitted `summary` and `non_blocking_notes` and carried `root_cause: null`. This rule is load-bearing, not ceremonial.

### The plan/critic loop falsifying orchestrator hypotheses before code

- Evidence: for one defect I proposed three mechanisms — `TMPDIR=str(tmp_path)`, `KMP_WARNINGS=off`, and an executor-run `sandbox-exec` probe. The plan critic refuted the first from `strings libomp.dylib` (no `TMPDIR`/`TMP`/`TEMP` literal; `/tmp` hardcoded), I refuted the second by measurement, and the executor refuted the third by returning `PLAN_INFEASIBLE` with `sandbox_apply: Operation not permitted`.
- Effect: three wrong fixes cost planning rounds instead of merged commits. The executor's refusal to work around an impossible instruction — committing nothing, opening no PR — is exactly the specified behaviour and it worked.

### Critics permitted to run probes

- Evidence: once gate prompts allowed read-only probes, the m2 step critic measured `input_len=99` (empties the training split) and `input_len=5` (train=1/val=0, saves a 0.0-loss checkpoint) on the tiny fixture, and the m1-s2 round-3 critic verified the mandated code was ruff's fixed point by running `ruff format` and diffing.
- Effect: claims that had passed through two agents citing each other got settled against reality. Reviewers that can only read produce plausible reviews; reviewers that can execute produce correct ones.

## Friction and failures

### F5 — The installed verdict schema changed three times mid-run, discarding correct reviews

- Severity: `high`
- Phase: `step planning`, `execution`
- Observation: `verdict.schema.json` was read at startup with a 5-key `blocking_issues` shape. At mtime 09:53 it required 8 keys (`input`, `observation`, `replacement_execution` added). At 10:10 it required 9 (`execution_ref`, pattern `^execution-[0-9]{6}$`, at two levels). It then reverted to the 8-key shape.
- Evidence: two verdicts quarantined as `milestone-1/step-2/review-1.schema-violating.json` and `review-2.schema-violating.json`. Both obeyed the contract stated in their own dispatch prompt and failed validation against the file as it stood minutes later. Round 1's analysis was independently excellent — it is the one that falsified the `TMPDIR` mechanism.
- Inference: `pce` is under active development (confirmed by the operator), and the skill treats installed schemas as a stable startup-verified authority. Those two facts are incompatible.
- Impact: two of three plan-critic rounds for node m1-s2 were consumed by format drift rather than plan defects, which is what exhausted the cap and forced a human escalation to extend it. `execution_ref` also demanded an identifier whose referent is defined nowhere in the skill or schemas — emitting one would have been fabricating a reference.

### F6 — `pce dispatch gate` exits 0 on an artifact it judged schema-violating

- Severity: `high`
- Phase: any gate
- Observation: sequence 31 carries `artifact_outcome: schema-violating` and `exit_status {kind: exited, code: 0}`.
- Evidence: the same log also shows `not-validated` for the prose `step-plan-writer` (correct, no schema) and `validated` elsewhere, so the field is meaningful — it is only the exit status that fails to reflect it.
- Inference: the artifact outcome is recorded for the audit trail but not wired to the process result.
- Impact: any orchestrator that routes on exit status silently accepts malformed verdicts. Combined with F5 this is how a schema change becomes a wrong merge rather than a loud stop.

### F7 — Three sandboxes disagree, and no agent can observe the difference from inside

- Severity: `high`
- Phase: `isolate`, `execution`, `merge`
- Observation: three distinct sandbox behaviours matter, and nothing surfaces them:
  1. The contract-measurement profile denies `$TMPDIR`. Torch's OpenMP runtime then writes `OMP: Warning #179: Function Can't set size of /tmp file failed:` to a fresh subprocess's stderr.
  2. The codex executor sandbox permits `/tmp`, so the same test suite is green there.
  3. A seatbelt sandbox cannot nest inside the executor's sandbox: `sandbox-exec` fails with `sandbox_apply: Operation not permitted`, exit 71.
- Evidence: milestone 1 merged two regressions asserting `completed.stderr == ""`. They passed the executor's gates, passed my independent unsandboxed re-run, and passed the pre-execution worktree contract check (which ran before the tests existed). The first time they met the measurement sandbox was the post-merge `pce contract refresh`, which failed 2/341 and left `main` red on its own contract check. The repair then could not be verified by the executor at all, because (3).
- Inference: the executor is asked to make gates pass in an environment that is not the environment those gates are later judged in.
- Impact: one falsely-green merge, one extra delta node, one full extra step (plan ×4, critic ×4, executor ×3), and one human escalation. Also note `pce dispatch` appends issuance *before* spawning, so a child killed seconds after launch still consumes a round — I had to reconcile one such productless dispatch with a `delta`.

### F8 — `pce ready` cannot classify a milestone that runtime graph adaptation has touched

- Severity: `high` (no workaround inside the documented workflow)
- Phase: `readiness`, both altitudes
- Observation: after milestones 1 and 2 merged, `pce ready --graph milestones.json` returns `dependency-inconclusive` for `m3`, whose only dependency is `m1`, and no invocation makes it resolve.
- Evidence: `milestone-1/steps.json` contains exactly `['m1-s1']`; `m1-s2` exists only as a prose `delta` record; `pce status` reports `m1-s1`, `m1-s2`, `m2-s1` all `merged`; both step squash commits `20e74c2` and `4e62990` are ancestors of `main`; no palaestra PR is open. Creating and pushing the `milestone-3` integration branch did not change the classification (that remedy fixed a different `dependency-inconclusive` case earlier in this run, where the integration branch was genuinely absent). The no-`--graph` default path emits non-JSON and fails to parse, reproducing `2026-08-04-values-carry-the-descriptors-that-dimension-them.md`.
- Inference: readiness reconciles a milestone's merged step set against its approved step graph, and a runtime stub is deliberately absent from that graph. `SKILL.md` states a delta node has "no second durable representation" and that merged nodes stay byte-identical, so the graph is *correct* to omit `m1-s2` — the two mechanisms simply cannot both hold.
- Impact: the skill names `pce ready` "the sole readiness authority", and after the first delta node it can no longer serve that role for the affected milestone or anything downstream of it. I proceeded by establishing m3's readiness from graph edges plus measured git ancestry and recorded the deviation as a `key-finding`, but that is judgement substituting for the verb — precisely what the skill forbids doing silently.

## Recommendations

### R5 — Make the gate exit status reflect the artifact outcome

- Addresses: F6
- Change: exit non-zero when `artifact_outcome` is `schema-violating`, `schema-invalid`, `missing`, or `truncated`.
- Location: the gate dispatch completion path in `src/main.rs`.
- Trade-off: callers currently treating exit 0 as "child ran" must distinguish spawn success from artifact validity. That is the point.
- Confidence: `high`

### R6 — Pin the schema per dispatch

- Addresses: F5
- Change: have `pce dispatch` resolve and hash the schema once at issuance, record that digest in the dispatch record, and validate the returned artifact against those bytes rather than re-reading the file. A mid-flight edit then cannot invalidate a conforming child.
- Location: `pce dispatch` schema handling and the dispatch/dispatch-completion payloads.
- Trade-off: adds a digest field; the log grows slightly.
- Confidence: `high` — I implemented the orchestrator-side half of this (snapshot the schema at dispatch, validate against the snapshot) and it removed the failure mode immediately.

### R7 — Reconcile runtime deltas with readiness

- Addresses: F8
- Change: pick one. Either (a) `pce ready` treats a merged canonical step node absent from the approved graph as satisfying rather than inconclusive — reading `delta` records for the node's declared `depends_on`; or (b) the workflow gains a supported way to record a delta node in the graph without invalidating its approval digest. (a) is smaller and keeps merged artifacts byte-identical.
- Location: the readiness dependency resolver; `SKILL.md` § *Routing, caps, and adaptation* if (b).
- Trade-off: (a) makes readiness trust `delta` prose, which is not schema-validated. Requiring an explicit `id` and non-empty `reason` in the stub — which `SKILL.md` already mandates — is probably enough.
- Confidence: `high` that it needs fixing; `medium` on which option.

### R8 — Make the measurement sandbox reachable before merge

- Addresses: F7
- Change: expose the contract-measurement environment as something a step can be checked against before its PR merges — e.g. a `pce contract check --profile measurement` the orchestrator runs in the step worktree (which is what I ended up doing by hand), and document that the executor's sandbox is NOT that environment. Separately, granting `$TMPDIR` in the measurement profile would remove the OpenMP noise class entirely (see R1 in the first report).
- Location: the contract-measurement profile builder; `SKILL.md` § Phase 3 step 2.
- Trade-off: none beyond making an existing capability explicit.
- Confidence: `high`

### R9 — Do not count an issuance whose child never ran

- Addresses: the round-accounting note in F7
- Change: either append issuance after a successful spawn, or record a completion with an explicit `aborted` outcome so round derivation can exclude it.
- Location: `pce dispatch` issuance ordering.
- Trade-off: an issuance-after-spawn ordering loses the record if the process dies between spawn and append; the `aborted` completion avoids that and is preferable.
- Confidence: `medium`

## No-change decisions

- **The executor's `PLAN_INFEASIBLE` contract.** It fired correctly on an impossible instruction, committed nothing, and opened no PR. The defect was in my plan, not in the rule.
- **Runtime graph adaptation producing prose-only stubs.** The mechanism is right; F8 is a gap in readiness, not an argument for a second durable representation.
- **The 3-round caps.** They were exhausted once, and the cause was F5 consuming rounds on format drift, not the caps being too tight. Fix the cause.

## Suggested follow-up

- One issue covering R5 and R6 together: both concern verdict artifacts being trusted without being validated against a stable contract, and they compound.
- One issue for R7. It is the only finding here with no in-workflow workaround, and every run that adapts its graph at runtime hits it from that point on.
- R8 belongs with R1 from the first report — same profile, same root cause.
