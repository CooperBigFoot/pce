# Brief: a gate repair must re-prove the criteria it can break

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-19.

## The defect, freshly proven

A package's criteria are judged against the worker's commit; a gate repair merges AFTER that
judgment (package-repair-merged) and is proven only by its own amendment criterion — the test
the finding named. Nothing ever re-executes the package's authored criteria against the
hardened oid. The repair can therefore break what the package just proved, and the breakage
surfaces arbitrarily later, attributed to whoever trips over it.

Proven on gridded-statics (vision dir
`/Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-09-gridded-statics-self-name-to-every-consumer`,
supervision.md 2026-08-19 sections): WP1's worker sealed a durable evidence baseline
(67,213 file hashes, roots recorded in CLI argument order), and WP1's criteria passed at
judgment with that worker's binary. WP1's gate then accepted a root-ordering finding and
repaired it with a two-line commit (`canonicalize sealed root ordering` — `canonical_roots.sort()`,
orthographos 83433297). The repair passed its amendment test and merged. Because seal
verification demands exact JSON equality of the recomputed snapshot, the post-repair binary
can never verify the pre-repair baseline: sorted root order ≠ sealed root order. The
incompatibility first EXECUTED two plan versions later, inside a different package (WP4),
burned a retry and a local-patch rung there, and cost a full supervisor forensic pass to
prove the evidence itself was intact. Had WP1's authored criteria been re-run against the
hardened oid at gate close, criterion "Evidence baseline is sealed" would have failed on the
spot, inside the gate that caused it.

## What to build (decide the details yourself)

After a gate finding's repair merges into the package lineage, the package's authored
criteria are re-executed against the hardened oid before the package's completion stands.
Decide the mechanism; constraints and candidates:

- **Where:** the natural seam is between package-repair-merged and the completion becoming
  final (or, if completion is already journaled before repairs merge — check the actual event
  order in the fold — a re-proof step whose failure retracts/blocks the completion). The
  existing join-criterion-executed machinery (conflicted-join re-runs, plan-carry
  re-verification) is precedent for re-running authored criteria outside first judgment;
  reuse its execution path rather than inventing a second one.
- **What runs:** the package's authored criteria plus all previously accepted amendment
  criteria, against the hardened oid's materialization. Criteria are re-runnable by doctrine
  (to-graph runtime doctrine: commands re-execute at package/join/assembly/carry).
- **Attribution (ADR 0022 — a package never pays for its judge):** a re-proof failure is the
  GATE's failure, not the package's. It must not consume the package's recovery ladder.
  Decide the concrete flow: reject the repair (finding returns to the gate with the failing
  criterion as evidence), spend against the gate-failure budget, and leave the package
  criteria-green at its pre-repair oid unless the finding is itself accepted-and-unrepaired —
  decide what happens then and record why.
- **Journal:** a typed event per re-proof execution (name it; gate-reproof-executed or
  similar) carrying criterion name, command, exit status — same shape discipline as
  criterion-executed. Old journals without the event must keep deriving.
- **Scope check:** assembly amendments and carried amendments execute as repaired/reverted
  pairs already (amendment portability); confirm the re-proof composes with that machinery
  instead of restating it, and state in the report whether assembly-time execution would have
  eventually caught the WP1 case (and why gate-close detection is still the right layer —
  the answer is in how late and how misattributed the failure lands).

**Decide the edge cases and record them:**
- multiple findings in one gate: re-proof once after the last repair merges, or per repair
  (recommend: once, after all repairs — cheaper, same guarantee);
- a repair whose re-proof fails only for environmental reasons: environment failures stay
  uncharged, as everywhere;
- re-proof duration: criteria like gridded's full-tree verifies run minutes — this is
  accepted cost (state it), not something to sample or skip.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at 2fd2512 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  `./install.sh` from the MAIN checkout only. Do NOT install — live drivers; the supervisor
  coordinates installs at fleet-quiet boundaries.
- PARALLEL WORK IN FLIGHT: criterion-revision (freeze/advance path) and worker-environment
  (dispatch env path) are being implemented concurrently. This brief's territory is the gate
  finish / repair merge / completion seam — do not touch `run_graph_freeze`, the invariance
  check, or dispatch env plumbing; if your change collides there, stop and report rather than
  merging over them.
- Key code: gate lifecycle in `crates/core/src/package_driver.rs` and `src/main.rs`
  (`gate-finished`, `package-repair-merged`, `package-completed` emission order,
  `finding-replayed`), gate-failure budget from ADR 0022 (`--gate-failure-limit`,
  gate-blocked state), join-criterion-executed execution path, amendment portability
  (repaired/reverted pair execution), fold `derive_driver_snapshot`.
- Evidence: gridded supervision.md (2026-08-18/19 WP4 sections: the failure, the 67,213-file
  intact-evidence proof, the two-line repair diff); journal events gate-finished
  package-gate-8, package-repair-merged WP1 (83433297), WP4 criterion-executed failures.
- Tests: gate lifecycle coverage near the driver tests; add one where an accepted repair
  breaks an authored criterion and the re-proof rejects it without charging the package.
