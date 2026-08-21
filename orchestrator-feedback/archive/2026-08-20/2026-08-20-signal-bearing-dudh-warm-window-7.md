# PCE workflow feedback: a package whose deliverable is a live resource completes, but its deliverable does not persist

- Date: `2026-08-20`
- Orchestrator: `Claude Code / work-graph skill, session b2d7df0c`
- Run: `bluesmith planning/2026-07-29-signal-bearing-dudh-warm-window`, plan versions 11 to 13
- Outcome: `worked around inside the graph; the underlying asymmetry remains`

## Executive summary

Package completion is a durable claim about the past. Some packages deliver something that only exists
in the present — here, a running EC2 host. W7 provisioned and proved that host across nine criteria and
completed. Its recovery budget then read `dispatches_remaining: 0`, so it could never re-dispatch. When
the instance was later terminated, every dependent was permanently blocked: the graph recorded a
delivered host that no longer existed, and no package in it could produce another.

This cost two plan versions and roughly three hours. It is worth reporting because the workaround —
factoring the establishment chain into a fresh package — does not remove the asymmetry, it only moves
it one package along.

## Evidence reviewed

- `driver-journal.jsonl`, W5 `package-parked` at issuance 98 and at issuance 101
- `scripts/m9-reference-host.py` in the composed base, `main()` and `require_host()`
- `driver-status` recovery table for W7
- `graph.v11.json`, `graph.v12.json`

## Friction and failures

### A completed provisioning package cannot restore what it delivered

- Severity: `high`
- Phase: `execution`
- Observation: W5 parked with
  `replan: missing dependency: exactly one live W7 Linux/aarch64 reference host available to
  scripts/m9-reference-host.sh`.
- Evidence: the transport provisions only under an explicit flag —
  `scripts/m9-reference-host.py:823`:

  ```python
  record = ensure(authority, state_path) if args.ensure else require_host(authority, state_path)
  ```

  `require_host` raises `expected exactly one live reference host, found 0` at line 211. Across the
  frozen graph exactly one criterion passed `--ensure`: W7's first. W5's and W6's criteria all used
  `--sync --run-in`. W7's recovery state was `dispatches_remaining: 0, next_rung: replan`.
- Inference: completion carries across plan versions by package id; the resource does not. Nothing in
  the model relates the two, so a graph can hold a completion whose subject has ceased to exist.
- Impact: total blockage of two downstream packages, resolved only by authoring a new package and
  freezing a new plan version.

### The workaround relocates the problem rather than removing it

- Severity: `medium`
- Phase: `milestone planning`
- Observation: v11 added W8 carrying W7's establishment chain; v12 added W9 carrying it again.
- Evidence: W8 completed at issuance 100 and thereafter showed `state: complete`. Within an hour W5
  parked again for an unrelated reason, and had the host been torn down in that window, W8 would have
  been as unable to restore it as W7 was. The supervisor deliberately kept a paid instance running
  through the repair for exactly this reason, and recorded that decision.
- Inference: any package that both proves and provides a live resource inherits the asymmetry the
  moment it completes.
- Impact: a paid resource must be held running across deliberation, or the graph must grow another
  package each time it lapses.

## Recommendations

### Let a package declare that its completion is contingent on a live precondition

- Addresses: both findings
- Change: allow a criterion, or a package, to be marked as establishing a live resource, and re-run it
  when a dependent is about to dispatch rather than treating its past success as sufficient. The
  transport is already idempotent — `ensure` reports `provisioned` or `reused` — so re-execution is
  cheap when the resource is present and correct when it is not.
- Location: package completion and carry model
- Trade-off: reintroduces execution before dependents, which is what completion was meant to avoid;
  the cost is bounded by the idempotence of the check.
- Confidence: `medium` — the need is demonstrated, the right shape is a design question.

### Failing that, say so in the brief

- Addresses: finding 1
- Change: when a package's criteria create external state, state plainly in its brief that the state is
  not carried and that dependents must not assume it. The supervisor inferred this from a park; a
  worker cannot.
- Location: package brief generation
- Trade-off: none.
- Confidence: `high`

## No-change decisions

The supervisor briefly concluded that `--sync` implied `--ensure` and retracted the original finding on
that basis. That retraction was wrong and the code above is why. Recorded here because the retraction
delayed the correct diagnosis by several hours, and because the failure mode — talking oneself out of a
correct structural finding because the immediate symptom had another explanation — is worth naming.

## Suggested follow-up

Check whether other visions provision cloud resources through a completing package. If so they carry
the same latent blockage, and it will surface the first time a run is resumed after the resource lapses.
