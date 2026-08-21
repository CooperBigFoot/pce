# PCE workflow feedback: 2026-08-19-incidence-python-binding (third report)

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill, session 3a3b5e2d`
- Run: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-19-incidence-python-binding`, graph.v3.json, plan version 3
- Outcome: `blocked` (7/8 packages complete; IPB7 cannot be dispatched)

## Executive summary

Two findings, both about things the run cannot observe about itself.

First, the falsifiable missing-dependency fault worked exactly as designed and should be considered
settled: IPB7's park named the artifacts it checked and the command it used, and I confirmed the claim
in one command instead of the hour IPB5's bare `IPB4` cost me.

Second, a `herdr` upgrade landed mid-run and broke `pce`'s dispatch path. `herdr` was replaced at
12:40:18 and the driver relaunched at 12:43:31 against a `pce` built the previous evening. `herdr 0.8.2`
has a different `agent start` interface, so every future package dispatch fails at spawn. Nothing in the
workflow detected the skew before dispatch, and nothing in the journal identifies it as a toolchain
version problem rather than a package problem.

## Evidence reviewed

- `driver-journal.jsonl` records 158-161 (`plan-version-advanced`, `package-base-composed` IPB7,
  `worker-dispatched`, `worker-spawn-failed`)
- `package-outcomes/IPB7/11.json` (the plan-v2 park)
- `stat` and `--version` on `/Users/nicolaslazaro/.local/bin/herdr` and `/Users/nicolaslazaro/.local/bin/pce`
- `herdr agent start --help` on the installed 0.8.2
- `git log` in `/Users/nicolaslazaro/Desktop/work/pce`
- `pce package driver-status --graph graph.v3.json --journal driver-journal.jsonl`

## What worked

### Falsifiable missing-dependency faults

- Evidence: `package-outcomes/IPB7/11.json` carries
  `"id":{"missing":"IPB6 committed basin-scale sweep baseline record and workload generator",
  "checked":["bindings/python/benchmarks/sweep-baseline-v1.json","bindings/python/benchmarks/sweep_baseline.py"],
  "command":"git ls-tree -r --name-only HEAD -- ..."}`.
- Effect: I re-ran the worker's own command at its base ref, got an empty result, and confirmed the
  complaint in a single step. The first report asked for exactly this; it is delivered and the finding
  should be closed.

### Re-edging risk-ordering to buildability carried the predecessor's content

- Evidence: IPB7's base moved from `8eb551d9` with dependencies `[(IPB4, 7a515d2b)]` under plan v2 to
  `1162359a25e0716f645828d5f5db15e432aeda33` with `[(IPB4, 7a515d2b), (IPB6, af619290)]` under plan v3.
- Effect: the missing baseline record is now in IPB7's base, and all seven completed packages carried
  forward without re-execution.

## Friction and failures

### A risk-ordering edge sequences a package without supplying what its criterion must read

- Severity: `high`
- Phase: `graph authoring / base composition`
- Observation: IPB7's frozen criterion `Sweep cost is bounded - flatness half` requires the run be
  "compared against the measurement record IPB6 committed". IPB7 depended on IPB6 with kind
  `risk-ordering`, so IPB6's commit was not composed into IPB7's base, and the package parked.
- Evidence: the two `package-base-composed` records above; `git merge-base --is-ancestor af619290
  8eb551d9` returns false.
- Inference: risk-ordering expresses "decide after measuring" but composition treats it as ordering only.
  A criterion whose text names a predecessor's committed artifact therefore cannot be satisfied through
  a risk-ordering edge.
- Impact: one full park, one plan version, and one human freeze to convert an edge kind. The graph passed
  `pce graph check` at every version, including `--strict` warnings review, without flagging it.

### A breaking `herdr` upgrade during a run is invisible until dispatch fails

- Severity: `high`
- Phase: `execution / dispatch`
- Observation: `worker-spawn-failed` for IPB7 issuance 12 with
  `herdr command failed with exit status: 2 ... unknown option: --cwd`. The driver passes `--cwd`,
  `--workspace`, `--tab`, `--no-focus` to `herdr agent start`.
