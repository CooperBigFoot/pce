# PCE workflow feedback: 2026-08-20-taqsim-modelling-layer-on-incidence

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `planning/2026-08-20-taqsim-modelling-layer-on-incidence` in `taqsim`, graph `graph.v1.json`, journal `driver-journal.jsonl`
- Outcome: `blocked, then unblocked by a human ruling and relaunched` (in flight at time of writing)

## Executive summary

A six-package, two-repository graph (`incidence`, `taqsim`). `IN1` completed cleanly, including a
gate that caught a real serialization defect the authored criteria could not reach. The run then
blocked immediately: `TQ1` parked with `missing-dependency`, because `IN1`'s output was not
addressable from a `TQ1` worker's workspace.

The highest-impact finding is structural. A `depends_on` edge of `kind: buildability` that crosses a
repository boundary **orders** the two packages but **delivers nothing**. `IN1` produced exactly what
`TQ1` needed, in the same session, on the same machine, and `TQ1` could not reach it. The graph was
authored correctly and the driver honoured it correctly; the gap is that a cross-repository edge has
no delivery semantics at all. Resolving it required a human ruling, an external push, and spending
`TQ1`'s one-shot overrule allowance on what was an orchestration gap rather than a worker error.

A second, independent finding: `criterion-executed` records the criterion's `working_directory` but
not the oid of the tree it ran against. Because driver materializations are deleted promptly, a
criterion's proof becomes unanchorable shortly after it is written.

## Evidence reviewed

- `planning/2026-08-20-taqsim-modelling-layer-on-incidence/graph.v1.json` (frozen, `pce graph check` valid, plan version 1)
- `planning/2026-08-20-taqsim-modelling-layer-on-incidence/driver-journal.jsonl` (through `package-parked TQ1` and the subsequent overrule)
- `planning/2026-08-20-taqsim-modelling-layer-on-incidence/package-outcomes/TQ1/2.json`
- `planning/2026-08-20-taqsim-modelling-layer-on-incidence/supervision.md` and `supervision-state.json`
- `planning/2026-08-20-taqsim-modelling-layer-on-incidence/.pce/supervision-evidence/TQ1-2/` (pane transcript, `herdr workspace get`, `herdr worktree list`, git observations)
- `pce package driver-status` output before and after the overrule
- `git` observations in `/Users/nicolaslazaro/Desktop/work/incidence` and `/Users/nicolaslazaro/Desktop/work/taqsim`

## What worked

### The package gate caught a defect the authored criteria structurally could not

- Evidence: `finding-replayed` `IN1` / `package-gate-1-1` / finding 0, command
  `cargo test -p incidence-core --test power_operation power_operation_preserves_v2_numerical_semantics_when_roundtripped`;
  witness ref `171d483` exit 101 with
  `assertion left == right failed / left: String("v1") / right: "v2"`; repair ref `3eef8c8`.
- Effect: the authored criterion `Power operation computes exactly` passed at exit 0 both before and
  after the fix, so it could never have surfaced this. A power expression roundtripped through the
  model document was losing its `v2` numerical-semantics stamp. The gate converted that into a
  carried amendment criterion. This is the single clearest value delivered by the run so far.

### Workspace cleanup was exact and self-describing

- Evidence: two `dispatch-pane-cleanup` records for `IN1`/1, workspaces `w159` (worker) and `w15E`
  (gate), both `outcome: closed` with detail "herdr confirmed closure of the exact workspace
  containing the run-created pane".
- Effect: no orphaned workspace to reason about, and the record states the identity basis for the
  closure rather than leaving the supervisor to infer it.

### The park carried a machine-checkable complaint, not prose

- Evidence: `package-outcomes/TQ1/2.json` `fault.kind = missing-dependency` with a `checked` array of
  four items and a `command` field containing the exact probe the worker ran.
- Effect: the supervisor could test the complaint mechanically by re-running the worker's own
  command, rather than paraphrasing it. When the complaint later became false, the same command
  became the overrule's evidence. Typed faults with a reproducible probe are what made a defensible
  overrule possible; a prose complaint would not have.

## Friction and failures

