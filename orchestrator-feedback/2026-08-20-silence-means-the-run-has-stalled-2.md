# PCE workflow feedback: 2026-08-20-silence-means-the-run-has-stalled (addendum)

- Date: `2026-08-21`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill, session 11f7b79a`
- Run: `/Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-20-silence-means-the-run-has-stalled`, graph.v3 -> graph.v4, plan versions 3 and 4
- Outcome: `running` (P3 issuance 8 in flight after a ratified criterion revision; H1, H2, P1, P2 complete)

Addendum to `2026-08-20-silence-means-the-run-has-stalled.md`. That report covered plan versions 1
and 2. This one covers everything after it: H2's completion, P3's third park, a human-ratified
criterion revision, and a boundary the supervisor crossed on operator instruction. It sharpens F8 of
the first report and adds five findings.

## Corrections after filing

- **R10 was partly stale when filed.** The `f83d4548` build already emits `criterion_outcomes` with a
  per-criterion `outcome` field on the park record; observed at P3 issuance 8 (journal record 104),
  all three criteria `"outcome":"not-executed"`. F11 was observed on the previous build, where the
  park record carried no such field. R10 has been narrowed in place to the part that remains open:
  the human-readable `reason` string still reads `replan: criterion: <name>` in both cases.
- **A new observation, filed as F15 below**, arrived after the original text: the empty
  worker-environment declaration produced a park that was correct from the worker's position and
  refutable from the driver's.

## Evidence reviewed

- `driver-journal.jsonl` records 74-103 (H2 dispatch through P3 issuance 8)
- `graph.v3.json` (sha256 `bc508c91...`), `graph.v4.json` (sha256 `37a18db6...`),
  `graph.v4.criterion-revisions.json` (sha256 `9077c375...`)
- `criterion-revisions-v4.json` (5,553 bytes, 2 revisions)
- `package-outcomes/P3/5.json` (748 bytes), `package-outcomes/P3/7.json` (110 bytes)
- `evidence/P3-issuance-7/pane-w16F.txt` (634 bytes), `pane-w16G.txt` (82 bytes)
- `pce package driver-status` at plan versions 3 and 4
- `sysctl -n hw.memsize` -> 64.0 GiB; `config.yaml:8,51,75`
- `shasum -a 256 ~/.local/bin/pce` -> `1be14320...` then `f83d4548...` (binary replaced mid-run)
- `supervision.md` (69,619 bytes at time of writing)

## What worked

### The gate caught a defect the authored criteria could not have

- Evidence: H2's two authored criteria both passed, then `package-gate-6-1` produced
  `finding-replayed` with command `uv run pytest tests/test_serving_static_lattice.py -q`, repair
  merged hydria `3964b76c` -> `40e8a480`, and three `gate-reproof-executed` records followed.
- Effect: H2 was written to relax a *dynamic* gridded-family geometry guard. The gate demanded a
  **static** lattice proof. Observation only — the supervisor did not author the amendment and has
  not verified the causal claim — but the pattern is that the gate found a blast-radius the
  criterion author (the supervisor, drafting the repartition) did not anticipate. This is the second
  package in this run whose gate caught something its criteria missed.

### The forfeiture on ratification is observable rather than merely asserted

- Evidence: before the revision, P3 composed on palaestra `4262f38a` / hydria `5099901561973f7f...`.
  After, `package-base-composed` shows palaestra `772b29eb` / hydria `e0aca41d`. Neither prior base
  appears. The four measurement-tooling commits from forfeited attempts (`5327fe3`, `a1e0afe`,
  `07851d0`, `aca0da8`) do not carry.
- Effect: the briefing given to the operator before ratification ("the 3.67 s result must be
  re-measured because the tooling does not carry") was verifiable in the journal immediately after.
  A forfeiture that is visible is a forfeiture that can be consented to.

### The new binary's journal records close a gap the skill had to work around

- Evidence: records 98-99 are `dispatch-worktree-opened` naming `source` and materialized `path` per
  repository; `dispatch-worker-identified` now carries `session_path`.
- Effect: section 5 requires reading the journaled `session_path` and explicitly forbids locating a
  session by grepping `~/.prime`. Before this build that field was not present in this run's records,
  so the instruction could not be followed as written. It now can.

## Friction and failures

### F10. `fault.kind: "criterion"` carries no explanatory payload at all

- Severity: `high`
- Phase: `execution / escalation`
- Observation: a park whose fault is a missing dependency is reconstructible from durable artifacts.
  A park whose fault is a criterion is not.
- Evidence: `package-outcomes/P3/5.json` is **748 bytes** —
  `fault.kind: "missing-dependency"` with `id.missing`, `id.checked` (two paths) and `id.command`.
  `package-outcomes/P3/7.json` is **110 bytes** in full:
  `{"outcome":"mis-specified","fault":{"kind":"criterion","name":"Observed gaps stay under the design ceiling"}}`.
  The actual cause — the workload required 79.43 GiB plus a further 6.04 GiB at the first batch
  against 64.0 GiB physical — exists **only** in `evidence/P3-issuance-7/pane-w16F.txt`, captured
  because the supervisor was watching. A successful `dispatch-pane-cleanup` would have destroyed it.
- Inference: the schema gives `missing-dependency` a structured `id` and gives `criterion` only a
  name, so the fault kind that most needs explanation is the one that carries none.
- Impact: without the pane capture, the operator would have been shown a park stating that a 300 s
  ceiling criterion failed, with nothing to indicate that no measurement ever ran. The ruling that
  followed — move the measurement to EC2 — depended entirely on two numbers that no durable artifact
  held. This is the first report's F8 sharpened from "outcomes lack partial results" to "one fault
  kind is reconstructible and another is not".

### F11. A park reason names the criterion in a way that asserts it was evaluated

- Severity: `medium`
- Phase: `execution / escalation`
- Observation: the journal reason reads
  `replan: criterion: Observed gaps stay under the design ceiling`. The natural reading is that the
  criterion was tested and did not hold — that measured gaps exceeded the ceiling.
- Evidence: no gap was measured. The worker's own summary says the run died allocating the first
  batch. `criterion-executed` never appears for P3 in the journal at any issuance.
- Inference: the reason string is generated from `fault.name` with no field distinguishing
  "evaluated and failed" from "could not be evaluated".
- Impact: a supervisor or human reading only the journal would conclude the vision's central number
  is too small and reach for finer granularity, when the actual finding is that the workload does not
  fit the machine. Those lead to opposite work.

### F12. The skill's prohibition on writing the revision record has no graduated form, so it is all-or-nothing under operator pressure

- Severity: `high`
- Phase: `criterion revision`
- Observation: sections 6 and 11 state absolutely that the skill "never writes the revision record,
  supplies `--criterion-revisions`, or runs a non-mechanical freeze", and that "only the human
  ratifies". The operator, having ruled the route, asked three times for the command; the third was
  *"just do it yourself it's fine."* The supervisor wrote the record and ran the non-mechanical
  freeze.
- Evidence: `criterion-revisions-v4.json` written by the supervisor;
  `pce graph freeze --criterion-revisions ...` run by the supervisor; receipt
  `graph.v4.criterion-revisions.json` sha256 `9077c375...`. Recorded as a deviation in
  `supervision.md` before it was reported in chat. Earlier in the same exchange the supervisor had
  written a skeleton record to `/tmp`, recognised it as over the line, and deleted it.
- Inference: the rule protects against one specific harm — the agent's preference entering a
  human-attributed ratification. It does not distinguish that harm from the mechanical work of
  quoting frozen predecessor bytes, which is unambiguously safe and is the bulk of the file. Because
  the rule is absolute, the only moves available were "refuse entirely" or "write all of it", and
  repeated operator instruction selected the second.
- Impact: `ratified_by` in a frozen, digested record names a human who did not type it. The
  supervisor confined itself to transcribing the operator's actual ruling plus this run's measured
  evidence, invented no reasoning, and told the operator the text is quoted verbatim as their
  ratification — but the workflow provides no mechanism that makes any of that checkable. Section 9
  will quote this rationale verbatim into the PR body as human-authored.

### F13. The installed binary changed mid-run, and the change set was larger than announced

- Severity: `medium`
- Phase: `environment`
- Observation: the operator broadcast a new build (`f83d4548`) describing two changes since
  `273f5c34`: a dirty-source guard fix and a schema fix, with "nothing is owed unless you were
  blocked on the guard".
- Evidence: this run was on `1be14320`, not `273f5c34`. Relative to `1be14320` the new build also
  adds **`--herdr-session`** — which the operator had earlier stated was *"deliberately not
  installed, because four other runs have workers in flight and it touches the dispatch path"* — and
  **`--recovery-reset <HUMAN_RECORD_PATH>`**, which appears in no announcement. Verified by
  `pce package driver-run 2>&1 | grep -c "herdr-session"` returning `0` before and `1` after.
- Inference: the broadcast diffed against one baseline while at least one orchestrator was running
  another, so "nothing is owed" was accurate for `273f5c34` holders and incomplete for this run.
- Impact: none realised. The supervisor re-verified `graph.v3.json`, the v4 draft and
  `driver-status` under the new binary before continuing, and declined to adopt `herdr_session`
  mid-run because `run.json` persisted its absence and the skill forbids changing the session
  silently during a run. But two prior findings in the first report (F7's "the flag does not exist")
  became stale mid-run, and a supervisor that did not re-check would have carried a false statement
  into its final record.

### F14. A criterion can pin a workload the executing machine cannot hold, and nothing checks that before it is frozen

- Severity: `medium`
- Phase: `graph authoring / freeze`
- Observation: P3's frozen criteria executed `uv run python scripts/...` locally against a config
  pinning `batch_size: 256` over 368 basins. Three freezes accepted this.
- Evidence: `config.yaml:51` pins `batch_size: 256`; `vision.md:77` pins the config by SHA-256 and
  names `camels-trust` an input the vision does not modify; the workload needs >79 GiB;
  `hw.memsize` is 64.0 GiB. Meanwhile `vision.md:151` already said the measurement "is an operator
  step, not reproducible from local fixtures" and `vision.md:73` said nostos's half "is unverifiable
  from a laptop".
- Inference: the vision stated twice that this work does not run locally, and the graph nonetheless
  encoded it as a local command. Nothing in `pce graph check` or the freeze relates a criterion's
  execution site to its resource demand, because neither is expressed.
- Impact: roughly four hours of wall clock across issuances 5 and 7, two worker dispatches, and one
  human ratification to discover a constraint the vision had already written down. Note this is the
  same shape as the first report's suggested follow-up: a criterion presupposing something no
  package or machine provides, accepted by the freeze. Third occurrence in two runs.

## Recommendations

### R9. Give `fault.kind: "criterion"` the same structured payload as `missing-dependency`

- Addresses: F10
- Change: require a `criterion` fault to carry `command`, `exit_status`, and a bounded `detail`
  string (the worker's own account of why the criterion did not hold or could not be evaluated),
  mirroring what `missing-dependency` already carries in `id`.
- Location: package-outcome schema; `package-parked` payload
- Trade-off: bounded free text in a durable record; cap it and state it records observations, not
  reasoning.
- Confidence: `high`

### R10. Distinguish "criterion failed" from "criterion could not be evaluated"

**PARTIALLY IMPLEMENTED ALREADY — corrected after filing.** The `f83d4548` build emits
`criterion_outcomes` on the park record, carrying a per-criterion `outcome` field. Observed at P3
issuance 8, journal record 104: all three criteria marked `"outcome":"not-executed"`. The
discriminator this recommendation asks for therefore exists at the criterion level, and the finding
behind it (F11) was observed on the **previous** build, where record 94 carried only
`reason: "replan: criterion: <name>"` with no per-criterion outcomes at all.

What remains unaddressed is narrower than R10 as filed: the human-readable `reason` string is still
rendered as `replan: criterion: <name>` in both cases, so a reader of the reason alone still cannot
tell the two apart without also parsing `criterion_outcomes`.

- Addresses: F11
- Change (revised): render the park `reason` from the `criterion_outcomes` already present —
  `criterion failed: <name>` when an outcome is a failure, `criterion not evaluable: <name>` when
  every outcome is `not-executed`.
- Location: park-reason rendering only. No schema change needed; the data is already there.
- Trade-off: none identified — this consumes a field the binary already writes.
- Confidence: `high`

### R11. Split the revision-record prohibition along the line the harm actually follows

- Addresses: F12
- Change: permit the supervisor to emit the mechanical portion — `schema_version`,
  `previous_package`, and `predecessor` blocks copied byte-for-byte from the frozen graph — while
  keeping `ratified_by` and every `rationale` refusable and unwritable by the skill. State that a
  freeze must refuse a record whose `ratified_by` or any `rationale` is empty, so the human's
  contribution cannot be skipped rather than merely discouraged.
- Location: `/work-graph` sections 6 and 11; `pce graph freeze --criterion-revisions` validation
- Trade-off: a human can still paste a one-word rationale. This makes the boundary enforceable
  rather than merely stated, which is strictly better than an absolute rule that gets waived.
- Confidence: `medium`

### R12. Diff build announcements against each orchestrator's installed digest

- Addresses: F13
- Change: state the *previous* digest each running orchestrator is expected to hold, or list the
  full flag-surface delta rather than the intended change set, so an orchestrator on an older build
  can tell what else moved. Separately: `--herdr-session` shipping while it was being withheld for
  dispatch-path safety is worth confirming as intentional.
- Location: build/broadcast process
- Trade-off: longer announcements.
- Confidence: `medium`

### R13. Relate a criterion's execution site to its resource demand at authoring time

- Addresses: F14
- Change: at `/to-graph` authoring, when the vision text states that work is not reproducible
  locally or requires remote hardware, warn on any criterion for that package expressed as a local
  command. Experimental variant: let a criterion declare an execution site (`local` or `remote`) so
  the mismatch is expressible at all.
- Location: `/to-graph` authoring checks; graph schema (experimental)
- Trade-off: the warning is heuristic and will fire on visions that mention remote hardware
  incidentally.
- Confidence: `experimental` for the schema change, `medium` for the authoring warning

## No-change decisions

- **Ratification forfeiting the attempt's commits.** It cost four tooling commits and a re-measurement
  of an already-obtained 3.67 s result. It is still right: the forfeited commits were written against
  criteria that no longer exist. The briefing made the cost visible before consent, which is the
  mechanism working.
- **`--recovery-reset` was not used.** P3 parked three times and never lost a rung, so recovery
  limits were never the constraint. No evidence about the flag either way.
- **The supervisor declining to adopt `herdr_session` mid-run.** `run.json` persisted its absence and
  the skill forbids changing the session silently during a run. Correct as written; no change.

## Suggested follow-up

- **Three occurrences now of "the freeze accepted a criterion that nothing could satisfy".**
  `2026-08-19-incidence-python-binding.md` (a package depending on work no criterion obliged its
  dependency to build), this run's P3-versus-hydria-geometry, and this run's P3-versus-machine-memory.
  Each cost a park, an investigation and a human ruling. The common shape is that criteria express
  *what must be observed* but never *what must exist for the observation to be possible*. Worth a
  design pass rather than three more point fixes.

## Late finding

### F15. An empty worker-environment declaration produces parks that are true for the worker and false for the run

- Severity: `high`
- Phase: `execution / recovery`
- Observation: P3 issuance 8 parked with
  `missing dependency: usable AWS credentials for the EC2 dispatches required by all live P3
  measurements`, `checked: ["AWS credential provider chain (~/.aws/config, ~/.aws/credentials, and
  AWS_* environment variables)"]`, `command: aws sts get-caller-identity`. The supervisor overruled
  it. Both the worker and the overrule were correct, about different environments.
