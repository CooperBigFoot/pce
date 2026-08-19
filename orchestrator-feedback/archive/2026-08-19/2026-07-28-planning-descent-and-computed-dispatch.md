# PCE workflow feedback: planning descent and computed dispatch

- Date: `2026-07-29`
- Orchestrator: `Claude Code (Opus 5), single session, ultracode`
- Run: `planning/2026-07-28-planning-descent-and-computed-dispatch` — 4 milestones, 8 steps, 13 PRs, `main` at `39550ba`
- Outcome: `completed`

## Executive summary

The vision was delivered in full. All seven mechanically-verifiable acceptance criteria pass; criteria 8 and 9 are behavioural and by construction can only be checked on the next run. The workflow's adversarial gates worked, and worked hard: **32 review artifacts against 8 delivered steps**, with defects caught at every altitude including several that no test, schema, or gate could have caught by construction.

Four findings dominate, and they share one root.

**Every gate judges its artifact against the layer immediately above it, so a defect originating above passes all of them.** A plan critic checks the step, a step critic checks the milestone, a milestone critic checks the vision. Nothing checks an artifact against a *consumer authored later*, because that consumer does not exist to be read. This produced the run's most expensive detour: a verb that passed two plan rounds, a PR review with seven mutations, and a live dogfood invocation, and was still architecturally insufficient — discovered only when the next milestone tried to write prose against it.

**The orchestrator is the one actor whose output no gate reads.** Planner output faces a critic; executor output faces a reviewer; a prompt goes straight to a cold agent. This injected three separate defects by three distinct mechanisms (false claim, over-broad constraint, propagated wrong count), each caught downstream by luck of scope rather than by design.

**"Must be tested" is satisfied by a test existing, not by a test that can fail.** Acceptance criterion 3 demands the computation be "unit-tested in isolation" — and isolation is precisely what concealed the run's most severe defect, a caller obligation invisible from inside the unit.

**Mid-run contract changes strand artifacts silently.** Merging milestone 1 flipped the installed graph schema instantly via symlink. Three already-approved artifacts became unparseable while `pce status` continued reporting them `digest-matches`, because that is a claim about bytes, not conformance.

## Evidence reviewed

- `planning/2026-07-28-planning-descent-and-computed-dispatch/events.jsonl` — 108 records: 65 dispatch, 23 key-finding, 13 planning-artifact-approved, 2 delta, 2 escalation-open, 2 escalation-close, 1 repository-contract
- 32 `review-*.md` audit trails and their verdict JSON
- PRs #74–#86 and their diffs against named base/head refs
- `pce status` snapshots at every readiness, merge, and removal decision
- Direct invocations of the delivered `pce ready` against the live event log
- Independent gate re-runs (`cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`) after every execution

## What worked

### Cold re-dispatch on every revision

- Evidence: milestone-critic round 2 refuted a claim from round 1 (`canonical_nodes` "silently discards milestone nodes") that the round-2 *planner* had already copied into the graph as its implementation mechanism. A warm planner or warm critic carrying round 1's reasoning would have inherited it.
- Effect: prevented shipping a change that would have broken `pce status` — the verb this orchestration itself invokes before every merge. Cold dispatch is expensive and earned its cost here.

### Mutation testing as the operational meaning of "vacuity"

- Evidence: four PR reviewers ran mutation suites unprompted or on instruction — 10 mutations on the dispatchable-set computation, 5 on the selector retyping, 10 on the graph selector, 6 on `--graph`. Across ~31 mutations, one survivor, which the reviewer then *proved pre-existing* by applying the same mutation to the base ref.
- Effect: converted "the tests pass" into "the tests can fail", which is what `CONTEXT.md`'s vacuity definition actually requires. One reviewer went further and checked *kill quality*, noting two mutations exited non-zero for an unrelated reason so status-only assertions would have been vacuous.

### The event log as sole durable state

- Evidence: round counts, cap positions, hold status, and provenance were derived from records at every decision; no counter, map, or narrative was maintained. `(node, role)` keying kept Phase 1's `(m1-s1, milestone-planner)` distinct from Phase 3's `(m1-s1, step-plan-writer)` with no collision across 63 dispatches.
- Effect: no state drift, and a mid-run append failure was detectable and repairable precisely because the log was the only authority.

