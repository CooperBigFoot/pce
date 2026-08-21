# Brief: a gate's failure is billed to the package it judges

Status: GRILLED 2026-08-18 — ready to dispatch. Doctrine is recorded in
`docs/adr/0022-a-package-never-pays-for-its-judge.md` and in the `Gate misbehaviour`,
`Gate re-run`, `Challenged finding`, `Gate-blocked`, `Gate ref anchoring` entries of
`CONTEXT.md` (plus their alias and relationship rows). Written after the defect parked an
innocent package with an exhausted ladder during the third real-world driver run (vision
`2026-08-09-gridded-statics-self-name-to-every-consumer`, palaestra). Third observed instance;
the first two were ledger notes from the incidence run (2026-08-17).

## The incident that forces the fix

gridded-statics WP1's implementation passed all three of its criteria twice in consecutive
attempts. It is nonetheless parked "recovery spending exhausted after 3 attributable failures"
(`driver-journal.jsonl` event 48). The three charges:

1. Attempt 2: a real criteria failure — the manifest-only re-issue guards were missing
   (journal event 13). Legitimate.
2. Gate `package-gate-5` emitted one structurally malformed finding →
   `package-failed: gate produced 1 findings; all were structurally malformed` (event 36) →
   charged, local-patch rung consumed. The finding was substantively REAL: the standalone
   reissue probe accepted any TIFF below any ancestor named `gridded_static`, so a
   lexicographically earlier `archive/gridded_static/junk.tif` decoyed it away from the
   canonical `basin=<id>/gridded_static/<label>.tif` raster. Witness `f267ae1` / repair
   `9109419` exist and are sound (`.pce/package-gate-outcomes/WP1/5.json`). Its content was
   discarded AND the package was charged.
3. Gate process crashed: `gate dispatch stopped without an outcome: Exited { code: 1 }` after
   171s (`.pce/package-results/WP1/15.json`, journal event 47) → third charge → park.

The asymmetry that makes this a defect and not a policy: a WORKER that stops without its
required artifact is a `worker-environment-failed` — uncharged, redispatched, separately
budgeted (6 allowed). A GATE that stops without its outcome is `package-failed` — charged to
the package's 2-rung work ladder and answered by redispatching the WORKER, replacing a
criteria-green implementation to cure a defect the implementation does not have. Same failure
shape, opposite accounting, and the party being judged pays for the judge's absence.

Instance history: IC8/incidence (malformed finding → retry burned, worker re-did completed
work), WP1/gridded-statics twice (gate 5 burned the local-patch rung; the crash parked it).
Every instance charged a package whose own work was green at the moment of the charge.

## The root cause of the malformed half — found during the grill, and in scope

The rejection detail is not "the gate wrote nonsense". Both rejections (journal events 34 and
60) read:

    finding refs do not satisfy the WP6 witness/repair contract: package gate ref
    `f267ae10...` resolves but is not reachable from any ref or HEAD in
    /tmp/pce-work-package-worktrees/pce-e21bfc68.../00-orthographos

The commits are correctly authored and the witness/repair relationship is sound. They are
unreachable because:

- every worktree is created detached (`git worktree add --quiet --detach`, `src/main.rs:2660`
  and `src/main.rs:3639`), so a gate's commits advance no name;
- `resolve_package_gate_commit` (`src/main.rs:1110`, reachability at `src/main.rs:1131-1160`)
  requires some ref to contain the OID, or HEAD to be a descendant;
- the driver replays findings in the **worker's** worktrees (`implementation_worktrees` in
  `run_composed_driver_gate`), not the gate's, and a linked worktree's detached HEAD is not
  visible to `for-each-ref` from a sibling worktree;
- the gate brief (`crates/core/src/package_gate.rs:192,196`) says "Do not push, merge, or tag"
  and never says "leave a name pointing at your commits".

So a correct finding survives only if the gate agent happens to create a branch.
`package-gate-8` in the same run produced a usable finding whose repair was merged (journal
events 75, 80); why it differed is not proven, and anchoring makes the question moot rather
than answering it. Attribution alone would only re-roll this dice, so the anchoring fix is in
scope.

## Grilled decisions

1. **Gate misbehaviour is never charged to the package.** Both `package-failed` emissions in
   `run_composed_driver_gate` (`src/main.rs:2100-2118` for the crash, `src/main.rs:2156-2168`
   for the malformed outcome) stop being `PackageFailed`. `charged_failure_count`
   (`crates/core/src/package_driver.rs:1505`) must not see them. A gate that runs and accepts is
   package evidence; a gate that never speaks is not.
2. **The remedy is a new judgment, not new work.** The package stays in
   `DriverPackageState::Judging { issuance }` (`crates/core/src/package_driver.rs:493`) so the
   driver loop's existing Judging branch (`src/main.rs:4521-4527`) re-runs the gate at the same
   issuance. The worker is never redispatched and the implementation head never moves. Model the
   uncharged event on `worker_environment_outcome`
   (`crates/core/src/package_driver.rs:1437-1494`): a `Judging`-preserving event, promoted to a
   distinct terminal event at the ceiling.
