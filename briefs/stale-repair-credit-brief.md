# Brief: a repair is proven by its criterion, never by its bytes

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-19.

## The principle both defects violate

Two independent runs aborted the same day because repair accounting treats REMEMBERED BYTES
(an oid, a hunk) as the proof of an amendment, when the system already possesses the real
proof: the amendment CRITERION executing green against the tree in question. In both aborts
the passing criterion sits in the journal one record before the fatal error. Attribution and
crediting must accept criterion-proof and degrade byte-bookkeeping mismatches to visible,
recoverable events. Both aborts also share the second half: the driver exits nonzero with NO
typed journal event, leaving driver-status deriving running/judging for a dead process.

## Defect A (bluesmith): stale credit after a rebuilt lineage — reproduced twice

Full evidence: `orchestrator-feedback/2026-08-19-signal-bearing-dudh-warm-window-2.md`
(exact oids, journal state, both stderr captures) and the bluesmith vision dir
`/Users/nicolaslazaro/Desktop/work/bluesmith/planning/2026-07-29-signal-bearing-dudh-warm-window`
(journal at 262 events, graph.v4.json, graph.v5.json, the v4 revision record).

The legal sequence that reaches it: a gate under plan v4 (`package-gate-20-1`) merged repairs
into BOTH of a two-repository package's repos. Plan v5 revised the package (W7), which
forfeited its completion and re-composed from the authored refs into a new lineage. The
amendment carried (amendments are criteria; they carry); the repair COMMITS did not (they
belong to the forfeited attempt's lineage — one of them, 7c56462d…, exists only in the OTHER
repository, stopwatch). After the next gate on the recomposed lineage finishes, the driver
tries to credit the remembered repair and exits 1:

    Error: credited repair 7c56462d… for package-gate-20-1 finding 0
    is not a fast-forward of package W7 lineage b4e77741…

Deterministic: two driver processes, same abort, after package-gate-26-1 and -26-2. Both of
those gates were themselves healthy (accepted finding, witness exit 1, repair exit 0), and
the carried amendment criterion PASSED at issuance 26 on its own merits — the worker
recreated the required test in the fresh lineage. The remembered repair oid is stale
bookkeeping, not proof, once the lineage was rebuilt.

Two halves, the second worse: (1) the credit step is fatal for a package-attributable-to-
nobody reason — no recovery verb, overrule, or graph revision reaches it, and W7's budget is
rightly untouched; (2) the abort appends NO typed event — the journal's last entry is an
ordinary gate-finished, driver-status derives outcome:running / W7:judging with no process
alive, and the only explanation lives in terminal scrollback. The run proof cannot explain
why the run stopped, and it actively misleads a from-disk resume.

## Defect B (RivRetrieve): assembly attribution demands byte-exact hunk survival

Full evidence: `orchestrator-feedback/2026-08-19-a-fixture-is-a-recording.md` (exact oids,
the AST-identical diff, the reproduction reasoning) and the vision dir
`/Users/nicolaslazaro/Desktop/work/RivRetrieve/planning/2026-08-19-a-fixture-is-a-recording`.

The v2 driver aborted mid-assembly with
`Error: could not attribute unconstructable repair 7a34e785… for REC3` — immediately AFTER
all nine assembly criteria, including REC3's `probe_that_does_not_replay` amendment (the very
repair being attributed), executed and passed against the composed tree. All six
witness/repair oids are reachable ancestors of the resolved base; the trigger is a CONTENT
change to the repair's hunk in `boundary_probes.py`: the conflict-resolution worker ran the
repository's own `ruff format`, which collapsed a 5-line expression to 1 line (103 chars,
line-length 120 — the config mandates it; AST-identical, parsed and compared). No package
commit touched the file. Attribution looked for the fix character-for-character, missed, and
killed the run one step short of assembly-completed. Re-running reproduces it exactly: same
merge, same reflow, same abort. Five packages complete, 1616 tests passed, promotion closed
by the abort alone.

