# PCE workflow feedback: 2026-08-07-close-the-seven-basin-coverage-gap (plan versions 9-10)

- Date: `2026-08-20`
- Orchestrator: Claude Code (Opus 5), `/work-graph` skill, session `5f9f76fa`
- Run: `planning/2026-08-07-close-the-seven-basin-coverage-gap` in `/Users/nicolaslazaro/Desktop/work/hfx`, `graph.v10.json`
- Outcome: running (SB11 complete, SB9 in flight at issuance 26)

Continues `2026-08-19-close-the-seven-basin-coverage-gap-2.md`. Findings already filed there and
in `archive/2026-08-19-evening/` are not repeated: worktree accumulation, `find`/`bfs`,
`graph freeze` not enforcing criteria invariance, and the `pce-protect-criteria` substring match
on `vision.md`.

## Executive summary

One high-severity finding, and it is the expensive kind: **nothing in the workflow checks that a
criterion's command enforces the criterion's own prose.** SB7's frozen criterion specifies "fails
naming any pair that remains ambiguous"; the script implementing it fails instead on inequality
against a hardcoded transcript of one campaign's numbers. The divergence was invisible while the
data was static and surfaced only when a successor campaign legitimately recorded different
numbers. Because the orchestrator had no way to detect the divergence, it diagnosed the failure
as an evidence-mutability problem and spent a plan version and a new package on that diagnosis
before the real cause became visible.

The second finding is the one that diagnosis was aimed at, and it is real on its own terms: a
completed package's proof can be silently invalidated by in-place mutation of a path outside git,
and pce records nothing that would detect it.

Both are reported with the correction sequence intact, because the orchestrator's own wrong turn
is the evidence for the first finding.

## Evidence reviewed