### Cross-repository `buildability` edges order work but deliver nothing

- Severity: `high`
- Phase: `execution / dependency composition`
- Observation: `TQ1` (repository `taqsim`) declares
  `depends_on: [{id: IN1, kind: buildability, reason: "the basin's build path imports
  incidence.compile_model and reads incidence.PresenceSeries, and taqsim cannot declare incidence as
  a dependency at all until the binding stops declaring itself a non-package"}]`. `IN1` completed
  with hardened oid `3eef8c8`. `TQ1` was then dispatched into a worktree containing only `taqsim` and
  parked with `missing-dependency`, having probed `../incidence/bindings/python` and
  `https://github.com/CooperBigFoot/incidence.git commit 3eef8c8…` and found neither reachable.
- Evidence:
  - `package-base-composed` `TQ1` / `taqsim` / base `ceb7dc1…` / `dependencies: []` — the edge
    contributed nothing to the base, correctly, since the dependency is in another repository.
  - `herdr workspace get w15H` → checkout
    `/tmp/pce-work-package-worktrees/pce-4ffe3157f41cd321b025458bce85/00-taqsim`, a linked worktree
    of `taqsim` only. Relative to that path, `../incidence` does not exist.
  - `git -C .../incidence branch -r --contains 3eef8c8…` → empty; `git log --oneline -1 origin/main`
    → `90398c8`. Every `IN1` artifact was local-only.
  - `git -C <TQ1 worktree> log ceb7dc1..HEAD` and `git diff --stat` both empty; the worker wrote
    nothing and refused.
- Inference (distinguished from the above): the edge appears to be implemented as an ordering
  constraint only. Same-repository edges get delivery via base composition; cross-repository edges
  get ordering and no delivery mechanism. I did not read the driver source, so this is inferred from
  observed behaviour, not confirmed in code.
- Impact: five of six packages blocked on one edge. `TQ2` depends on `IN1` identically and would have
  parked the same way; `TQ3`–`TQ5` sit behind `TQ1`/`TQ2`. Resolution required a human ruling, a push
  to a public remote, and consumption of `TQ1`'s one-shot overrule. The overrule allowance is
  designed to absorb a mistaken worker complaint; here the worker was factually correct and the
  allowance was spent covering an orchestration gap. `TQ1` now has no overrule left for a genuine
  future misjudgement.

### A completed package's output has no addressable handle for a downstream package

- Severity: `high`
- Phase: `execution / handoff`
- Observation: `IN1` finished with its result reachable only as local git objects and local branches
  in the source worktree. The `TQ1` worker, needing that result, guessed at two addresses — a sibling
  directory path and the public GitHub remote at the exact oid — and both failed.
- Evidence: the `checked` array in `package-outcomes/TQ1/2.json` lists
  `"../incidence/bindings/python"` and
  `"https://github.com/CooperBigFoot/incidence.git commit 3eef8c8877dd98e464d640bee05a894a3855767c"`.
  The worker knew the correct oid, which means its brief conveyed the identity of the dependency's
  output but not a way to reach it.
- Inference: the brief exposes the upstream oid without exposing a resolvable location for it. The
  worker's guesses were reasonable, and one of them (the remote at that oid) became correct the
  moment a human pushed the branch — which is evidence that the address was right and only the
  publication step was missing.
- Impact: a worker spent an attempt discovering an unreachability that the orchestrator already knew
  about, and the run stopped for a human.

### `criterion-executed` does not anchor its proof to a ref

- Severity: `medium`
- Phase: `execution / proof recording`
- Observation: `criterion-executed` records `execution.working_directory`, `command`, `exit_status`,
  `stdout`, `stderr` — but no oid for the tree the criterion ran against. Materialization directories
  are removed promptly after use.
- Evidence: `IN1`'s criteria ran in
  `driver-materializations/22206-criteria-1787253357405320000/00-repository`. While inspecting that
  directory I captured `git rev-parse HEAD` → `1c52ace…`, and a follow-up `git -C <same path>`
  seconds later returned
  `fatal: cannot change to '.../00-repository': No such file or directory`. Had I not captured the
  oid inside that window, the journal alone would not have told me which tree produced the proof.