### Runtime graph adaptation

- Evidence: two delta stubs (`m3-s3`, `m3-s4`) were created from findings that surfaced mid-run, each appearing in the projection as a tracked step from its delta record alone. No milestone was re-cut and no second durable representation appeared.
- Effect: absorbed two genuine design corrections without destabilising the approved graph.

### Escalation on a genuine blocker

- Evidence: the m4 step-critic routed a finding upward as unserviceable rather than producing prose that could not be followed, and the orchestrator stopped with an `escalation-open` rather than improvising a resolution.
- Effect: the human chose among four analysed options; the chosen one became a delta with its rejected alternatives recorded.

## Friction and failures

### The startup probe cannot discriminate a stale binary

- Severity: `high`
- Phase: `orientation`
- Observation: the installed `pce` was a build predating the `log` and `status` verbs. The probe printed `usage: pce vision new "<name>"` and exited 1.
- Evidence: `pce status --file … --vision-dir …` → usage string, `EXIT=1`; `ls -la ~/.local/bin/pce` → symlink into `target/release/` dated three weeks earlier; `src/main.rs` at HEAD already contained `parse_log_command`.
- Inference: `SKILL.md` names only NotFound versus permission/bad-path as probe failure causes. A stale binary produces a fourth shape containing no `failed to open event log` prefix, so the discrimination rule does not classify it.
- Impact: an orchestrator following the rule literally would classify this as "no prior run to resume" and then attempt appends with a binary that has no `log` verb. Reachable on any machine where the repo was built before a verb landed.

### The subagent result channel drops completed results

- Severity: `high`
- Phase: `all phases with a Claude gate`
- Observation: five separate times, a completed agent's result was not delivered. Failure modes varied: silent idle (×2), audit file written but no message (×2), and an announcement-without-payload (×1).
- Evidence: the orientation analyst returned two `idle_notification`s with no report; `step-critic-m2-r2`, `plan-critic-m2s1-r1`, and `pr-reviewer-m3s2-r1` each required an explicit `SendMessage` to surrender a verdict they had already produced.
- Inference: `SKILL.md` defines routing for verdicts it *receives* and has no stated recovery for a dispatch that neither fails nor returns.
- Impact: recoverable here because re-asking a completed agent consumes no round. A genuine loss on a capped critic role would force a choice between burning a round and proceeding ungated, and the skill forbids the second.

### Installed schemas are a third self-modification class, and mid-run activation strands artifacts

- Severity: `high`
- Phase: `merge`
- Observation: merging milestone 1 replaced the installed graph schema instantly. Three already-approved artifacts became unparseable by the tooling the same run shipped.
- Evidence: after `git pull`, `grep -c files_touched skills/pce/schemas/graph.schema.json` → 0, `readlink ~/.claude/skills/pce` → the repo path; `milestones.json`, `milestone-1/steps.json`, `milestone-2/steps.json` all carry `files_touched`; `pce ready --graph …/milestones.json` → `graph node 0 has invalid keys: … got depends_on,files_touched,id,repo,summary,title`. `pce status` reported all three `digest-matches` throughout.
- Inference: `vision.md` names two activation classes (skill inert until next invocation; binary live after rebuild). Schemas are a third — live immediately, no rebuild, no reinstall. And `planning-artifact-approved` binds a path to a digest with **no field for which contract version the bytes satisfied**, so `digest-matches` is a claim about bytes, not conformance.
- Impact: this run's own milestone graph is permanently unselectable by its own verb and cannot be repaired — rewriting it invalidates the digest its approval pins. Harmless here only because the orchestrator was the sole consumer.

### Integration branch names collide across visions

- Severity: `medium`
- Phase: `isolation`
- Observation: `milestone-1` already existed from a prior vision, 24 commits behind `main`.
- Evidence: `git branch -a` showed `milestone-1`…`milestone-5` plus `origin/milestone-6`; `git rev-list --count main..milestone-1` → 0 with `milestone-1..main` → 24. The first post-bootstrap status reported `branch: milestone-1, state: present` on a brand-new run.
- Inference: `SKILL.md` namespaces step branches and worktrees by vision slug but names integration branches bare `milestone-<m>`.
- Impact: reusing the stale branch would have cut every step 24 commits behind, silently excluding the discovery-artifact commit this run made as its own precondition — the exact absence vision lines 55 and 90 warned about, arriving through a different door. Also required a fast-forward of `origin/milestone-<m>` before each step PR, or the PR would show unrelated commits.

