# PCE workflow feedback: values carry the descriptors that dimension them

- Date: `2026-07-31`
- Orchestrator: `Claude Code, Opus 5, session 8dc0cfbd`
- Run: `palaestra/planning/2026-07-31-values-carry-the-descriptors-that-dimension-them` (cross-repo: palaestra, hdx, orthographos, metis)
- Outcome: `in progress` — 3 of 9 planned steps merged, a 4th under review, 4 milestones not yet decomposed

## Executive summary

The adversarial gates are the strongest part of this workflow. Six substantive
planning defects were caught before a line of product code was written,
including one that would have silently written one basin's values under another
basin's geometry. That is the workflow paying for itself.

The weakest part is that the workflow has a precise model of *step* state and
almost no model of *orchestrator* state. Every high-severity failure in this run
came from an orchestrator action that the workflow does not describe: committing
to `main` outside a step, force-moving a branch, and bumping a version after a
plan had already been approved against it. The orchestrator is given git
authority and version-policy authority with no rules constraining either, and it
used both to destroy three merged commits.

Second finding of equal weight: `pce status` is designated the merge authority
but cannot answer the merge question on a cross-repo run, because it projects
every step through the primary repository. On this run the mandated authority
was unusable at three of its own call points and had to be replaced with ad-hoc
`gh pr view`.

## Evidence reviewed

- `planning/2026-07-31-values-carry-the-descriptors-that-dimension-them/events.jsonl` — the full run log, all seven kinds
- `.../milestones.json` (digest `291b37dade1979b4…`) and `milestone-{1,2,3,6}/steps.json`
- `.../milestone-{1,2,3,6}/step-1/plan.md` and their `review-*.json` verdicts
- `.pce/repository-contract.json` in all four repositories
- Merged step commits: palaestra `60a4c61`, hdx `4bbb768`, orthographos `1c209d2`; metis `65154ba` under review
- `git reflog`, `git merge-base --is-ancestor`, and `git tag` output across the four repositories during the recovery described below
- `codex exec` executor transcripts and this session's own tool output

## What worked

### Adversarial critics gating every artifact before execution

- Evidence: six defects caught by `milestone-critic`, `step-critic`, and
  `step-plan-critic` rounds, each recorded as a `delta` or a plan revision:
  1. The vision's premise for hdx was false — `check_t2` already exists at
     `validate.rs:1176` and is invoked at `:1490`. The real fault is that hdx
     *asserts* an encoding it never reads. Vision item 7 was retargeted.
  2. `Op::Coverage` at `ops/mod.rs:527` is a sixth gridded operation the vision
     miscounted; area-weighted reduce is two legs, making seven executor
     transformations. ADR-0013's enumeration was widened.
  3. m6 could not honestly record a windowed *scalar* fit window before m5. Both
     earlier milestone-critic rounds had reasoned only about the unwindowed leg.
  4. The m3-s1 plan would have sourced `merge.rs:793`'s geometry from the
     canonical carrier, writing one basin's values under another basin's
     geometry — silently, because `n_lat`/`n_lon` only feed
     `force_align_slab_onto_axis`.
  5. m3-s3 over-claimed acceptance criterion 1's axis/columns half, which is not
     provable until m4.
  6. The m6-s1 replay test was assigned to an integration crate that cannot
     reach a `pub(crate)` fold.
- Effect: defects 4 and 6 would have reached merged code. Defect 4 is a silent
  data-corruption class — exactly the failure the vision exists to eliminate.

### Ground truth supplied as refs, not checkouts

- Evidence: every critic and reviewer dispatch supplied
  `git show <ref>:<path>` and `git diff <base>...<head>` rather than a path.
- Effect: no critic reviewed a stale or dirty tree, in a run where working trees
  were dirty for lockfile reasons much of the time.

### Appendable contract knowledge, propagated verbatim

- Evidence: `appendable.environment_hazards` and `lockfile_rules` in all four
  contracts. Each entry was purchased by a specific failed isolate — sibling
  symlinks for orthographos's transitive path-dependency closure, two
  independent gitignored fixture generators, `uv.lock` rewriting under any `uv`
  invocation in a worktree, and the sandbox's need for
  `<repo>/.git/worktrees/<node-id>` as an explicit writable root.
