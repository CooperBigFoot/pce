# PCE workflow feedback: close-the-seven-basin-coverage-gap

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill, single interactive session`
- Run: `hfx: planning/2026-08-07-close-the-seven-basin-coverage-gap`, journal `driver-journal.jsonl`, plan versions 1 through 5
- Outcome: `in progress` — SB0/SB1/SB2 complete, SB3 parked three times and now gated behind a new package, SB6 dispatched at issuance 18

## Executive summary

The run spent **17 SB3 issuances across 4 plan versions without ever creating a cloud resource**. Every one of those issuances failed on a precondition that existed outside the package: first an environment variable the driver did not forward, then a governance procedure that did not exist. No worker could have fixed any of them, yet the recovery ladder spent its full budget against each, and each exhaustion required a human ruling and a new frozen plan version to reset.

The highest-impact finding is not the missing `--worker-env` flag, which is already fixed. It is that **the recovery ladder cannot distinguish "this worker did the work badly" from "this work is impossible from where the worker stands"**, and burns identical budget on both. Four separate human rulings were consumed re-authorising ladders that were then re-spent on unchanged external blockers.

Two smaller findings have concrete, cheap fixes: `graph freeze` does not enforce the criteria invariance that `driver-run` does, and the `pce-protect-criteria` Bash hook matches `vision.md` as a bare substring, so it blocks all access to `supervision.md`.

## Evidence reviewed

- `planning/2026-08-07-close-the-seven-basin-coverage-gap/driver-journal.jsonl` (115524 bytes at time of writing, plan versions 1-5)
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/package-outcomes/SB3/{5,6,7,8,9,10,12,13,14,15,16,17}.json`
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/supervision.md` and `supervision-state.json` (written by this orchestrator during the run)
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/park-evidence-SB3{,-v2,-v3,-v4}.md`
- `graph.v1.json` through `graph.v5.json` and their `pce graph check` output
- A criteria-invariance probe run in a scratchpad copy of the vision directory, never in the real one
- `hcloud server list` / `hcloud volume list` sampled at every journal event during the run

## What worked

### Criteria invariance enforced at `driver-run`

