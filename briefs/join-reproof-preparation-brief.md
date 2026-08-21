# Brief: the join re-proof must prepare its materialization and never charge the environment

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-19.

## The defect, live right now

Vision dir `/Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-09-gridded-statics-self-name-to-every-consumer`
(journal, supervisor log 2026-08-19 evening). After WP3's conflicted-join resolution
(WP2-vs-WP4 conflict in orthographos, ADR 0015 path), the driver re-ran the parent criteria
in a join materialization (`driver-materializations/70759-join-WP3-…`) and every criterion
needing the prepared environment failed environmentally:

- `The manifest-only re-issue guards are covered` and `gate:package-gate-8:finding:0`:
  exit 101, "error: failed to load manifest for workspace member …" — the orthographos cargo
  workspace cannot load because the configured prepare (`ln -sfn …/hdx ../hdx && … &&
  cargo build --release --workspace`) never ran;
- `Frozen evidence command succeeds`: exit 127, "./target/release/orthographos: No such file
  or directory" — no build ever happened.

The journal shows a run of nine `join-criterion-executed` events with NO
`environment-preparation-executed` anywhere between worker-done and package-failed — while
every ordinary criteria materialization (e.g. `70637-criteria-…`) emits preparation events
first and passed the same commands. The driver then failed the package:
`package-failed WP3 "conflicted join broke parent criteria: WP2:Raster contents unchanged, …"`
and charged WP3's recovery ladder (recovery-rung-attempted followed; issuance 20 is burning
the retry on the identical wall as this is written, and will reach park).

Double misattribution: (1) an environment failure reported as a semantic break of parent
criteria; (2) charged to the dependent package's ladder — the ADR 0022 family, "a package
never pays for its judge," here "a package never pays for the driver's unprepared bench."

## What to build (decide the details yourself)

1. **Prepare the join materialization exactly like a criteria materialization**: same
   configured prepare commands per repository, same `environment-preparation-executed`
   events, before any join-criterion-executed. Reuse the criteria path's preparation code;
   do not fork a second implementation.
2. **Attribution**: a join re-proof failure that is environmental (preparation failed, tool
   missing) is uncharged, exactly like worker environment failures — decide how the existing
   environment/criteria distinction applies and record it. A GENUINE semantic break of parent
   criteria on a prepared bench keeps today's behavior (that is the ADR 0015 guarantee and it
   is right).
3. **Bonus, small, same seam** (found while diagnosing; fold in if cheap, else note it):
   negative-form criteria like `if <tool> …; then exit 1; fi && test …` FALSE-PASS when the
   tool is missing (exit 127 skips the then-branch) — the journal shows `Verify-only never
   seals` exit 0 with stderr "orthographos: No such file or directory". That's an authoring
   hazard, not a driver bug; the driver fix that makes benches prepared removes the trigger,
   but add a line to `skills/to-graph/SKILL.md` authoring doctrine: a negative criterion must
   first assert its tool exists (e.g. `test -x <tool> && if …`).

Guard test: a conflicted-join resolution over a graph whose criteria require a prepare-built
tool re-proves green after preparation; and with a prepare command that fails, the join
re-proof records an uncharged environment failure, not a package failure.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at e647d1d or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  push origin. Do NOT install and do NOT `cargo build --release` in the MAIN checkout (the
  installed binary symlinks to its target/release); build only in pce-integration. The
  supervisor coordinates installs — the affected driver is LIVE mid-ladder.
- Key code: the conflicted-join re-proof path (join-criterion-executed emission,
  package_driver.rs / src/main.rs), the criteria materialization preparation path
  (environment-preparation-executed), recovery charging, ADR 0015/0022 lineage in docs/adr.
- Known flake: `tests/dispatch.rs gate_execution_echoes_large_input_without_deadlock` fails
  occasionally under full parallel load, passes in isolation — not yours.
- The waiting consumer: gridded-statics WP3 — WP1/WP2/WP4/WP5 complete, WP3 is the last
  package and will likely be parked with an exhausted ladder by the time this lands. The
  supervisor holds a mechanical v5 bump ready (now agent-runnable) to reset the ladder after
  install; state in your report what a healthy join re-proof pass looks like so the
  supervisor can verify it.