- Effect: five isolate failures occurred before the hazards existed; none
  recurred after they were propagated into later dispatches.

### `version_policy: SERIALIZE_DISPATCHES` as an accidental safety net

- Evidence: after the branch destruction described below, `v0.1.83`, `v0.1.96`,
  and `v0.1.214` still pointed at the discarded merge commits, which is how they
  were recovered.
- Effect: the policy was chosen for release discipline and turned out to be the
  only durable reference to merged work. Under the `version_policy: NONE` that
  bootstrap originally measured for these repositories, the same mistake would
  have left those commits reachable only through reflog.

## Friction and failures

### `pce status` cannot answer the merge question on a cross-repo run

- Severity: `high`
- Phase: `merge`
- Observation: `pce status` projects every step node through the *primary*
  repository. On this run the primary is palaestra, but steps m2-s1, m3-s1, and
  m6-s1 live in hdx, orthographos, and metis. The snapshot reports merge state
  against palaestra for all of them.
- Evidence: `pce status --file <LOG> --vision-dir <DIR>` output during the m2-s1
  and m3-s1 merges; compared against `gh pr view` in the owning repositories,
  which disagreed.
- Inference: `resolve_primary_repository` selects one root and
  `canonical_nodes` projection reuses it for every node, with no per-node
  repository binding. The event log has no field that binds a node to a
  repository, so the projection has nothing else to use.
- Impact: SKILL.md mandates quoting a successful status snapshot immediately
  before every step merge and every milestone merge. That mandate was
  unsatisfiable for three of four repositories. The orchestrator substituted
  direct `gh pr view` calls and recorded the deviation, meaning the designated
  authority was bypassed at its highest-stakes call point.

### Nothing prohibits the orchestrator from destroying merged work

- Severity: `high`
- Phase: `recovery`
- Observation: the orchestrator ran
  `git branch -f pce/<vision>/milestone-<m> main` in three repositories. Step PRs
  merge into the *milestone* branch and never into `main`, so `main` did not
  contain the step merges and the force-move discarded `60a4c61`, `4bbb768`, and
  `1c209d2` locally and on origin.
- Evidence: `git merge-base --is-ancestor <step> <milestone-branch>` returned
  non-zero for all three after the force-move; the `delta` record at node
  `m1-s1` documents the full sequence and repair.
- Inference: the two-tier merge model is stated as a merge *direction* rule but
  its consequence — that `main` is strictly behind every active milestone
  branch, so fast-forwarding a milestone branch to `main` is always a
  destructive operation — is never stated. The orchestrator reasoned "main has
  the newer contract commit, move the branch there" without checking
  containment.
- Impact: three merged step commits and their PR merge state destroyed on
  origin. Recovered only because of the tag side effect described above. This is
  the single largest risk realized in the run.

### `version_policy` has no rule for orchestrator commits to `main`

- Severity: `high`
- Phase: `orientation` / `merge`
- Observation: `SERIALIZE_DISPATCHES` describes one patch bump folded into each
  *step* commit. The orchestrator also commits directly to `main` — to add
  `.pce/repository-contract.json` and later to append learned hazards. Those
  commits bumped `main` to `0.1.83`, `0.1.96`, and `0.1.214`, versions the step
  merges had already claimed and tagged, leaving two distinct commits asserting
  one version in each repository.
- Evidence: `fatal: tag 'v0.1.83' already exists` (and the hdx and orthographos
  equivalents) emitted during the hazard commits; confirmed afterwards by
  comparing `git show main:<manifest>` against `git rev-parse v<version>`.
- Inference: the contract lifecycle explicitly authorizes tracked-file edits to
  `.pce/repository-contract.json` outside a step, but the version policy is
  written only in terms of dispatches. The two authorized behaviors contradict
  each other in any repository whose `AGENTS.md` mandates bump-and-tag per
  commit to `main`.