3. **Any rejected finding makes the judgment incomplete.** The current trigger is *all* findings
   malformed (`structurally_usable == 0`, `src/main.rs:2143-2168`); a mixed outcome completes the
   package and drops the rejected finding silently. Under the fix, one `FindingRejected` is
   enough to make the outcome incomplete and earn a re-run. Repairs already merged from that
   outcome's usable findings stand, so the re-run judges the hardened tree — accepted, because
   the hardened tree is what ships.
4. **The rejected finding is carried forward as a challenge.** The next gate attempt's brief
   carries the rejected finding's `description`, `repair`, and `proposed_criterion_command`,
   stated as: a previous attempt reported this and could not prove it — confirm it with a proper
   witness and repair pair, or refute it. Stated as a challenge, never as a conclusion, because
   the re-run gate judges the artifact and not its predecessor's report; the priming risk is real
   and the demand for its own witness/repair proof is the guard.
5. **Gate misbehaviour has its own ceiling.** Not shared with the worker's environment budget: a
   new `--gate-failure-limit`, default 3 (gate rounds cost minutes; a stuck gate is likelier
   deterministic than a flaky worker), parsed beside `--environment-failure-limit`
   (`src/main.rs:1888-1940`) and carried on `RecoveryLimits`.
6. **The counted reason text is a fixed phrase.** Same trap as the environment ceiling
   (`Environment-failure ceiling and its reason text` in `CONTEXT.md`): the count is over
   *identical* reasons, so the text must not carry the attempt number, the exit code detail, the
   finding count, a duration, or a worktree path — all of which appear in today's strings. Two
   distinguishable fixed phrases, one for "stopped without an outcome" and one for "outcome
   incomplete", each separately counted; varying detail goes in the event's own fields.
7. **Exhaustion names the gate and is not completion.** At the ceiling the package reaches a
   `gate-blocked` terminal state carrying the gate's reason and the identical-failure count — the
   shape of `PackageEnvironmentBlocked` / `DriverPackageState::EnvironmentBlocked`
   (`crates/core/src/package_driver.rs:264,508`). It is neither `PackageCompleted` nor
   `RecoveryParked`: the package does not ship, its dependents stay blocked, and the run stops
   with the judge named. `charged_failure_count` for that package stays where it was.
8. **Each gate attempt has its own identity.** The gate name is `package-gate-{issuance}`
   (`src/main.rs:2121`), the outcome path is
   `.pce/package-gate-outcomes/<package>/<issuance>.json` (`src/main.rs:2020-2030`), and the
   dispatch graph vision is `<vision>-gate-{issuance}`. All three must vary by gate attempt as
   well, or a re-run overwrites its predecessor's outcome and reuses finding indices under one
   gate name in the journal. Same doctrine as `Attempt isolation and the dispatch temporary
   directory` in `CONTEXT.md`.
9. **The binary anchors the gate's commits.** `pce package gate-agent` names each finding's
   witness and repair commits in the shared ref store before the outcome is accepted (a
   per-attempt ref namespace, e.g.
   `refs/pce-gate/<package>/<issuance>/<attempt>/<n>/{witness,repair}`). The obligation lives in
   the binary, not in the brief, because an obligation an agent can drop is one it will drop —
   the `Role frame and the skill` doctrine. The brief may state it too, as backup.
   `--defer-finding-validation` is already passed to the gate agent (`src/main.rs:2066`), so that
   verb is the right place.
10. **The structural contract does not move.** Anchoring makes a gate's commits findable; it
    never makes a finding admissible. `validate_package_gate_finding_refs`
    (`src/main.rs:1173-1240`) keeps refusing a repair that is not the witness's sole child, a
    witness equal to its repair, and any repository outside the package.
11. **No retroactive repair.** gridded-statics continues under its plan-version bump (v2,
    packages byte-identical), which already reset the unjustly spent ladder — the designed
    escape, already exercised. The v1 park stands as correct history under the old accounting.
    Old journals must still derive under the new binary.

## Acceptance criteria

Each is name / input / observation. #5 and #8 are the designed-to-fail probes. #7 is only fully
trustworthy outside the run that delivers it.

1. **Crashed gate costs the package nothing** — input: a package at green criteria, then a gate
   whose dispatch exits 1 without writing its outcome. Observation: no `package-failed` for that
   package, its charged-failure count is unchanged, and the next dispatch is a gate at the same
   issuance.
2. **The worker is not redispatched for the judge** — input: the same run. Observation: no
   `worker-dispatched` for that package after the gate failure, and the implementation head is
   the same commit before and after.