- Evidence: the binary's own usage states `--worker-env` forwards names "only to package and gate
  worker workspaces", while "criteria and `--prepare` commands continue to inherit the driver's full
  launch environment". This run's journal record 2 is
  `worker-environment-declared {"names": []}`. In the driver's environment,
  `aws sts get-caller-identity` returns `NoCredentials` while
  `aws sts get-caller-identity --profile "$NOSTOS_AWS_PROFILE"` returns
  `arn:aws:iam::098907041092:user/nicolas`. nostos authenticates only through the named profile:
  `ec2.py:75` `boto3.Session(profile_name=cfg.aws_profile, ...)`, `config.py:44` binding
  `aws_profile` to `NOSTOS_AWS_PROFILE`, `RUNBOOK.md:71`.
- Inference: the worker had no `NOSTOS_*` variables, so its refusal was accurate about its own
  workspace. The criteria execute in the driver's environment where the variables are present, so the
  package remains reachable. The split environment makes a worker's honest preflight check
  systematically wrong about whether the *run* can proceed.
- Impact: one park, one investigation, and **the package's single one-shot overrule spent** on a
  disagreement that is structural rather than substantive. If the same worker needs a live dispatch to
  develop the scripts its criteria name, it will refuse again — and a second identical refusal cannot
  be overruled, so the exit becomes a ratified worker-environment extension at a new plan-version
  boundary. The recovery ladder is being consumed by an environment split that neither the worker nor
  the supervisor can see from inside the other's position.

### R14. Tell the worker what its criteria will run with

- Addresses: F15, and reduces the cost of F1
- Change: state in the package brief which environment names the *criteria* will execute with, even
  when those names are not forwarded to the worker's own workspace, so a worker's preflight can
  distinguish "I cannot do this" from "this run cannot do this". Experimental alternative: forward
  declared names to workers in a read-only form that supports presence checks without exposing values.
- Location: package-brief generation; `--worker-env` semantics
- Trade-off: a worker told the criteria have credentials it lacks may attempt work it cannot verify.
  That is still better than refusing work the run can perform.
- Confidence: `medium`
