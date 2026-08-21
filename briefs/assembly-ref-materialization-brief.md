# Brief: the driver must materialize every ref its journal proves

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-19.

## The gap, proven on the first cold resume

Context: d6682bf's stale-repair-credit fix added cold-resume handling for aborted assembly
journals. It works — RivRetrieve
(`/Users/nicolaslazaro/Desktop/work/RivRetrieve/planning/2026-08-19-a-fixture-is-a-recording`)
resumed at the exact abort point, appended repair-credit-stale (reason
"counterfactual-unconstructable"), ran REC4/REC5, appended assembly-completed; driver-status
Finished; 12/12 assembly criteria exit 0; the ancestry check passes (b9e8a3e0 is a direct
parent of assembly 7056e1e4).

But promotion (work-graph section 8) then refuses, correctly, because
`pce/2026-08-19-a-fixture-is-a-recording/assembly-v2` does not exist:
`git rev-parse --verify …/assembly-v2^{commit}` → fatal. The only ref pointing at 7056e1e4
is a manual retention tag the orchestrator created when the commit was found DANGLING
(pce-retained/…/assembly-v2-resolution). `refs/pce-assembly-resolutions/*` is empty — the
new anchoring applies only to resolutions the fixed binary CREATES; the resume path adopts a
pre-fix resolution commit without anchoring it or creating the assembly branch, both of which
live on the normal assembly path the cold resume skips.

Section 8 rightly forbids the skill/orchestrator from reconstructing refs — a hand-made ref
asserts something the driver never certified. So the certified oid sits in the journal with
no driver-created name, and promotion is closed on a Finished, fully proven run.

## What to build (decide the details yourself)

The principle: any oid the journal proves as a terminal product — the assembly oid, retained
package-attempt inputs, resolution commits — must exist under a driver-created ref the moment
the proving event lands, on EVERY path that can land it (normal, conflicted-join, cold
resume). Candidates; decide:

- fold ref materialization into the cold-resume path itself so it exactly matches the normal
  path's refs (assembly branch `pce/<vision>/assembly-v<plan_version>` at the proven oid,
  resolution anchor under refs/pce-assembly-resolutions/*, attempt branches if any are
  missing); and/or
- a read-only-journal, write-refs-only verb (e.g. `pce package materialize-refs --graph …
  --journal …`) that derives the terminal snapshot and creates exactly the refs the journal
  proves, refusing when a ref exists at a DIFFERENT oid (never move, never force). This also
  heals any journal already in RivRetrieve's state without re-running anything — decide
  whether the verb is the implementation and the resume path calls it.
- Idempotent: re-running materialization on a healthy run changes nothing and says so.
- Journal a typed event for each ref created this way (the proof should show the name came
  from the driver, answering exactly the objection that stopped the orchestrator).
- The skill text (section 8's "missing ref is terminal") stays as strict as it is for
  hand-made refs; add only that the supervisor may run the materialization verb (if you build
  it) before the section-8 resolution — driver-certified creation, not reconstruction.

Guard test: RivRetrieve's exact shape — journal Finished via cold resume, assembly oid
reachable only via an out-of-band tag — materializes the branch + anchor, then section 8's
checks pass; and a ref existing at a wrong oid refuses without moving it.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at e4517c8 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main.
  Do NOT install and do NOT `cargo build --release` in the MAIN checkout — the installed
  binary is a symlink to its target/release; build only in pce-integration. Supervisor
  coordinates installs.
- Key code: the cold-resume path added in d6682bf (src/main.rs), the normal assembly path's
  branch creation and refs/pce-assembly-resolutions anchoring, gate-ref anchoring under
  refs/pce-gate/ as prior art, `derive_driver_snapshot` for the terminal snapshot,
  `skills/work-graph/SKILL.md` section 8.
- The waiting consumer: RivRetrieve, Finished at plan v2, assembly 7056e1e4 held by tag
  pce-retained/2026-08-19-a-fixture-is-a-recording/assembly-v2-resolution, nothing pushed,
  no promotion-state.json (a later legitimate promotion is not barred). State in your report
  the exact command sequence its orchestrator runs after install to materialize and promote.