- Evidence: `herdr` mtime `2026-08-20T12:40:18`, `herdr --version` -> `0.8.2`; driver relaunched
  `12:43:31`; `pce` mtime `2026-08-19T19:50:44`. `herdr agent start --help` on 0.8.2 shows
  `herdr agent start <NAME> --kind <KIND> --pane <ID> [--timeout <MS>]` — none of the four options the
  driver sends. Every dispatch earlier in this journal succeeded against the previous herdr.
- Inference: `herdr 0.8.2` changed `agent start` from creating a workspace and running an argv to
  attaching a known agent kind to an existing pane. The installed `pce` predates that change.
- Impact: no package can be dispatched. The run is blocked with seven of eight packages complete and a
  correct base for the eighth. `/Users/nicolaslazaro/Desktop/work/pce` is at `5360580`, the same commit
  as the installed binary, so no fix exists to rebuild from.

### The spawn failure is not distinguishable from a package fault in the journal

- Severity: `medium`
- Phase: `execution / journal semantics`
- Observation: `worker-spawn-failed` carries the herdr argv and stderr, which is enough for a human to
  diagnose, but it is a package-scoped event. Nothing marks it as an environment or toolchain condition
  affecting every future dispatch rather than something about IPB7.
- Evidence: the record's shape — `package`, `issuance`, `reason` — with no environment classification.
- Inference: the event predates the case where a shared tool breaks for all packages at once.
- Impact: a supervisor could burn IPB7's remaining dispatches retrying a failure that will recur
  identically for every package. I did not retry, but only because I read the stderr and checked the
  binary mtimes.

## Recommendations

### Check the `herdr` interface once at driver start

- Addresses: "A breaking `herdr` upgrade during a run is invisible until dispatch fails"
- Change: at `driver-run` startup, probe the herdr version or the presence of the `agent start` options
  the driver depends on, and refuse to start with a clear message naming both versions if they do not
  match — before any plan advance or base composition.
- Location: `driver-run` startup, alongside the existing repository and graph validation.
- Trade-off: one extra subprocess per driver start, and a hard refusal where today there is a per-package
  failure.
- Confidence: `high`

### Classify a spawn failure as environment-scoped

- Addresses: "The spawn failure is not distinguishable from a package fault in the journal"
- Change: journal a failure of the dispatch mechanism itself (as opposed to the worker's own exit) as an
  environment-scoped event, or add a field marking it as affecting all packages, so the supervisor does
  not spend package recovery rungs on it.
- Location: the driver's spawn error path.
- Trade-off: another event kind or field for readers to handle.
- Confidence: `medium`

### Warn when a criterion names an artifact reachable only through a risk-ordering edge

- Addresses: "A risk-ordering edge sequences a package without supplying what its criterion must read"
- Change: extend the existing `artifact-provenance` analysis so that when a criterion's referenced
  artifact is produced by a package reachable only via a non-buildability edge, the warning says so
  explicitly rather than reporting the generic "a producer cannot be proven" message.
- Location: `pce graph check` authoring warnings.
- Trade-off: requires the check to reason about edge kinds during provenance analysis; still limited by
  the absence of output declarations.
- Confidence: `medium`

## No-change decisions

- **The human's original election to keep IPB6 -> IPB7 as risk-ordering.** The intent — measure before
  deciding whether IPB7 should exist — was sound and is preserved verbatim in the v3 edge reason. The
  defect was that the edge kind expressing that intent does not also deliver the measurement, which is
  recommended above as a check-time warning rather than a change to edge semantics.
- **Retrying IPB7 dispatch.** Its recovery ladder is untouched (`dispatches_remaining: 2`,
  `retry_remaining: 1`, `local_patch_remaining: 1`) and retrying would fail identically until the
  toolchain matches.

## Suggested follow-up

- Decide whether `pce` should pin a minimum `herdr` version in its own metadata, so an upgrade that
  removes an option `pce` depends on is caught at install time rather than at the next dispatch.
