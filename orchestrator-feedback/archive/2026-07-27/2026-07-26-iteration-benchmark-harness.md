# PCE workflow feedback: iteration-benchmark-harness

- Date: `2026-07-26`
- Orchestrator: `Claude Code (Fable 5), session 6ebd7846-a398-4ef7-ad93-41c9b5d11fa6, /pce skill (PCE-PR-C)`
- Run: `bluesmith:planning/2026-07-17-iteration-benchmark-harness` (cross-repo: bluesmith producer, stopwatch consumer)
- Outcome: `completed` — build phase 8/8 milestones + consume-bump (2026-07-17→24, with a multi-day user pause), then an operational validation phase (real EC2) that the workflow does not model, then `/land-ticket` of bluesmith#119

## Executive summary

The core PCE-PR-C loop (Codex authors, Claude gates, orchestrator owns git/state/escalation) delivered all eight milestones across two repos with no product-quality escapes, and `state.json` carried the run across a multi-day pause and at least two context compactions. The highest-impact findings are: (1) the mocked acceptance gates were structurally blind to environmental contracts — eight real-world defects survived every green gate and were only caught by gated real runs, which the workflow does not model as a phase; (2) Codex dispatch has three recurring environment traps (stdin hang, zsh backtick substitution, read-only `.git` in worktrees) of which the skill documents only one, so the other two were re-discovered by failure; (3) the cross-repo freshness check as initially contracted (`cargo check`) false-greened 21 latent consumer test failures, forcing an unplanned milestone. Most friction was absorbed by the delta/escalation mechanisms working as designed.

## Evidence reviewed

- `bluesmith:planning/2026-07-17-iteration-benchmark-harness/state.json` — 6 deltas, 2 escalations, milestone statuses, `operational_log` (5 events), `pause`/`resume_orientation_2026-07-24`, `lockfile_lesson`, `stopwatch_provenance_rule`, `pre_spend_checklist`, `followups`
- Session transcript (post-compaction portion directly; pre-compaction via the compaction summary and durable artifacts)
- Dispatch prompt files: session scratchpad `fix_jitter.txt`, `fix_external_id.txt`, `fix_schema_null.txt`, `fix_bootstrap_sed.txt`, `fix_cargo_env.txt`, `fix_rsync.txt`, `bootstrap.sh`
- Codex verdicts: `.codex-result.json` for fixes #5–#7 (including the fix #6 `BLOCK: git-metadata-read-only` verdict)
- GitHub: bluesmith PRs #139–#145, tags v0.1.495–v0.1.511; stopwatch v0.2.6/.7/.12; githubstatus.com API output ("Pull Requests major_outage") captured in-transcript 2026-07-24
- Harness outputs: `iteration-benchmark-fleet-summary.json`, `iteration-benchmark-verdicts/0000-main-0d6e4079e367/verdict.json` (first screen REJECT, second screen PASS)
- Project memory: `iteration-benchmark-harness-pce-paused.md`, `pce-cross-repo-eager-rebuild-cargo-test.md`, `codex-exec-stdin-hang.md`, `codex-git-metadata-sandbox.md`

## What worked

### Verdict-schema-gated cold dispatches (never `resume`)

- Evidence: every Codex artifact returned a schema-valid APPROVE/BLOCK (`.codex-result.json` files); the M1 executor BLOCK on the wrong plan-fixture units ("m3 s-1") forced a plan revision instead of silently shipping a wrong fixture; fix #6's BLOCK correctly reported the sandbox commit denial rather than fabricating a sha.
- Effect: no dispatch ever ended in an ambiguous state; a blocked artifact was always distinguishable from a broken dispatch.

### `state.json` as the resume spine

- Evidence: `pause` (2026-07-17, user answer "Pause for 118") and `resume_orientation_2026-07-24` keys; the run also crossed context compactions during the operational phase and continued from `operational_log` without re-deriving prior decisions.
- Effect: a multi-day, multi-context run stayed coherent; nothing was re-planned after resume — the recorded `D-resume-m4m8-post-118` delta re-scoped content while preserving the approved decomposition.

### Delta mechanism for runtime graph adaptation

- Evidence: 6 recorded deltas (`D-fmt-clippy-scope-bluesmith`, `D-defer-consume-bumps`, `D-resume-m4m8-post-118`, `D-consume-bump-node`, `D-fix-run-instances-count`, `D-fix-bootstrap-run-toml-sed`) in `state.json.deltas`.
- Effect: baseline rustfmt drift, a post-#118 re-scope, and a batched consumer version-pin update were all absorbed without re-running Phase 1/2 planning; the audit trail states each rationale.