- Impact: version sequence corrupted in three repositories. Repaired by moving
  `main` above the tagged step (`v0.1.84`, `v0.1.97`, `v0.1.215`) and merging
  forward, at the cost of three extra commits, three tags, and three merges. The
  collision error was the correct signal and was initially dismissed as noise,
  which delayed detection of the branch destruction that accompanied it.

### A version bump silently invalidates already-approved plans

- Severity: `high`
- Phase: `step planning`
- Observation: approved plans pin the bump by value ("change the root package
  version exactly from `0.1.213` to `0.1.214`"). When the orchestrator later
  refreshed `Cargo.lock` and bumped versions on `main`, every approved plan
  pinning a now-stale starting version became infeasible.
- Evidence: executor verdicts for m2-s1 and m3-s1, both
  `PLAN_INFEASIBLE`: *"the required serialized patch bump must start at 0.1.94,
  but the supplied worktree already starts at 0.1.95"* and *"the supplied
  worktree is already version-bumped one commit beyond the mandated starting
  ref"*.
- Inference: `planning-artifact-approved` records a digest of the plan, which
  correctly detects the plan changing. Nothing detects the *world the plan pins*
  changing. The approval remains valid while its premises rot.
- Impact: two executor rounds lost outright. The orchestrator then repaired
  m2-s1 and m3-s1 but missed m6-s1, costing that step a third execution round.
  Three wasted `codex exec` executions traceable to one unmodelled dependency.

### A gate verdict can be a summary rather than evidence

- Severity: `medium`
- Phase: `review`
- Observation: the orchestrator dispatched a probe to check whether the m6-s1
  executor's `/usr/bin/time -l` measurement worked. The probe's prose summary
  said yes; its own raw output contained no `maximum resident set size` line. On
  the strength of the summary the orchestrator accused the executor of a
  fabricated measurement. A second probe demanding raw output showed
  `grep -c` = 0 and `sysctl kern.clockrate: Operation not permitted`. The
  executor had been right.
- Evidence: both probe transcripts; the resulting hazard entry in hdx's contract
  now records the sandbox `sysctl` denial.
- Inference: the delegation contract requires a schema or artifact path from
  every dispatch, but does not require a verdict to quote the raw output it
  rests on. A conclusory field satisfies the schema.
- Impact: one wasted round trip and a false accusation against a correct
  executor. In a run where the orchestrator is the only party that can overrule
  a gate, a gate that reports conclusions without evidence is worse than no gate.

### `pce contract learn` cannot fire in practice

- Severity: `medium`
- Phase: `orientation`
- Observation: `contract learn` requires a byte-exact key-finding match between
  the current log and a prior log to promote a finding into the contract.
- Evidence: SKILL.md's stated recurrence condition; no invocation succeeded in
  this run despite several findings recurring in substance across steps.
- Inference: key findings are free-text authored by different agents at
  different times. Byte-exact recurrence across two runs is effectively
  impossible.
- Impact: every hazard in every contract on this run was written by hand by the
  orchestrator. The designated learning mechanism contributed nothing.

### `codex exec --output-schema` combined with `-o` clobbered an authored artifact

- Severity: `medium`
- Phase: `milestone planning` / `step planning`
- Observation: a `codex exec` invocation that both wrote `steps.json` and
  declared `--output-schema` with `-o` pointed at that same path overwrote the
  planner's authored content with the structured-output envelope.
- Evidence: milestone-3 `steps.json` had to be recovered verbatim from the
  planner's own transcript diff.
- Inference: the workflow specifies `--output-schema` for schema-bearing
  dispatches and specifies exact artifact paths for authoring dispatches, and
  does not warn that combining them aims two writers at one file.
- Impact: near-loss of an approved planning artifact; recovery required reading
  a transcript.

### `contract bootstrap` does not support a CI-less, non-Cargo repository

- Severity: `medium`
- Phase: `orientation`
- Observation: bootstrap failed on palaestra (uv/Python, no CI workflows, no
  `Cargo.toml`) and had to be escalated to the human, who authorized the
  orchestrator to hand-author the contract from `AGENTS.md`.
- Evidence: `escalation-open` / `escalation-close` pair at node `m1-s1`; the
  resulting hand-authored `.pce/repository-contract.json`.
- Inference: bootstrap's gate detection appears to assume a Cargo workspace or a
  CI workflow file to read commands from.
- Impact: the primary repository of a cross-repo run required a human decision
  in the first ten minutes. Bootstrap also measured `version_policy: NONE` for
  repositories whose `AGENTS.md` mandates a bump per commit, which required a
  second escalation.

### Approved plans go stale against merged sibling work with no detection

- Severity: `medium`
- Phase: `step planning`
- Observation: plans cite starting-ref line numbers and symbol names. A merged
  step in the same milestone moves them. Nothing rechecks an approved plan
  against the ref the executor will actually receive.
- Evidence: the version-pin instance above is the sharp case; the same exposure
  exists for every `file:line` citation in every approved plan.
- Inference: `planning-artifact-approved` binds plan bytes to a digest but not
  to a ref.
- Impact: latent. Only the version pin fired on this run, but m3 has three steps
  in one crate and the exposure grows with milestone depth.

### Subagent liveness is not observable

- Severity: `low`
- Phase: `review`
- Observation: `TaskList` returns no tasks for a running Claude subagent, so an
  orchestrator that lost its notification cannot distinguish "still running"
  from "dead".
- Evidence: `TaskList` returned "No tasks found" while a `pr-reviewer` subagent
  was demonstrably still running.
- Inference: harness-level, not PCE-level, but it interacts with PCE because
  re-dispatching produces a duplicate agent racing for one output path.
- Impact: risk of duplicate dispatch corrupting a verdict file. Avoided here
  only by prior knowledge.

## Recommendations

### Bind each node to a repository in the event log

- Addresses: "`pce status` cannot answer the merge question on a cross-repo run"
- Change: add a required `repository` field to the `dispatch` payload, and have
  `canonical_nodes` projection resolve each node's repository from its most
  recent dispatch rather than from `resolve_primary_repository`. Fall back to
  the primary repository when a node has no dispatch yet.
- Location: `crates/core` node projection; the `dispatch` payload schema in
  `SKILL.md` "Event log contract"
- Trade-off: a new required field on the most frequently appended kind; existing
  logs need the fallback path.
- Confidence: `high`

### State that `main` is behind every active milestone branch

- Addresses: "Nothing prohibits the orchestrator from destroying merged work"
- Change: add to the merge section of `SKILL.md`: *"Step PRs merge into the
  milestone branch and never into the default branch, so the default branch is
  strictly behind every active milestone branch until that milestone merges.
  Never fast-forward, reset, or force-move a milestone branch. To carry a
  default-branch commit into a milestone branch, merge it. Before any branch
  write, verify containment with `git merge-base --is-ancestor <old-head>
  <new-head>` and abort if it fails."*
- Location: `SKILL.md`, Phase 3 merge rules
- Trade-off: none; it forbids an operation that is never correct here.
- Confidence: `high`

### Give orchestrator commits to `main` their own version rule

- Addresses: "`version_policy` has no rule for orchestrator commits to `main`"
- Change: state that a contract-lifecycle commit to the default branch takes the
  next patch above the highest existing tag in that repository, not the next
  patch above the default branch's current manifest version. Add: *"A tag
  collision during a bump means the branch topology is not what you believe.
  Stop and re-derive; never retry the bump."*
- Location: `SKILL.md`, "Version and tag behavior is per repository"
- Trade-off: requires the orchestrator to read tags, not just the manifest.
- Confidence: `high`

### Make the plan's starting ref part of the approval, and re-verify before dispatch

- Addresses: "A version bump silently invalidates already-approved plans" and
  "Approved plans go stale against merged sibling work"
- Change: extend the `planning-artifact-approved` payload with a required
  `starting_ref`. Before every `step-executor` dispatch, require the
  orchestrator to compare the recorded `starting_ref` against the ref the
  executor will receive; if they differ, the approval is void and the plan
  returns to `step-plan-critic` rather than proceeding.
- Location: `SKILL.md`, "Event log contract" and Phase 3 step 3
- Trade-off: a re-approval round whenever a milestone branch advances. That cost
  is strictly smaller than the three executor rounds lost on this run.
- Confidence: `high`

### Require raw evidence in every verdict, not a conclusion

- Addresses: "A gate verdict can be a summary rather than evidence"
- Change: add a required `raw_evidence` string to `verdict.schema.json`,
  specified as verbatim command output — not a description of it — and instruct
  gates that a verdict whose `raw_evidence` does not itself demonstrate the
  claim is invalid. Instruct the orchestrator to read `raw_evidence` before
  acting on `verdict`.
- Location: `~/.claude/skills/pce/schemas/verdict.schema.json`; the delegation
  contract in `SKILL.md`
- Trade-off: larger verdict payloads.
- Confidence: `high`

### Never point `-o` at a path the same dispatch authors

- Addresses: "`codex exec --output-schema` combined with `-o` clobbered an
  authored artifact"
- Change: add to the delegation contract: *"A dispatch that authors a file at a
  known path must not also declare `--output-schema` with `-o` pointed at that
  path. Use one or the other: a schema-bearing dispatch returns structured
  output and writes nothing; an authoring dispatch writes its artifact and
  returns nothing."*
- Location: `SKILL.md`, "The delegation contract"
- Trade-off: none.
- Confidence: `high`

### Replace byte-exact recurrence in `contract learn` with orchestrator judgment

- Addresses: "`pce contract learn` cannot fire in practice"
- Change: drop the byte-exact prior-log match. Let the orchestrator invoke
  `contract learn` with a `--finding` it authors, citing the two `key-finding`
  sequence numbers it considers recurrent, and record those citations in the
  appended record.
- Location: `contract learn` in `SKILL.md` and its `main.rs` adapter
- Trade-off: the promotion decision becomes a judgment rather than a string
  comparison. Given that the string comparison never fires, this trades a
  non-functioning objective rule for a functioning subjective one.
- Confidence: `medium`

### Let `contract bootstrap` accept stated commands when it cannot measure them

- Addresses: "`contract bootstrap` does not support a CI-less, non-Cargo
  repository"
- Change: when bootstrap finds no CI workflow and no recognized manifest, have
  it read gate commands and version policy from the repository's `AGENTS.md` or
  `CLAUDE.md`, write them to the tracked contract, measure them, and mark the
  record as stated-not-derived rather than failing.
- Location: bootstrap adapter in `src/main.rs`
- Trade-off: an initial contract that may state a wrong command; the measurement
  step still catches a command that does not run.
- Confidence: `medium`

## No-change decisions

- **Two-tier merge (step squash into milestone, milestone merge-commit into
  main).** It produced a clean, readable history and a single reviewable
  milestone diff. The destruction incident was caused by the orchestrator
  misunderstanding its consequence, not by the model itself. Documenting the
  consequence is the fix; changing the model is not.
- **`codex` authors, Claude gates.** The separation held. Every one of the six
  caught defects was found by a Claude gate against Codex-authored content, and
  no gate rubber-stamped. Worth the extra round trips.
- **Plan verbosity.** The approved plans run 250–280 lines with per-file change
  descriptions, an explicit not-touched list, and exact expected assertion
  values. That density looked like ceremony and was not: the "explicitly not
  touched" list is what kept three executors inside scope, and the exact
  expected error strings are what made the red-first proofs checkable.
- **Round caps and stuck detection keyed on `(node, role)`.** No observed
  problem; the exact role vocabulary is fussy to type but the collision
  avoidance between Phase 1 `m1-s1` and Phase 3 `m1-s1` is real.

## Suggested follow-up

- A `pce doctor` subcommand that audits the physical state a run depends on and
  that the log cannot see: branch containment (every recorded merged step is
  still an ancestor of its milestone branch), version/tag agreement per
  repository, worktree cleanliness, and orphaned worktrees. Every high-severity
  finding in this report would have been caught by such a check, and the branch
  destruction went undetected for several minutes purely because nothing was
  watching.
- An experiment: have the orchestrator take an explicit safety snapshot — a
  `refs/pce/<vision>/<timestamp>` ref per milestone branch — before any git
  write outside a step. On this run the recovery depended on version tags that
  happened to exist for an unrelated reason.