### The orchestrator's output is never gated

- Severity: `high`
- Phase: `all dispatch composition`
- Observation: three defects injected into cold agents by three distinct mechanisms.
- Evidence: (1) a critic's remark about `run_state.rs` unit tests was generalised onto `tests/event_log_and_run_state.rs`, which uses `.expect(...)` 73 times — caught by the plan critic. (2) "all existing tests must pass unmodified, stop and report otherwise" was carried from a behaviour-preserving refactor to an *additive* change, where adding a struct field forces an E0027 pattern update — the plan critic rated it `critical`, since the executor would have halted correctly with the step incomplete. (3) a critic's test count of 122 was propagated as a "correction" to a plan that had it right at 120; the count came from an unanchored grep matching two JSON fixture literals.
- Inference: the delegation contract constrains what a dispatch must *supply* but nothing constrains the orchestrator's paraphrase of a prior verdict. Every other actor's output faces an adversary; a prompt does not.
- Impact: nil here, by luck of scope in each case. A paraphrase that altered a write-set, a gate command, or a type name would reach execution unchecked.

### Acceptance criteria are satisfiable without falsifiability

- Severity: `high`
- Phase: `step planning`
- Observation: a plan whose *rules* were correct specified tests that could not detect those rules being violated — three independent inversions passed every specified test, and the fix for one introduced two more.
- Evidence: `plan-critic-m2s2-r1` summarised it as "the plan fails on falsification, not on description". A single-`Inconclusive`-dependency setup passes under either check order; an implementation rewriting every non-first narrowed result to `Waiting` passed all eight tests; own-status rule 2 was entirely unexercised, so deleting it would return an inconclusive node as `Dispatchable`. All three produce the collapse ADR 0005 exists to prevent.
- Inference: criterion 3 says coverage must "prove" the inconclusive case is returned as its own case. A test asserting that, in a fixture where nothing else could happen, satisfies the sentence and proves nothing.
- Impact: three plan rounds on one step, consuming the entire cap. The standard that closed it — *would this assertion fail if its rule arm were inverted?* — is latent in `CONTEXT.md` as `vacuity` but is not an obligation any role carries.

### Testing "in isolation" concealed the run's most severe defect

- Severity: `high`
- Phase: `step planning`
- Observation: the planned verb would have failed on **any** real event log, and no specified test could catch it.
- Evidence: `derive_provenance` iterates *every* approved path and returns `MissingCurrentArtifactObservation` when one lacks an observation; the plan fed exactly one. The live log carries **eight** distinct approved paths. Every success fixture in the plan had exactly one approval, so the suite would have been green.
- Inference: the unit under test was correct; the caller's obligation to its callee was not. Isolation is precisely the condition under which that is invisible.
- Impact: would have merged and surfaced only on first human use. Caught by a critic reading the callee's loop — not by any criterion, gate, or test.

### No role runs the delivered artifact

- Severity: `high`
- Phase: `review`
- Observation: five gates approved a verb that a single invocation proved unusable.
- Evidence: after merge, `pce ready --file <live log> …` → `Error: selected artifact …/plan.md is not a conforming graph`. The verb selected the newest approval, which in a real run is almost always a `plan.md` — five of this log's eight approved artifacts are plans.
- Inference: `SKILL.md` assigns reading to every gate and execution to none. The executor runs gates but is scoped to its own worktree and plan; the reviewer reviews refs and may mutation-test but is never told to exercise the shipped surface against real inputs.
- Impact: one delta step (plan, two gate rounds, execute, review, merge). And the *second* consequence — that the verb could still only answer about one graph — was invisible even to dogfooding, because a single invocation against a single graph succeeds.

### Prose edits have no compiler