### Escalation discipline at genuine human decision points

- Evidence: `E1-stopwatch-base-ref` (trunk choice: main vs milestone-7) and `E2-ec2-boundary` (no AWS creds; real spend impossible/cost-incurring) in `state.json.escalations`; both resolved by explicit user answers; E2 produced `pre_spend_checklist`, which then gated every real EC2 action until the user's blanket "Can you do all of them?".
- Effect: no unauthorized spend and no guessed trunk; the spend gate survived the pause/resume boundary because it lived in state, not in conversation.

### Orchestrator-owned git after Codex sandbox denial

- Evidence: fix #6 BLOCK (`git-metadata-read-only` on `.git/worktrees/fix-cargo-env/index.lock`); the pre-existing memory rule ("orchestrator commits directly, explicit `git add`, never `-A`") converted it to a routine recovery; fix #7's prompt pre-adapted by instructing Codex to treat commit denial as non-blocking, and Codex then committed successfully anyway (54ce42b).
- Effect: a known-fatal sandbox limitation cost one extra orchestrator commit instead of a stalled loop.

## Friction and failures

### Mocked acceptance gates were blind to environmental contracts

- Severity: high
- Phase: execution/review (gate design), surfaced post-merge
- Observation: eight defects shipped through fully green gates (107→115 pytest, `cargo build`, plan critics, PR reviewers) and were found only by real EC2 runs: `aws run-instances` argv (`--min-count`/`--max-count` vs `--count`), box `run.toml` paths, parquet schema nullability, selector-form `external_id`, flat-curve calibration abort, missing cargo env in non-login ssh, rsync with no excludes (would upload a 148 GB `target/`), and a calibration pin beyond the package horizon.
- Evidence: PRs #139–#145 / tags v0.1.503–v0.1.511; `operational_log` entries; first fleet verdict REJECT (`iteration-benchmark-verdicts/0000-main-0d6e4079e367/verdict.json`, "baseline row count 24 != resolved horizon 65536").
- Inference: the defects cluster exactly on seams the tests mocked (CommandRunner, fixture-authored parquet). Mocks inherited the author's wrong assumptions, so the tests pinned the wrong contract with full coverage. This is a property of the gate strategy, not of any single milestone.
- Impact: an entire unmodeled "operational validation" phase (~1 day, 7 fix mini-loops, ~$2 EC2) was required after the workflow declared the vision built.

### The operational phase exists outside the workflow's phase model

- Severity: medium
- Phase: post-merge (unmodeled)
- Observation: the skill's loop ends at milestone merges. Calibration, certification, the fleet screen, and the 7 defect-fix loops were run under an improvised compressed cycle: single-fix Codex dispatch from a scratchpad prompt file → verdict → orchestrator merge, with no plan critic and no PR reviewer.
- Evidence: `fix_*.txt` prompt files; `state.json.phase` values (`operational:calibrating`, `operational:fleet-rescreen`, `operational:complete`) — none of these phases appear in the skill; PRs #141/#142/#144/#145 merged on Codex self-verdict plus orchestrator diff review only.
- Inference: the compressed loop was the right cost/benefit for one-file fixes with a hard external oracle (the real run), but it was a judgment call each time; nothing in the workflow authorizes or bounds it.
- Impact: repeated improvisation; review-rigor inconsistency between phases is undocumented rather than chosen.

### Codex dispatch environment traps (two of three undocumented)

