# Brief: a worker must never end its turn with its act still in flight

Status: READY TO DISPATCH — no grill. The decisions below are evidence-based engineering
choices, and the evidence is complete; the human has delegated them. Examine the evidence,
make the best decision on each question, implement it, and record what you decided and why
in your completion report (and in CONTEXT.md where a decision names a durable term). The one
boundary: if any decision turns out to require changing ratified doctrine (an existing ADR
or a criterion), stop and report instead of deciding it. Written 2026-08-18 while the
incident was still live: two consecutive WP2 workers on the gridded-statics run orphaned the
same irreversible act the same way.

## The incident

WP2 (gridded-statics v2) mints `camels-four-quadrant@v3`: a one-shot ~19 GB copy-on-write
re-issue. Attempts 9 and 10 both did real committed work, then exited 0 without writing the
outcome file — scored as two identical environment failures. The session transcripts
(~/.prime/agent/sessions/) show why. Attempt 10's final message, verbatim:

> The one-shot v3 reissue is now running in the background:
> - PID: `31432`
> - Log: `et20-reissue-evidence/attempt-10-execute.log`
> I have not written the package outcome yet. It depends on execution and attestation
> completing.

The worker launched the mint with `bash ... &`-style backgrounding, then **ended its turn to
"wait"**. A headless `-p` agent has no next turn: ending the turn is exiting. The
dispatch-worker observed exit 0 with the required artifact absent, recorded an uncharged
environment failure, and dispatched attempt 11 — while attempt 10's mint kept writing the
irreversible destination in the background. Attempt 9 had done the same thing one hour
earlier.

Two distinct harms:

1. **The act is orphaned.** Nothing in the run's accounting knows the mint exists. Its
   completion report lands in the shared evidence directory only if the bash wrapper
   survives; its success is invisible to the journal.
2. **The successor races the predecessor.** The resume-by-verification license (authored to
   make one-shot acts recoverable) says: attest an existing destination; only after
   attestation *fails* may the destination be removed and re-minted. A half-written
   destination fails attestation by definition — so the license, read while the predecessor's
   mint is live, authorizes deleting a tree that is actively being written. The supervisor
   happened to be watching; nothing structural prevented it.

## Why the worker did it — name the tension honestly

The mint takes over an hour. The worker had (correctly) absorbed that long turns are risky —
this same machine spent the previous night on agents dying at the ~1-hour mark — and chose
backgrounding as self-defense. The eviction bug is fixed (prime-agent 0.7.3), but the
instinct will recur wherever an act outlasts an agent's comfort. Doctrine that just says
"don't background" without answering "then how do I safely run a 90-minute command?" will be
rationalized away by the next worker in the same corner.

## Decisions delegated to you — decide from the evidence and implement

- **The doctrine sentence and where it lives.** The worker's operating text is composed in
  `crates/core/src/package_worker.rs` (the "Required outcome" section, ~line 266, is what
  every worker reads). Candidate: "Run your acts to completion in the foreground and write
  the outcome before exiting. Ending your turn is exiting. A process you background is
  orphaned the moment you stop — never leave one running." Does it also belong in the gate
  brief text (`crates/core/src/package_gate.rs`)?
- **The sanctioned way to run a long act.** Foreground-and-wait inside one tool call (the
  agent's runtime supports long-yield execution — attempt 10's own `ipython` calls used
  `yield_time_ms`)? A worker-side wait loop polling the child? State the pattern in the brief
  text so the safe path is as easy as the unsafe one.
- **Mechanical backstop, or prose only?** The dispatch-worker could, at child exit, inspect
  the worker's process group for surviving children and record their existence in the result
  (a `surviving_processes` field) — turning "exit 0, artifact absent, children alive" into a
  distinguishable state instead of a generic environment failure. That is observation, not
  enforcement (ADR-0012 constrains liveness enforcement); does it pull its weight?
- **Tighten the resume license against the race.** The license text (authored per-package at
  graph time; current instance in WP2's "Raster contents unchanged" criterion input) needs
  one more clause: before treating a failed attestation as permission to remove, verify no
  predecessor process is still writing (the completion report and status file the act
  produces are the natural evidence; absence of a fresh status plus a running process means
  WAIT, not remove). This clause also belongs in `/to-graph`'s runtime-doctrine section
  (skills/to-graph/SKILL.md, "Author for the driver's failure model") so future graphs get it
  at authoring time.
- **Retroactive:** the live run needs no repair — the mint either completes (its status file
  and completion report let the next attempt attest honestly) or fails visibly. State that
  the two environment failures stand as correct history.

## Environment facts a dispatched agent will need

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at 77d5bb6 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  `./install.sh` from the MAIN checkout only; the human times installs (a driver may be
  live).
- Key text to edit: `crates/core/src/package_worker.rs` (worker brief composition — the
  "Required outcome" block near :266 and whatever section fits the long-act pattern);
  possibly `crates/core/src/package_gate.rs` (gate brief); `skills/to-graph/SKILL.md`
  ("Author for the driver's failure model" section, added 2026-08-17).
- Mechanical-backstop code, if grilled in: the dispatch-worker result write in `src/main.rs`
  (`run_package_worker` ~:4024, `PackageWorkerResult`), where exit status and artifact
  presence are already observed.
- Evidence: gridded-statics vision dir supervision.md (2026-08-18 entries), session
  transcripts `01a014cd-*` (attempt 10) and its predecessor in
  `~/.prime/agent/sessions/`, results `.pce/package-results/WP2/{25,27}.json` (exit 0,
  artifact absent), the mint wrapper visible in `ps` output for pid 31432 (a bash -c with
  status-file bookkeeping — the worker built decent scaffolding around the wrong lifecycle).
- Tests: `tests/package_worker.rs` and `tests/skill_dispatch_review.rs` assert on brief
  text; expect to update them alongside the prose.