- Inference: the working directory is treated as the record of *where* a criterion ran, in a scheme
  where that location is guaranteed not to survive. The oid is the durable identity and is absent.
- Impact: post-hoc verification of a criterion's proof depends on winning a race. This also bears on
  promotion: section 9 of the `work-graph` skill must correlate final package proofs, and it has to
  do so through composition events rather than through the criterion records themselves.

### Criterion output is not summarized, and a truncated read is actively misleading

- Severity: `low`
- Phase: `execution / proof recording`
- Observation: `cargo test --workspace <filter>` emits one `test result:` line per workspace test
  binary. For `IN1`'s first criterion that is 41 lines, of which 40 read `0 passed; … filtered out`
  and exactly one reads `2 passed`. Any prefix read of that stdout shows only zero-count lines.
- Evidence: scanning the full record gives
  `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`, one occurrence out of
  41 `test result` lines. `git grep power_operation 1c52ace…` confirms two matching test functions in
  `crates/core/tests/power_operation.rs`.
- Inference: I formed and then refuted a theory that the criterion was passing vacuously. The
  refutation cost two extra commands and would have cost the human an unnecessary criterion ruling
  had I reported the theory first. The failure mode is generic to workspace-wide test filters, not
  specific to this project.
- Impact: low here because it was caught before escalation, but the same shape would produce a false
  escalation from a less careful reader, or a false sense of proof if the single non-zero line were
  the one absent.

### `pce-protect-criteria` blocks read-only inspection of `vision.md`

- Severity: `low`
- Phase: `supervision`
- Observation: `grep -niE 'incidence' <vision-dir>/vision.md` was refused with
  "REFUSED: Bash may not access vision.md during an active run; use Read for inspection".
- Evidence: `PreToolUse:Bash` hook error from `$HOME/.local/bin/pce-protect-criteria`.
- Inference: the hook guards against modification but matches on any Bash access, including
  unambiguously read-only ones.
- Impact: minimal — the refusal names the correct alternative, and I used it. Recorded because the
  guard is doing real work and the message is good; only the breadth of the match is worth noting.

## Recommendations

### Give cross-repository dependency edges an explicit delivery contract

- Addresses: "Cross-repository `buildability` edges order work but deliver nothing" and "A completed
  package's output has no addressable handle for a downstream package".
