# Brief: the driver must forward declared environment to worker workspaces

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-19.

## The defect, located exactly

A criterion command and a worker must see the same world. Today they do not: criteria run as
children of `pce package driver-run` and inherit its environment; workers are spawned by the
herdr daemon and inherit the daemon's environment instead. The env flag exists at every layer
except the hop that matters — `pce dispatch package` and `herdr workspace create` both accept
`--env`; `driver-run` forwards nothing, and the journaled dispatch argv shows no `--env`.

Proven on hfx (vision dir
`/Users/nicolaslazaro/Desktop/work/hfx/planning/2026-08-07-close-the-seven-basin-coverage-gap`,
supervision.md and park-evidence-SB3-v3.md): the human exported `HFX_CAMPAIGN_EVIDENCE` and
`HFX_S3_ENV_FILE` into the driver's environment. SB0's three criteria printed both paths and
passed — driver children see them. SB3's worker (issuance 13) failed with "HFX_CAMPAIGN_EVIDENCE
and HFX_S3_ENV_FILE are unset", and issuance 14 parked mis-specified on the same fact. An
admissible overrule was spent discovering that no true claim about the composed tree can
supply a missing environment variable. A paid, human-authorized campaign has been blocked
across two plan versions on this hop alone.

## What to build (decide the details yourself)

`driver-run` (and the gate/assembly dispatch paths, if they spawn through herdr the same way)
gains a way to forward named environment entries to every worker workspace it creates. Decide
the mechanism; candidates:

- `--worker-env NAME` (pass-through from the driver's own environment, repeatable) and/or
  `--worker-env NAME=VALUE` (explicit); pass-through is the safer default since the human
  already exports into the driver's launch shell;
- whether criteria execution should ALSO consume the same declaration rather than silently
  inheriting everything (recommend: leave criteria inheritance as is — it works — but state
  the asymmetry in --help so it stops being a trap);
- plumbing: driver → `pce dispatch package` `--env` → `herdr workspace create` `--env`, for
  package workers, gate workers, and any dispatch-continuation respawn — the forwarded set
  must survive a driver restart re-attaching to a live attempt.

**Journal doctrine (hard constraint):** environment VALUES must never enter the journal.
Record names only. Note that `dispatch-worker-identified` journals the observed argv today —
if the value form is used, the argv either must be redacted to names or the event must record
the names separately; decide and test it. A credential PATH is not itself a secret, but the
mechanism must be safe for values that are.

**Skill text:** `skills/work-graph/SKILL.md` section 2's run.json shape gains an optional
`"environment"` entry (decide the shape: list of names to pass through, or name→value map —
recommend names-only for the same journal-safety reason) and section 3's launch argv includes
the corresponding flags. The skill currently rejects unknown run.json fields; update shape,
validation prose, and `tests/skill_dispatch_review.rs` together.

**Decide the edge cases and record them:**
- a declared name unset in the driver's environment at launch: refuse at launch, naming it
  (the hfx failure mode must become a single pre-flight refusal, not six worker refusals);
- interaction with `--prepare` commands (they run in worker workspaces too — same forwarded
  set, presumably);
- whether the forwarded names are recorded in a typed journal event at driver start so a
  fold can state what environment contract a run was launched under (recommend: yes,
  names only).

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at 2fd2512 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  `./install.sh` from the MAIN checkout only. Do NOT install — live drivers (gridded-statics
  is mid-attempt); the supervisor coordinates installs at fleet-quiet boundaries.
- Key code: `driver-run` worker dispatch (`src/main.rs`, the argv builder that produced the
  journaled `pce dispatch package-worker … -- pce package agent …` vector),
  `pce dispatch` env handling, gate dispatch, `dispatch-worker-identified` argv capture,
  restart re-attach path from the dead-dispatch-closure change.
- The waiting consumer: hfx SB3 (paid Hetzner campaign, human-authorized, zero spend so far).
  After this lands and the supervisor installs at a quiet boundary, hfx relaunches with its
  existing v3 graph — SB0/SB1/SB2 completions carry, no plan version needed. State that
  explicitly in your report so the hfx orchestrator's next step is unambiguous.