- `planning/2026-08-07-close-the-seven-basin-coverage-gap/driver-journal.jsonl` (264361 bytes at
  the time of review; events for issuances 24-26)
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/graph.v10.json` (frozen, sha256
  `48bca19b7faeb3981183e1cc4080fe4d993d51242183780d2d866379e4547b51`)
- `planning/2026-08-07-close-the-seven-basin-coverage-gap/supervision.md`
- `git show db4fb9c3e16a21f686e4b1432ce8afe2834c9acf:adapters/tdx-hydro/verify_orientation_against_campaign_evidence.py`
- `git diff 4f428fd97fde9e873075e77ce05ebac7b5bde433 bc8b9f40adcdd6f99a9329e45182686ff75fcd8b`
  (SB11 gate repair)
- `/Users/nicolaslazaro/hfx-campaign-evidence/2026-08-07-close-the-seven-basin-coverage-gap/campaign-record.json`
  and `/Users/nicolaslazaro/hfx-campaign-evidence/snapshot-campaign1-20260819/campaign-record.json`

## What worked

### `--worker-env` forwards names without leaking values

- Evidence: journal records `worker-environment-declared` and
  `worker-environment-extended {"plan_version": 10, "added_names": ["HFX_CAMPAIGN_EVIDENCE_V1"]}`.
  A journal-wide scan for the forwarded variables' *values* (two absolute paths and the credential
  file path) returns zero occurrences.
- Effect: the tooling-contract defect reported on 2026-08-19 is closed, and the fix honoured the
  constraint it was given — names journaled, values never. Three prior plan versions failed
  entirely on this gap.

### Re-edging moved a join conflict onto the package chartered to resolve it

- Evidence: `package-join-conflicted` SB9, `conflicting_input` SB7 `db4fb9c3`, `conflicted_paths`
  `["adapters/tdx-hydro/build_adapter.py"]`, immediately followed by `worker-dispatched` SB9
  issuance 26 — not a park. Under the earlier edge layout the same conflict parked SB3.
- Effect: the conflict was dispatched to the only package whose criteria authorise reconciling
  that function, instead of parking a package with no authority to touch it. This is the
  recommendation "Re-prove a package's criteria after a join conflict is resolved" from the
  previous report working in practice: ten `join-criterion-executed` records replayed every
  contributing package's frozen criteria after the merge.

### The gate caught a defect that the package's own criteria could not

- Evidence: `finding-replayed` SB11 gate `package-gate-25-1`, witness `705a900c` exit 1 with
  `ValueError: cannot read frozen SB7 verifier at db4fb9c...: fatal: not a tree object`, repair
  `bc8b9f40` exit 0.
- Effect: SB11's worker had obtained frozen files via `git archive <rev>` against the repository
  root, which works in the supervisor's worktree and fails in any tree lacking the object. All
  three of SB11's own criteria passed at exit 0 in the worker's environment; only the gate,
  running in a different materialization, exposed it. The repair replaced it with sha256-pinned
  vendored files. A package written to stop proofs rotting against moving artifacts had itself
  been anchored to one, and the gate is the only thing that noticed.

## Friction and failures

### Nothing verifies that a criterion's command enforces the criterion's prose

- Severity: high
- Phase: authoring, execution
- Observation: SB7's criterion "Every recorded real-basin ambiguity is settled" failed at exit 1
  under SB9's join replay with `FAIL: 2020003440 orientation pair changed: (147096, 148472) !=
  (665258, 666634)`. The criterion never asks for that comparison.
- Evidence: the frozen criterion's `input` reads "Take every compile refusal **recorded in
  campaign-record.json** ... and resolve each named reach pair under the delivered orientation
  rule"; its `observation` states the failure condition once — "fails naming any pair that
  **remains ambiguous**". The script at `db4fb9c` does read the pair from the record
  (`actual_pair = _pair(verdicts[basin])`, line 88) and then adds, at lines 87-92, an equality
  assertion against `ORIENTATION_BASINS`, a hardcoded transcript of campaign 1's numbers.
- Inference: the criterion is passing-capable at the current evidence root; the script refuses.
  The over-assertion is invisible for as long as the underlying data never changes, which is
  exactly as long as it takes for the divergence to be forgotten.
- Impact: the orchestrator initially recorded "the criterion cannot pass at the current evidence
  root, and criteria are immutable" — true of the script, false of the criterion — and escalated
  that reading to the maintainer. Plan version 10 and package SB11 (three criteria, one gate
  cycle, ~12,500 vendored lines) were authored against it. SB11 was not wasted: it discharged a
  genuinely stuck roster criterion, and its principle is what named this bug. But the cheaper fix
  was one repository script all along, and no mechanism existed to reveal that.

### A completed package's proof can be invalidated by in-place mutation outside git

- Severity: high
- Phase: state persistence, recovery
- Observation: SB7 completed against campaign 1's evidence. Campaign 2 wrote its results to the
  same `$HFX_CAMPAIGN_EVIDENCE` path, overwriting `campaign-record.json` in place. SB9's replay of
  SB7's criteria then failed, and issuance 24 parked `mis-specified / missing-dependency`.
- Evidence: `package-parked` SB9 issuance 24, reason naming `2020003440 as 147096->148472 while
  the frozen verifier requires 665258->666634`; the two `campaign-record.json` files differ at
  that key.
- Inference: pce anchors package inputs to git refs and verifies them, but criteria commands may
  read arbitrary external paths, and the journal records nothing about the identity or content of
  what was read. A completed package's proof therefore has an unrecorded dependency that nothing
  can detect changing.
- Impact: SB7's proof became unreproducible at its own recorded evidence root. It was recoverable
  only because the orchestrator had copied campaign 1's evidence to a snapshot directory before
  campaign 2 ran — an improvisation, not a workflow rule. Campaign 1's server and volume were
  destroyed at teardown, so without that copy the inputs were gone permanently.

### `package-failed` attributed a pre-existing criterion failure to the join, and three workers chased it

- Severity: high
- Phase: execution, recovery
- Observation: SB9 failed three consecutive issuances (26, 27, 28) with the driver-supplied reason
  `conflicted join broke parent criteria: SB7:Every recorded real-basin ambiguity is settled`, then
  `recovery-parked` on exhaustion. All three failed on the identical assertion
  `FAIL: 2020003440 orientation pair changed: (147096, 148472) != (665258, 666634)`.
- Evidence: the criterion was executed at SB7's **own** ref in a detached worktree at
  `db4fb9c3e16a21f686e4b1432ce8afe2834c9acf`, with no SB9 work present, against the same evidence
  root — identical failure, identical message. The join is not the cause; the criterion was already
  failing at the parent's ref.
- Inference: the driver appears to derive this reason from position (a criterion that fails after a
  conflicted join) rather than from comparison (does it also fail at the parent's ref?). Each worker
  was then handed a brief naming its own merge as the cause, and the observed behaviour is
  consistent with all three having worked the merge rather than the failing assertion.
- Impact: an entire recovery ladder spent on a defect that did not exist, ending in a park that
  requires a plan version to discharge. The actual fix is one line-range in one repository script.

### `package-join-conflicted` records carry no issuance

- Severity: low
- Phase: execution
- Observation: SB9 has two `package-join-conflicted` records (issuances 24 and 26). Neither
  contains an issuance field; the fields are `package`, `repository`, `base_oid`, `dependencies`,
  `conflicting_input`, `remaining_inputs`, `conflicted_paths`, `reason`.
- Evidence: both records dumped in full from the journal.
- Inference: attribution to an issuance is available only positionally, by reading the adjacent
  `worker-dispatched` record.
- Impact: minor for a human reader, real for any programmatic replay or audit that groups events
  by issuance. The two records differ only in that the second lists SB11 among `dependencies`,
  so distinguishing them requires diffing payloads.

## Recommendations

### Record the identity of external evidence a criterion reads

- Addresses: "A completed package's proof can be invalidated by in-place mutation outside git"
- Change: allow a criterion to declare external evidence paths, and have the driver record a
  content digest of each declared path in the `criterion-executed` record. On replay, compare and
  report a digest change as a distinct, named condition rather than letting it surface as an
  opaque command failure or a `missing-dependency` park.
- Location: criterion schema; `driver-run` criterion execution; journal `criterion-executed`
  payload.
- Trade-off: digesting large evidence trees is expensive; a declared-paths list is authoring
  burden, and a digest over a directory needs a defined traversal order.
- Confidence: medium — the failure is proven, the specific mechanism is a design proposal.

### Make the divergence between criterion prose and criterion command reviewable

- Addresses: "Nothing verifies that a criterion's command enforces the criterion's prose"
- Change: at gate time, require the gate to state explicitly whether the command's failure
  conditions are the ones the `observation` names, and to report any assertion the command makes
  that the observation does not. This is a prompt/checklist change to the gate brief, not a
  mechanism — the gate already reads both.
- Location: gate brief construction in `pce package agent` / gate dispatch.
- Trade-off: adds a judgement call to every gate and will produce false positives where prose is
  loose; it cannot be mechanically enforced.
- Confidence: experimental. The problem is proven and expensive; this is the cheapest probe of a
  fix, not a solution.

### Test a parent criterion at the parent's own ref before blaming the join

- Addresses: "`package-failed` attributed a pre-existing criterion failure to the join"
- Change: when a parent criterion fails after a conflicted join, re-run it at the parent package's
  own completed ref before composing the failure reason. If it fails there too, say so — the join
  is exonerated and the reason should name the criterion as already failing against current
  inputs, not the merge. Only if it passes at the parent ref is "the join broke it" a supported
  claim.
- Location: join-replay failure handling in `driver-run`; the `package-failed` reason string.
- Trade-off: one extra criterion execution per failing parent criterion, in a path that is already
  running many; and it needs a materialization at the parent ref, which the driver can already
  produce.
- Confidence: high — the counter-test is cheap, deterministic, and was run by hand here.

### Add `issuance` to `package-join-conflicted`

- Addresses: "`package-join-conflicted` records carry no issuance"
- Change: include the issuance the conflict was recorded for, matching `worker-dispatched`.
- Location: journal event construction for `package-join-conflicted`.
- Trade-off: none identified; additive field.
- Confidence: high

## No-change decisions

- **The gate's ~12,500-line vendoring of two `build_adapter.py` copies.** Repository weight was
  decided by a gate repair rather than by a human, which is worth noticing. It is not a workflow
  defect: the repair was the correct response to the witness, and the alternative — reading files
  at a git revision — is the defect the witness caught. Flagged, not recommended against.
- **SB11's `FROZEN_FILE_SHA256` table.** It resembles the hardcoded fixture criticised above and
  is not the same thing: it pins which bytes constitute a frozen verifier, while the
  contributor-to-root mapping is still derived by execution. Verified against the repair diff.
- **The orchestrator's snapshot improvisation.** Copying campaign 1's evidence before campaign 2
  overwrote it is what made recovery possible, but making it a rule belongs with the digest
  recommendation above rather than as a separate instruction to copy things speculatively.

## Suggested follow-up

- The two `ORIENTATION_BASINS`-style fixtures in this repository encode one campaign's recording
  as an expectation. Auditing for that pattern across adapter verifiers is product work for the
  hfx repository, not a pce workflow change, but this run shows the pattern is load-bearing and
  fails silently until the data moves.