- Change: when a package's `depends_on` names a package in a different repository, the driver should
  materialize that dependency's completed oid somewhere the dependent worker can reach, and state the
  location in the brief. The smallest version that would have prevented this park: check out the
  dependency's completed oid into a sibling directory of the dependent's worktree (the workspace
  already contains `00-<repo>`; a `01-<dependency-repo>` alongside it would have satisfied the
  worker's very first probe, `../incidence/bindings/python`), and record a
  `dependency-materialized` event naming package, repository, oid, and path.
- Location: driver dependency-composition path, wherever `package-base-composed` computes
  `dependencies` for a package; and the package brief template that currently conveys the upstream
  oid without an address.
- Trade-off: extra checkout cost and disk per cross-repository edge, and a decision about whether the
  dependency checkout is writable (it should not be — the dependent must not mutate another
  package's proven output).
- Confidence: `high` that the gap is real and blocking; `medium` on the sibling-checkout shape being
  the best fix, since a path-source checkout and a pushed-ref pin solve different downstream needs. A
  `pyproject.toml` pinned to a local absolute path is not committable, which argues that the delivery
  mechanism and the *committed* dependency declaration are two separate problems and only the first
  belongs to the driver.

### Refuse a cross-repository edge at freeze time if delivery is not implemented

- Addresses: the same findings, as a cheaper stopgap.
- Change: if the driver cannot deliver across repositories, `pce graph check` should warn — and
  `pce graph freeze` should require acknowledgement — when a `depends_on` edge crosses a repository
  boundary. The failure currently surfaces after a full package execution and a worker dispatch; it
  is knowable statically from the graph alone.
- Location: `pce graph check` validation, alongside the existing warnings array.
- Trade-off: adds a freeze-time obstacle to a graph shape that is legitimate and will work once
  delivery exists; the warning would need removing when it does.
- Confidence: `high`. This is strictly cheaper than discovering it at runtime, and the graph in this
  run was statically diagnosable at authoring time.

### Record the composed oid on `criterion-executed`

- Addresses: "`criterion-executed` does not anchor its proof to a ref".
- Change: add an oid field to the `execution` object (or alongside it) naming the commit the
  criterion ran against, so the proof survives deletion of the materialization directory.
- Location: the driver's criteria-execution journaling path, next to where `working_directory` is
  written.
- Trade-off: one more field per criterion record; none behaviourally.
- Confidence: `high`. The journal is described as the admissible run proof, and this is a case where
  it currently records a location guaranteed to become invalid rather than an identity that does not.

### Do not spend a package's overrule on an orchestrator gap

- Addresses: the overrule accounting noted under the first finding.
- Change: distinguish a park that a *human ruling plus external action* resolved from a park the
  supervisor refuted on its own evidence. Only the latter should consume the one-shot allowance. In
  this run the complaint was true when made and false only after the human authorized a push; that is
  closer to the environment-failure accounting rule (which explicitly leaves recovery rungs
  unchanged) than to a supervisor refutation.
- Location: driver overrule accounting; and the `work-graph` skill section 6, which currently offers
  the supervisor exactly one door regardless of who actually resolved the blockage.
- Trade-off: a second overrule path is a second thing to audit, and the distinction could be gamed by
  a supervisor that manufactures a human ruling. Mitigated by requiring the rationale to name the
  external action and its result, as this run's rationale does.
- Confidence: `experimental`. The current behaviour is defensible — one door per package is simple
  and hard to abuse — and this is one run's evidence, not a pattern.

### Make "read the whole criterion output" an explicit supervisor rule

- Addresses: "Criterion output is not summarized, and a truncated read is actively misleading".
- Change: one sentence in the skill's journal-interpretation section: never characterize a criterion
  from a prefix of its output; scan the whole record before forming a theory about whether it proved
  anything.
- Location: `work-graph` skill, section 4.
- Trade-off: none.
- Confidence: `high`, low cost. An alternative worth considering as an experiment is having the
  driver record a small structured summary per criterion, but that requires parsing arbitrary test
  runners and is likely worse than telling the reader to read.

## No-change decisions

- **The gate's two-workspace cost.** `IN1` consumed two workspaces (`w159` worker, `w15E` gate). This
  looks like overhead, but the gate is what caught the `v1`/`v2` defect. No change.
- **`package-base-composed` reporting `dependencies: []` for `TQ1`.** Initially read as a bug. It is
  correct: a `taqsim` base cannot compose an `incidence` commit. The defect is the absent delivery
  mechanism, not this record. No change to the record's semantics.
- **`pce-protect-criteria` breadth.** Blocking read-only `grep` on `vision.md` is mildly annoying, but
  the guard protects a real invariant, the message names the correct alternative immediately, and
  narrowing the match to write-shaped commands risks being fooled by shell quoting. Not worth the
  weakening.
- **The driver's clean exit on `Blocked`.** Pane exited with status 0 and `remain-on-exit` preserved
  it. Correct behaviour; recorded here only because an exited pane can read as a crash to a
  supervisor that does not check `pane_dead_status`.

## Suggested follow-up

- Decide, separately from this run, how `incidence` is meant to be consumed by `taqsim` as a
  committed dependency. The push taken here (branch
  `pce/2026-08-20-taqsim-modelling-layer-on-incidence/IN1/attempt-1` at `3eef8c8`, `main` untouched)
  unblocked the run, but a git-rev pin to a per-attempt archaeology branch is not a durable
  dependency declaration. This is a project decision, not a workflow change.
- Consider whether `promotion` supporting exactly one repository is compatible with two-repository
  graphs being authorable and freezable at all. This graph froze cleanly and, if it reaches
  `assembly-completed`, will refuse promotion on repository count. That mismatch is knowable at
  freeze time and is currently discovered at the terminal boundary. Filed as follow-up rather than a
  finding because this run has not reached that boundary and the refusal has not been observed.