- Severity: `high`
- Phase: `step planning` (the `SKILL.md` rewrite)
- Observation: three surgical edits whose specifications did not produce the outcomes the plan asserted.
- Evidence: the target reads `merge and readiness state`, where `state` is shared. Removing `readiness state, and ` yields "merge and recovery information", destroying `merge state` — which the same plan sentence promised to preserve. Separately, `A NONE policy omits all three.` stays grammatical after one of three mechanisms is deleted, and is then false. Separately, `…keeps paths relative to that root.` survived as a rule reading a graph field that no longer exists, and the zero-occurrence `files_touched` check could not catch it because the clause says "paths".
- Inference: for code, the compiler and suite are a second opinion on whether an edit did what was meant. For prose there is none — `cargo test` passes identically whether `SKILL.md` says the right thing or quietly says the opposite.
- Impact: all three were invisible to coverage review, which is the natural posture: the plan named every rule correctly and quoted 23 anchors byte-exactly. Only executing each edit against the source exposed them. The artifact is the contract every future run reads.

### Operational friction, lower severity

- **Relative `LOG_PATH`** (`low`, all phases): the shell's working directory persists across calls, so an earlier `cd` silently invalidated the relative path and an append failed. Fail-safe for the write but not for accounting — the dispatch had already been issued, so the record briefly undercounted a round, and caps derive from those records. Mitigated by using absolute paths throughout.
- **Verbatim snapshot quoting does not scale** (`medium`, all phases): `SKILL.md` requires quoting the full `pce status` JSON at four call point types. The snapshot reached ~21KB and grows with the log. I complied in substance — invoking status at every required point and deciding from it — but extracted decision-relevant fields rather than pasting the blob dozens of times. That is a deviation from the rule as written, and it is recorded here rather than glossed.
- **`stop-do-not-improvise` has no route for wrong-but-satisfiable** (`medium`, execution): a plan instruction to place a re-export in non-alphabetical order was achievable, so the executor obeyed it and added `#[rustfmt::skip]` to make it stick — permanently exempting a growing block from formatting in a repo whose `AGENTS.md` delegates formatting to tools. The executor has a `BLOCK` route for *infeasible* and none for *achievable but mistaken*.

## Recommendations

### Add a stale-binary discrimination step to startup

- Addresses: "The startup probe cannot discriminate a stale binary"
- Change: before classifying a failed probe as a fresh run, require the orchestrator to confirm the binary recognises the subcommand — e.g. treat any failure whose output contains the `usage:` banner as an installation fault, not a missing log, and instruct rerunning the installer.
- Location: `skills/pce/SKILL.md`, "Startup and resume", the missing-log discrimination paragraph
- Trade-off: one more branch in a rule that is already the most intricate in the document.
- Confidence: `high`

### Namespace integration branches by vision, or mandate reconciliation

- Addresses: "Integration branch names collide across visions"
- Change: either name them `pce/<vision-slug>/milestone-<m>`, matching the existing step-branch convention, or state that `milestone-<m>` must be reset to the base branch at creation and that `origin/milestone-<m>` must be fast-forwarded before the first step PR.
- Location: `skills/pce/SKILL.md`, Phase 3 isolate and the cross-repo branch rules
- Trade-off: renaming breaks continuity with existing branches; reconciliation keeps the collision latent and relies on the orchestrator remembering.
- Confidence: `high`

### Make falsifiability a stated obligation of plan critics

- Addresses: "Acceptance criteria are satisfiable without falsifiability"; "Testing in isolation concealed…"
- Change: add to the plan-critic obligation that for each required test the critic must ask whether the assertion would fail if its rule arm were inverted or deleted, and that a test whose fixture cannot distinguish the correct rule from a stated wrong one is a blocking finding. `CONTEXT.md` already defines `vacuity`; this makes it a role obligation rather than something a critic must think of unprompted.
- Location: `skills/pce/SKILL.md`, Phase 3 step 1 minimum critic requirements
- Trade-off: longer plan reviews; some risk of critics manufacturing inversions to look thorough.
- Confidence: `high` — four PR reviewers reached this standard on their own, and it caught the most severe defects in the run.

### Require exact resulting text for surgical prose edits

- Addresses: "Prose edits have no compiler"
- Change: when a plan specifies an edit to prose by removal or replacement, require it to state the **exact resulting text**, not only the removal string; and require its critic to execute each edit against the source and compare, rather than confirming the right target was named.
- Location: `skills/pce/SKILL.md`, Phase 3 step 1 plan requirements
- Trade-off: verbose plans for prose-heavy steps.
- Confidence: `high` — two of the three defects would have been impossible to write down under this rule, because the author would have had to produce the broken sentence.