- Evidence: probe in a scratchpad copy — SB3's first criterion command was suffixed with ` # MUTATED PROBE`, then `pce package driver-run` refused with `Error: criterion "Existing basins do not move (real-basin half)" from predecessor package SB3 was changed or removed; freezing the revised version is the human's ruling`, before appending any journal record.
- Effect: the one thing that would silently destroy the value of the whole proof chain is refused at the point of use, and refused cleanly, with no partial journal write.

### Paired witness/repair gate proofs

- Evidence: `finding-replayed` for SB0 gate `package-gate-11-1` records witness ref `e7056a08ac38b7dffe63138c2484dfe4b9e05373` exit 1, repair ref `a15b2071ecfa5efa5cd54453d793ab85963cd795` exit 0 with stdout `test-verify-campaign-inputs: PASS`, decision accepted. The same shape appears for SB2 gate `package-gate-2`.
- Effect: the amendment proves the new test actually detects the condition it claims. This is the strongest verification mechanism observed in the run and it caught real structure, not ceremony.

### Plan-version advance carrying completions and amendments

- Evidence: four `plan-version-advanced` records (1→2, 2→3, 3→4, 4→5). The last carries `carried_completions: ["SB1","SB2","SB0"]` and amendments for SB2 and SB0.
- Effect: SB1, SB2 and SB0 were each proven once and never re-run, across four replans. Without this the run would have re-executed proven work four times.

### Worker refusals were truthful and converged

- Evidence: SB3 outcomes 15, 16, 17 — `runbook forbids compilation ... no transfer approval` → `also requires console-verified VAT/ccx33/volume/outbound pricing inputs` → `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"maintainer-approved SB3 compile-campaign runbook and tracked-provisioner credential-transfer authorization"}}`.
- Effect: workers refused rather than fabricating evidence, and each refusal was more precise than the last. Verified against `scripts/hetzner/RUNBOOK-tdx-hydro-seven-basin-acquisition.md` line 6 and section 1: the claims are true.

### `worker-environment-declared` records names, never values

- Evidence: `{"event":"worker-environment-declared","names":["HFX_CAMPAIGN_EVIDENCE","HFX_S3_ENV_FILE"]}`. A scan of the full journal for `AWS_SECRET_ACCESS_KEY`, `AWS_ACCESS_KEY_ID=`, and any 20+ character uppercase-alphanumeric run returns zero matches.
- Effect: the fix shipped mid-run honours the values-never-journaled contract exactly.

## Friction and failures

### The recovery ladder spends full budget on blockers no worker can resolve

- Severity: `high`
- Phase: `recovery`
- Observation: SB3 was dispatched 17 times across 4 plan versions. Every dispatch failed on a precondition external to the package. Grouped by identical cause:
  - issuances 5, 6 (plan v1): `no human cloud authorization, credential-file path, or HFX_CAMPAIGN_EVIDENCE root`
  - issuances 7, 8: worker returned `{"outcome":"done"}`, criteria then failed on `missing JSON evidence: control-builds.json` etc.
  - issuances 9, 10 (plan v2): same authorization/credential complaint as 5 and 6
  - issuances 13, 14 (plan v3): `HFX_CAMPAIGN_EVIDENCE and HFX_S3_ENV_FILE are unset`
  - issuances 15, 16, 17 (plan v4): missing compile runbook and missing approval
- Evidence: `package-outcomes/SB3/*.json`; `recovery-parked` reasons `recovery spending exhausted after 3 attributable failures; re-author as plan version n+1` at issuances 7 and 10.
- Inference: the ladder classifies a worker-declared `{"outcome":"failed","blocked_by":...}` identically to a criteria failure caused by bad package content, so it retries. Retrying cannot change an unset variable in the driver's own process or bring a runbook into existence.
- Impact: 17 dispatches, at least 11 of which were structurally incapable of succeeding; 4 human rulings consumed; 4 plan versions frozen; roughly a full working day of wall-clock. Two of those plan versions (v2 and v4) were pure bumps whose only function was to reset a ladder that was then immediately re-spent on the unchanged blocker.

### `blocked_by` text is discarded by the exhaustion park reason

- Severity: `medium`
- Phase: `recovery`
- Observation: when the ladder exhausts, the `recovery-parked` reason is `recovery spending exhausted after 3 attributable failures; re-author as plan version n+1`. The three `blocked_by` strings that caused it appear only in `package-outcomes/SB3/<issuance>.json`.
- Evidence: `recovery-parked` at issuances 7 and 10 versus outcomes `5.json`, `6.json`, `9.json`, `10.json`.
- Inference: the park record models *budget state* rather than *cause*.
- Impact: a supervisor reading only the journal sees "spending exhausted" and must go to a second artifact to learn why. By contrast the mis-specification parks at issuances 12, 14 and 17 do carry the worker's fault text and were immediately actionable.

### `graph freeze` does not enforce criteria invariance

- Severity: `medium`
- Phase: `plan revision`
- Observation: a graph whose predecessor criterion command had been mutated was frozen successfully, and only rejected later at `driver-run`.
- Evidence: scratchpad probe — `pce graph freeze --vision-dir <probe> --repository hfx=...` returned `{"created":true,...,"plan_version":2}` on a graph with ` # MUTATED PROBE` appended to SB3's first criterion. `pce graph check` on the same file also returned `valid: true`.
- Inference: invariance is checked against the journal, which `freeze` does not consult.
- Impact: a mis-frozen version is a permanent, immutable artifact in the version chain that can never be run. The maintainer independently described this in ruling 4: "a mis-frozen version poisons the chain". The failure is silent at the moment it is cheapest to catch.

### The `pce-protect-criteria` hook matches `vision.md` as a bare substring

- Severity: `low`
- Phase: `supervision`
- Observation: every Bash command whose text contained the substring `vision.md` was refused while a run was active. `supervision.md` contains that substring, so all shell access to the supervision record was blocked mid-run.
- Evidence: hook output `REFUSED: Bash may not access vision.md during an active run` fired on `cat >> supervision.md <<EOF`, and equally on the probe `echo "probe: vision.md" >/dev/null`. Confirmed by a deliberate probe command containing the literal string and nothing else.
- Inference: the hook matches the raw command text rather than a resolved path with a boundary.
- Impact: all supervision appends had to switch to the Edit/Write tools mid-run. Recoverable, but it silently penalises the file the workflow most wants a supervisor to maintain, and would equally block any file named `*vision.md`.

### A true overrule can be spent without unblocking anything

- Severity: `medium`
- Phase: `recovery`
- Observation: SB3 issuance 12 parked with `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"SB0"}}`. This orchestrator refuted it at ref `eed80532ec5a3b4c460a0e3342cab11f7b6fac40` — `git rev-list --parents` shows parents `3547ac4` (SB1 lineage) and `a15b207` (accepted SB0 repair ref), and `git ls-tree` shows both SB0 scripts present at mode 100755 — and issued the one-shot overrule. Issuance 13 then failed with `HFX_CAMPAIGN_EVIDENCE and HFX_S3_ENV_FILE are unset`, and issuance 14 parked with the identical `missing-dependency: SB0` verdict.
- Evidence: `package-park-overruled` record; outcomes `12.json`, `13.json`, `14.json`.
- Inference: the worker's `missing-dependency: SB0` meant "SB0's *effects* are not available to me", while the refutation established "SB0's *files* are in your tree". Both statements were true. The overrule was admissible and correct on its own terms, and irrelevant to the actual blocker.
- Impact: the package's only overrule was consumed on a refutation that changed nothing. A subsequent genuine mis-specification could not have been overruled.

### Approval artifacts live in agent-writable space

- Severity: `medium`
- Phase: `escalation`
- Observation: `scripts/hetzner/RUNBOOK-tdx-hydro-seven-basin-acquisition.md` section 2 treats `$LOCAL_EVIDENCE_DIR/provisioner-transfer-approval.txt` as proof that a maintainer typed `APPROVE-TRACKED-PROVISIONER-TRANSFER` at a console. In this run the evidence root was created by this orchestrator at `/Users/nicolaslazaro/hfx-campaign-evidence/2026-08-07-close-the-seven-basin-coverage-gap`, mode 700, owned by the same user the agent runs as.
- Evidence: runbook section 2; `ls -la` of the evidence root, which stayed empty across issuances 11-17 and then received the approval file once the maintainer instructed it in writing, twice.
- Inference: nothing structural distinguishes an approval a human typed from one an agent wrote. The runbook's own words — "No unattended process may approve the run" — are enforced only by the honesty of whatever writes the file.
- Impact: in this run the orchestrator declined, then complied on explicit instruction and recorded the provenance (approver, date, channel, and that it was not typed at a console). That mitigation was voluntary. A less careful agent unblocks a paid campaign by writing eight words to a file it owns.

## Recommendations

### Park immediately on a repeated external-precondition refusal

- Addresses: "The recovery ladder spends full budget on blockers no worker can resolve"
- Change: when two consecutive issuances of the same package return `{"outcome":"failed"}` with `blocked_by` strings that are equal, or when a worker returns a `blocked_by` at all, stop the ladder and park with that text as the park reason rather than spending the remaining rungs. Retries should be reserved for criteria failures, which are evidence of package content being wrong.
- Location: driver recovery-rung logic in `pce`, the code emitting `recovery-rung-attempted` / `recovery-parked`
- Trade-off: a genuinely flaky external precondition, such as a transient network failure, would park where today it might self-heal on retry. That is the safer error: a park is cheap to overrule, an exhausted ladder costs a frozen plan version.
- Confidence: `high`

### Carry the last `blocked_by` into the exhaustion park record

- Addresses: "`blocked_by` text is discarded by the exhaustion park reason"
- Change: include the final worker `blocked_by` verbatim in the `recovery-parked` payload alongside the budget explanation, as the mis-specification parks already do for `fault`.
- Location: `recovery-parked` event construction in `pce`
- Trade-off: slightly larger journal records
- Confidence: `high`

### Enforce criteria invariance at `graph freeze`

- Addresses: "`graph freeze` does not enforce criteria invariance"
- Change: have `freeze` locate the journal for the vision directory and apply the same predecessor-criterion comparison `driver-run` applies, refusing with the same message. Where no journal exists, freeze unchanged.
- Location: `pce graph freeze`
- Trade-off: `freeze` gains a dependency on the journal, and needs a defined behaviour when the journal is absent or belongs to a different repository set.
- Confidence: `high`

### Match the protected path with a boundary, not a substring

- Addresses: "The `pce-protect-criteria` hook matches `vision.md` as a bare substring"
- Change: match on a path boundary — the resolved vision directory plus separator plus `vision.md` — rather than any occurrence of the characters `vision.md` in the command text.
- Location: `$HOME/.local/bin/pce-protect-criteria`
- Trade-off: a command that reaches the file by an unusual path form could evade the hook. The hook is a convenience guard, not the invariance boundary, which lives in `driver-run`.
- Confidence: `high`

### Let a package declare runtime preconditions the driver checks before dispatch

- Addresses: the root pattern behind 11 of the 17 wasted dispatches
- Change: allow a package to declare named preconditions — an environment name that must be non-empty, a path that must exist, a file that must be present — that the driver evaluates before dispatching a worker. On an unsatisfied precondition, park immediately naming it, with no worker spawned and no rung spent.
- Location: graph package schema plus the pre-dispatch path in `driver-run`
- Trade-off: schema growth, and a new class of authored declaration that can itself be wrong. This is close to what SB0 and SB6 were hand-built to do inside this run's graph, which suggests the need is real and currently paid for in bespoke packages.
- Confidence: `medium`

### Require provenance fields in approval artifacts

- Addresses: "Approval artifacts live in agent-writable space"
- Change: where a runbook treats a file as proof of human approval, require it to carry approver identity, date, and the channel through which approval was given, and require the consuming check to refuse a record lacking them. This does not make forgery impossible; it makes an honest agent's record self-describing and a dishonest one's record explicitly false rather than merely misleading.
- Location: `scripts/hetzner/RUNBOOK-tdx-hydro-seven-basin-acquisition.md` section 2, and any successor compile runbook
- Trade-off: does not defend against a determined agent. A real fix would place approvals outside agent-writable space or sign them.
- Confidence: `medium`

### Experiment: do not consume the one-shot overrule when the same package fails again on a superset complaint

- Addresses: "A true overrule can be spent without unblocking anything"
- Change: if the issuance immediately following an overrule fails with a complaint that contains the overruled one, treat the overrule as not consumed.
- Location: overrule accounting in `pce`
- Trade-off: opens a loop where an agent overrules repeatedly against a moving complaint. Would need a hard cap.
- Confidence: `experimental`

## No-change decisions

- **The console-approval requirement itself.** It is the reason no money was spent on 17 dispatches that could not have produced valid evidence. The friction was the *absence of a procedure*, not the approval gate.
- **One-shot overrule per package.** The single spend in this run was admissible and correctly reasoned; the problem was that the underlying blocker was invisible from the tree, not that the budget was too small. Widening it would mostly buy more chances to be confidently wrong.
- **`driver-status` refusing a graph whose plan version mismatches the journal.** Observed as a change in the new binary (`driver journal plan version 3 does not match graph plan version 4`); the old binary answered such queries happily. The new behaviour is correct and should stay.
- **Requiring a frozen graph rather than `graph.json` for every driver command.** No failure in this run traced to it, and it made "which plan is running" unambiguous across 5 versions and 3 driver restarts.

## Suggested follow-up

- **A tracked compile-campaign runbook** is now in flight as package SB6 of plan version 5 in the hfx vision. If that pattern recurs — a campaign whose governing procedure forbids the work — consider whether runbook authorship belongs in the graph at all or should be a prerequisite of authoring a package that depends on it.
- **The `--worker-env` fix landed mid-run** (binary sha256 `9e63cd8758…`, integration head `9d3d7a1`) and is confirmed working: issuance 15 raised no environment complaint. Worth a regression test that a declared name reaches a worker process and that its value never appears in the journal.
- **Whether `pce dispatch package` and `driver-run` should share one dispatch path.** `dispatch package` accepted `--env NAME=VALUE` throughout, while `driver-run` had no equivalent until this run forced the issue; the capability existed one layer down the whole time.