- Severity: medium
- Phase: execution (dispatch mechanics)
- Observation: three traps recurred: (a) missing `< /dev/null` hangs `codex exec` ("Reading additional input from stdin") — documented in the skill, still hit once when combined with (b); (b) backticks in inline prompts are command-substituted by zsh before Codex sees them — `ps` output showed pytest output embedded in the prompt argv; (c) Codex under `--sandbox workspace-write` cannot write `.git` metadata from a linked worktree (fix #6 BLOCK).
- Evidence: user message "It's stuck" (M8 fix dispatch, pre-compaction); killed PIDs and re-dispatch via file + `"$(cat …)"`; `codex-exec-stdin-hang.md` and `codex-git-metadata-sandbox.md` memory entries; fix #6 `.codex-result.json`.
- Inference: (b) and (c) live only in this user's project memory; a fresh orchestrator without that memory re-derives them by failure.
- Impact: one hung dispatch requiring user intervention; one BLOCK verdict round-trip that a prompt rule would have avoided.

### Cross-repo freshness check false-greened as `cargo check`

- Severity: high (build phase, pre-compaction)
- Phase: orientation (repo contract) / eager rebuild
- Observation: the Phase 0 contract's `consumed_artifacts.freshness_check` was initially check-only; it stayed green while 21 stopwatch test failures were latent, which surfaced later and forced an unplanned milestone (M5b0).
- Evidence: `pce-cross-repo-eager-rebuild-cargo-test.md` memory entry ("check-only false-green hit 21 latent stopwatch failures → forced unplanned M5b0"); the corrected rule now appears in Program #117's Map Notes ("consumer `cargo test --workspace`, not `cargo check`").
- Inference: the skill's contract shape permits any command as `freshness_check`; nothing steers the orientation sweep away from the cheap-but-insufficient choice.
- Impact: one unplanned milestone of consumer repair; the fix is now tribal knowledge in a Map Note and a memory file rather than a workflow rule.

### Repo-contract gaps re-learned as mid-run rules: lockfile and provenance ordering

- Severity: medium
- Phase: execution (repeated across milestones)
- Observation: two repo behaviors caused failures until encoded as standing prompt text: (a) `cargo generate-lockfile` upgraded ~35 unrelated registry deps in the M3 PR, requiring lock restore + amend; every subsequent dispatch prompt carries "minimal regen via cargo build, never generate-lockfile"; (b) stopwatch provenance tests require version-bump BEFORE tests, a clean committed tree, and count untracked files (e.g. `pr-body.md`) as dirty — M2/M3 gates failed until the ordering was learned.
- Evidence: `state.json.lockfile_lesson` and `state.json.stopwatch_provenance_rule` (dedicated ad-hoc state keys — themselves evidence the contract shape had no home for these facts); identical boilerplate lines in all six `fix_*.txt` prompts.
- Inference: both are per-repo invariants discoverable at orientation time; the pinned contract shape (format/lint/typecheck/test/build/preflight/version_bump/branch_pr) has no field for "lockfile regen command" or "gate ordering constraints", so they leaked into prompt boilerplate.
- Impact: two spoiled gate runs, one amended PR, and permanent per-dispatch prompt overhead.

### External-service outage handling is unspecified

- Severity: low
- Phase: merge
- Observation: GitHub Pull Requests had a major outage during the kemme-pin merge; `gh pr create` failed repeatedly (GraphQL 5xx, then empty REST bodies). Improvised: verified via githubstatus API, ran a 60s-interval then 300s-interval background retry loop; PR #145 landed ~40 min later.
- Evidence: transcript commands and outputs (`PR_CREATE_GAVE_UP`, then `PR_CREATED=145`); githubstatus summary output ("Pull Requests major_outage").
- Inference: none needed; pure external failure. Notable only because the orchestrator initially retried inline three times before checking service status.
- Impact: ~40 min delay on one merge; no state damage (work continued in parallel because the config-driven re-screen did not depend on the merge).

### `gh pr merge --delete-branch` conflicts with the primary checkout

- Severity: low
- Phase: merge (recurring ceremony)
- Observation: `gh pr merge --squash --delete-branch` fails after merging with `fatal: 'main' is already used by worktree at <repo>` when invoked from a step worktree; the merge succeeds but the command exits nonzero mid-sequence. Every fix-merge thereafter used merge-without-delete plus separate branch/worktree cleanup.
- Evidence: PR #141 merge output in transcript; subsequent merges (#142, #144, #145) use the two-step form.
- Inference: interaction between `gh`'s local-branch cleanup and git worktrees; predictable, thus scriptable.
- Impact: one confusing error per naive invocation; minor.

## Recommendations

### Add the two undocumented Codex dispatch traps to the skill's dispatch rules

- Addresses: Codex dispatch environment traps
- Change: alongside the existing `< /dev/null` rule, add: "Never inline backticks (or `$(…)`) in a dispatch prompt string — write the prompt to a file and pass `"$(cat <file>)"`" and "A Codex dispatch inside a linked worktree may be unable to write `.git` metadata; instruct fix executors that a denied commit with all gates green is reported in the verdict, not a BLOCK, and the orchestrator commits the staged scope itself with explicit `git add` (never `-A`)."
- Location: `~/.claude/skills/pce/SKILL.md`, "The delegation contract / codex exec dispatch" section
- Trade-off: two more standing rules; the file-based prompt adds one Write per dispatch.
- Confidence: high (both traps reproduced; both mitigations used successfully in-run)

### Constrain `freshness_check` to the consumer's test gate

- Addresses: cross-repo freshness false-green
- Change: in the Phase 0 repo-contract instructions, require `consumed_artifacts.freshness_check` to be the consumer repo's `test` command (or state explicitly why a weaker command is sufficient); forbid defaulting to a check/compile-only command.
- Location: `~/.claude/skills/pce/SKILL.md`, Phase 0 repo-contract shape and "Cross-repo runs / eager rebuild" section
- Trade-off: eager rebuilds get slower (full consumer test suite per producer merge); `D-defer-consume-bumps`-style batching remains the escape valve and worked well here.
- Confidence: high (one unplanned milestone traces directly to the weak check)

### Add `lockfile` and `gate_ordering` fields to the repo contract shape

- Addresses: repo-contract gaps re-learned mid-run
- Change: extend the pinned contract JSON with optional `lockfile: "<exact minimal-regen command>"` and `gate_ordering: "<prose constraints, e.g. version-bump before tests; untracked files count as dirty>"`, populated by the orientation sweep; instruct plan-writers and fix dispatches to copy them verbatim instead of orchestrator boilerplate.
- Location: `~/.claude/skills/pce/SKILL.md`, Phase 0 contract shape (the normative JSON block)
- Trade-off: slightly larger orientation scope; two optional keys added to a pinned shape.
- Confidence: medium (clear recurring cost, but the boilerplate workaround did function)

### Define a lightweight "operational fix loop" profile

- Addresses: the unmodeled operational phase
- Change: add a short section: when a merged vision is being validated against a real environment, single-cause fixes with a hard external oracle may use a compressed loop (cold Codex fix dispatch with verdict schema → orchestrator diff review + local gate re-run → merge), recording each fix as a delta; anything multi-file or contract-changing returns to the full plan/critic/review loop. State that real-environment validation deserves an explicit phase in `state.json` (`operational:*`).
- Location: `~/.claude/skills/pce/SKILL.md`, after the Phase 3 loop
- Trade-off: codifies a lower-rigor path, which could be over-used; the single-cause + external-oracle preconditions are the guard.
- Confidence: medium (the pattern worked 7/7 times here, but n=1 run)

### Mark seam-mocked gates as non-final in plans that target external systems

- Addresses: mock-blind gates
- Change: instruct step-plan-writers that when acceptance gates mock an external boundary (CLI argv, wire schemas, remote shells), the plan must name the unverified contract explicitly ("real-run validation pending") in its done-criteria, so the milestone closes as "built, environmentally unverified" rather than implicitly done.
- Location: `~/.claude/skills/pce/SKILL.md`, step-plan-writer prompt (self-sufficiency checks)
- Trade-off: adds a labeling obligation, not a testing obligation — real verification still needs the operational phase; risk of boilerplate if applied to purely internal seams.
- Confidence: medium

## No-change decisions

- **AskUserQuestion pauses (E1/E2) and the pre-spend gate**: added latency (E2 paused the run for days) but both were genuinely the human's decisions (trunk choice; money). The spend gate surviving in `state.json` across the pause is the mechanism working, not friction.
- **Fresh-cold-dispatch-only (never `codex exec resume`)**: re-reading artifacts from disk each round cost tokens but eliminated stale-context failure modes; no observed defect traces to it.
- **Per-fix 3-line `pr-body.md` and one-commit-exact-scope contracts**: near-zero cost, and the exact-scope rule is what made orchestrator-side commit recovery (fix #6) trivially safe.
- **GitHub outage**: external; the improvised status-check-then-backoff behavior is not worth a workflow rule beyond what any orchestrator would do.
- **`gh pr merge --delete-branch` worktree conflict**: real but trivial; encoding merge mechanics into the skill would add ceremony for a one-line lesson that memory already carries.

## Suggested follow-up

- Experiment: a "contract-probe" milestone pattern for visions targeting external systems — one early, cheap real interaction per external contract (one `aws --dry-run`, one real parquet file read, one non-login ssh command) before the mocked build milestones pin those contracts in fixtures. Would likely have converted 5 of the 8 operational defects into build-phase test inputs. Needs design: it partially conflicts with the deliberate "build now, gate real runs" spend decision, so probes must be individually cost-bounded and pre-authorized.
- Separate issue (stopwatch, already filed in Program #117 Map Notes and `state.json.followups`): `m7-dudh-run.sh` `steps_executed` misreporting — product debt discovered by the workflow, tracked outside it.