### Add a discriminator to `planning-artifact-approved`

- Addresses: "Installed schemas are a third self-modification class"; both consumer-side workarounds
- Change: add a required kind field (graph vs plan) to the approval payload, and consider recording the contract version the bytes satisfied. The payload is `deny_unknown_fields`, so this invalidates existing records — which is why this run worked around it twice at the consumer instead.
- Location: `crates/core` event-log payload, `skills/pce/SKILL.md` event log contract
- Trade-off: a breaking change to the log format, requiring a migration path the design currently forbids ("no migration or legacy read path").
- Confidence: `medium` — the right fix, at a cost this run could not justify. Recorded twice in deltas so it cannot quietly become permanent.

### Assign someone to run the artifact

- Addresses: "No role runs the delivered artifact"
- Change: where a step's deliverable is an executable surface, require the PR reviewer to invoke it once against realistic inputs and report the result.
- Location: `skills/pce/SKILL.md`, Phase 3 step 5
- Trade-off: not every step has a runnable surface; the rule needs a trigger condition, and a reviewer invoking a tool needs a safe environment.
- Confidence: `medium` — it would have caught one of two defects in this run's verb, not both.

### Soften the verbatim-snapshot requirement

- Addresses: "Verbatim snapshot quoting does not scale"
- Change: require *invoking* status at the four call points and quoting the fields the decision turns on, rather than the entire snapshot; or add a `--brief` projection for decision points.
- Location: `skills/pce/SKILL.md`, "Status authority"
- Trade-off: weakens the audit trail's completeness; a `--brief` mode is new surface to maintain.
- Confidence: `medium`

## No-change decisions

- **The two-tier merge, worktree isolation, and the Codex-authors/Claude-gates split** produced no friction across 8 steps and 13 PRs. Every executor policy held on every step without exception: one conventional commit, correct write-set, no tag, no version change, no attribution footer, `pr-body.md` left uncommitted. No change warranted.
- **Cap of 3** was hit twice (Phase 1, and m2-s2's plan) and both converged on the final round. It is tight but not wrong. Raising it would have bought nothing; the m2-s2 loop converged only because round 3's fix was fully specified by round 2's critic.
- **Round-count derivation from dispatch records** worked correctly across 63 dispatches with no drift. The `(node, role)` keying handled the Phase-1/Phase-3 collision on `m1-s1` exactly as documented.
- **Requiring `evidence` on four record kinds** added real cost to every append and was worth it: this report is assembly rather than recollection, because each finding carries its reproducing command.
- **The `--policy` repeatable argument** looked over-engineered for a single-repo run, but the computation genuinely errors without an entry per candidate repository, so the shape is right.

## Suggested follow-up

- **Runtime-adaptation stubs have no dispatch gate under the new contract.** A stub lives only in a `delta` record, so it is absent from the approved artifact `pce ready` reads and can never be classified. The old readiness rule covered this; milestone 4 deleted it under acceptance criterion 5 and named no replacement. This run created two such stubs. Settle before the next run exercises the new contract: delegate immediately, re-approve into a graph, or give the verb a stub-aware input.
- **Milestone re-entry after `waiting` is implicit.** The deleted dependency-order walk carried the re-invocation trigger; the new rule describes a single `pce ready` invocation. Under-specified rather than contradictory, but worth one explicit sentence.
- **`SERIALIZE_DISPATCHES` is unglossed in prose.** The token appears only inside the quoted usage line; nothing states how a derived per-repository version policy maps onto it versus `NONE`.
- **Acceptance criteria 8 and 9 need a human on the next run** — whether it dispatches a pair concurrently, and whether any graph-critic blocker consists of the critic supplying a code fact the planner could have read. Note this run's descent was partly hand-fed: I supplied explicit read commands and symbol lists in planner prompts, which milestone 4 now makes a rule. The next run is the first real test of whether descent happens without that.
- **Two pre-existing test gaps** surfaced during mutation testing and are unrelated to this vision: no test constructs a non-`NotFound` read failure on an approved artifact path, and no test pins the parser's rejection of malformed `--graph` shapes.