Secondary finding from the same run (fix it here, it is small): the conflict-resolution
worker reformatted a file OUTSIDE its four assigned conflicted paths. Scope the resolution
worker's brief/doctrine to its conflicted paths — that narrows the blast radius of any
formatter but does NOT replace the attribution fix; the byte-check would still be brittle.

## What to build (decide the details yourself)

1. **A driver abort is a journal event.** Any non-zero exit path out of driver-run that today
   returns an `Error:` up through main must first append a typed event (e.g. driver-aborted
   {reason}) carrying the same diagnostic. Decide how the fold and driver-status read it
   (blocked seems right; running-with-no-process is the current lie). Old journals without
   the event keep deriving. Cover the write-failure case (journal unwritable → still exit
   nonzero, best-effort stderr).
2. **A stale credit degrades, never kills.** When a carried amendment's credited repair oid
   is not reachable from the package's current lineage, do not abort. Decide the disposition —
   the feedback's recommendation (treat the amendment as unsatisfied-and-recoverable: the
   amendment criterion must pass on its own merits, which is precisely what already happened
   at issuance 26) matches how amendments are supposed to work; the stale oid is dropped from
   crediting with a typed event recording the drop (repair-credit-stale or similar), so the
   drop is visible in the proof. The multi-repository case is the proven one: the stale oid
   may exist only in a repository the current lineage never contained.
3. **Make the forfeit rule explicit.** When a revised package forfeits its completion:
   amendments carry as criteria, repairs do not. State it in one place in the carry logic and
   in the skill texts that describe what carries (`skills/work-graph/SKILL.md`,
   `skills/to-graph/SKILL.md` if it speaks of amendments), so the next reader doesn't
   rediscover it from an abort. If you conclude a different rule is better (carry both /
   carry neither), that is a doctrine change — stop and report per the boundary.
4. **Check the feedback's follow-up question** and answer it in your report: can the same
   condition arise WITHOUT a plan revision (any other path that rebuilds a package lineage
   while an amendment credit is outstanding — conflicted joins, gate-reproof rollbacks)? If
   yes, add coverage for that path too.

**Reconcile, don't collide:** the gate-repair-reproof change (integration d3a7c26/fba9489)
touched the repair/amendment seam — repairs roll back on failed re-proof and amendments
retract in derived state. Your fix must compose with it: a rolled-back repair must not leave
a credit that later goes stale invisibly. Criterion-revision (f03f205) owns the forfeit
mechanics your rule in (3) describes — read its CONTEXT.md notes before writing new carry
logic.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at 9d3d7a1 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main.
  Do NOT install and do NOT run `cargo build --release` in the MAIN checkout —
  ~/.local/bin/pce symlinks to its target/release and a build IS an install; build only in
  pce-integration. The supervisor coordinates installs at fleet-quiet boundaries (a driver is
  live on gridded-statics).
- Key code: the repair-crediting path emitting "credited repair … is not a fast-forward of
  package … lineage" (src/main.rs / package_driver.rs), plan-advance carry
  (`ensure_driver_plan_version`, unchanged_package_ids, amendment carry from the
  amendment-portability change), driver-run's error/exit path, `derive_driver_snapshot`,
  driver-status outcome derivation.
- The waiting consumer: bluesmith is blocked at plan v5 with W7 judging-forever; W1–W3
  complete and carried; W7's work done and provably passing at issuance 26; spend closed
  (~USD 0.33, both hosts verified terminated). After this lands and the supervisor installs,
  the orchestrator relaunches on v5 — state in your report what the first driver pass should
  do with the recorded stale credit so the orchestrator knows what healthy looks like.
- Tests: driver/gate lifecycle coverage near the code above. Add: (a) an abort path appends
  the typed event; (b) a revised-and-recomposed package with a carried multi-repo amendment
  completes when the amendment criterion passes, with the stale credit dropped and journaled;
  (c) driver-status on a journal ending in driver-aborted reads blocked, not running.