3. **A discarded complaint reaches the next judge** — input: a gate outcome with one finding
   whose repair ref is unreachable. Observation: the next gate attempt's composed brief contains
   that finding's description and proposed command, framed as a claim to prove or refute.
4. **One good finding does not bury a bad one** — input: an outcome with two findings, one usable
   and one with unreachable refs. Observation: the usable repair merges, a second gate attempt is
   dispatched at the same issuance, and no `package-completed` is emitted from the first attempt.
5. **A permanently broken gate terminates on the gate** (designed to fail) — input: a gate stub
   that exits 1 immediately on every attempt, left to run. Observation: after the ceiling the
   package is gate-blocked with the gate's fixed reason, its charged-failure count is 0, and it
   is neither completed nor parked.
6. **The ceiling can actually be reached** — input: the same crashing stub across attempts.
   Observation: the counted reason text is byte-identical between attempts — no attempt number,
   exit code, finding count, duration, or path in it — and the identical-failure count reaches
   the ceiling.
7. **A fumbling gate's finding survives** — input: a gate that commits witness and repair on its
   detached head and creates no branch. Observation: `git for-each-ref --contains <repair>` in
   the worker's worktree returns a ref, and the finding replays instead of being rejected as
   structurally malformed. (A stub proves the anchoring; whether real gate agents stop producing
   unusable findings is measurable only in a live run.)
8. **Anchoring does not weaken the contract** (designed to fail) — input: an outcome whose repair
   commit does not descend from its witness, and one naming a repository outside the package.
   Observation: both are refused as structurally malformed, exactly as before.
9. **Ungated work does not ship** — input: a gate-blocked package in a graph with a dependent.
   Observation: assembly does not include it, the dependent stays blocked, and the run stops.
10. **Old journals still derive** — input: the existing gridded-statics `driver-journal.jsonl`
    replayed under the new binary. Observation: WP1's v1 park still derives with its original
    reason and no fold error, and the v2 packages proceed.

## Environment facts a dispatched agent will need

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at `ba54dcf` or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration` (branch `integration/work-package-harness`),
  full suite there, then fast-forward main. `./install.sh` from the MAIN checkout only. The
  install swaps the live `pce` binary under running driver runs; **the human times the install**.
- Workspace layout and doctrine: `AGENTS.md`. `src/` is the composition root only; all domain
  logic lives in `crates/*`. Clippy denies `.unwrap()`, `.expect()`, and `println!`/`eprintln!`
  in library crates. `cargo fmt`, `cargo clippy --workspace --all-targets`,
  `cargo test --workspace`.
- Key code, verified at `ba54dcf`:
  - `run_composed_driver_gate` — `src/main.rs:2012`; gate outcome path 2020-2030; gate dispatch
    graph 2035-2049; gate-agent worker arguments 2051-2070 (`--defer-finding-validation` at
    2066); crash emission 2100-2118; gate name 2121; finding replay loop 2123-2148;
    `GateFinished` 2149-2155; malformed emission 2156-2168; `PackageCompleted` 2169-2176.
  - Driver loop's Judging branch — `src/main.rs:4521-4527`; the post-criteria entry into the gate
    — `src/main.rs:2340-2358`.
  - `replay_driver_finding` and `DriverFindingDisposition` — `src/main.rs:5429-5572`; structural
    validation and `FindingRejected` at 5460-5490.
  - `validate_package_gate_finding_refs` — `src/main.rs:1173`; `resolve_package_gate_commit` and
    its reachability check — `src/main.rs:1110-1160`.
  - Detached worktree creation — `src/main.rs:2660` and `src/main.rs:3639`.
  - Recovery accounting — `charged_failure_count` `crates/core/src/package_driver.rs:1505`;
    `worker_environment_outcome` 1437-1494; the `PackageFailed` fold 1262-1277; the
    `Judging`-requiring folds for `GateFinished` / `CriterionExecuted` / `FindingRejected`
    1180-1200; `DriverPackageState` 488-515; `DriverEvent` variants 258-268 and 396-440.
  - Limit parsing — `src/main.rs:1888-1940`; usage string `src/main.rs:117`.
  - Gate brief composition — `crates/core/src/package_gate.rs:150-216` (the "do not push, merge,
    or tag" sentence at 192, the outcome schema sentence at 196).
- Evidence artifacts: the gridded-statics planning directory
  `/Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-09-gridded-statics-self-name-to-every-consumer/`
  — `driver-journal.jsonl` (events 13, 34, 36, 47, 48, 60, 62, 75, 80),
  `.pce/package-gate-outcomes/WP1/5.json` (the real-but-malformed finding),
  `.pce/package-results/WP1/15.json` (the gate crash), `supervision.md`. The incidence run's
  journal carries instance 1 (IC8, gate `package-gate-10`, finding rejected
  `structurally-malformed`, retry issuance 11). That run is live on plan version 2 — read it, do
  not drive it.
