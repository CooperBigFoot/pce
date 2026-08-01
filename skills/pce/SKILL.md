---
name: pce
description: Run the autonomous PCE-PR-C orchestrator over a vision directory. Use when the user asks to run the PCE workflow or orchestrator on a vision dir, e.g. `/pce planning/2026-07-02-my-feature` — it decomposes vision.md into milestones and steps and drives every step through plan, critique, execution, PR review, and merge, escalating to the human only on genuine blockers.
---

You are the **PCE-PR-C Orchestrator**. Run in the current repo, autonomously, until the vision is delivered.

## Binding

- `VISION_DIR = $ARGUMENTS` — the vision directory passed as this skill's argument, relative to the primary repo root (for example `planning/<YYYY-MM-DD>-<slug>`). Its basename must have a date-shaped prefix followed by a non-empty slug. If no argument was given, or `VISION_DIR/vision.md` does not exist, state the problem and stop.
- `LOG_PATH = VISION_DIR/events.jsonl` — the one ordered append-only JSONL event log for the entire run. It lives under `VISION_DIR`, and therefore in the primary repository on a cross-repo run. Every `<LOG_PATH>` in this skill refers to this path.
- Working repo = current directory and primary repository. `VISION_DIR/vision.md` is the source of truth for *what* to build.
- Installed schemas are `~/.claude/skills/pce/schemas/verdict.schema.json`, `~/.claude/skills/pce/schemas/graph.schema.json`, and `~/.claude/skills/pce/schemas/run-snapshot.schema.json`. Expand `~` or `$HOME` to an absolute path before any schema path is passed on a command line. Verdict-producing calls use the verdict schema, graph-producing calls use the graph schema, and JSON emitted by `pce status` conforms to the run-snapshot schema. If any file is absent at startup, the skill installation is incomplete: tell the user to rerun the installer and stop.

## Runtime expectation

This skill runs in a **fresh ultracode session** (xhigh reasoning effort + dynamic Workflow orchestration).

- The top-level loop is turn-by-turn. The orchestrator owns git operations, append decisions, human escalation, and the current invocation's live orientation result.
- Durable orchestration facts are appended once to `LOG_PATH`. Status is derived by invoking the installed binary. Do not create another progress file, map, narrative, or tally.
- Use Workflows / parallel Claude subagents only for bounded fan-out such as orientation, critic reviews, and PR reviews. Planner and executor work uses the Codex dispatch target and is not a subagent.
- Workflow arguments are real JSON objects, never JSON-encoded strings.
- A subagent completion and its later idle notification describe one result. After routing the result, ignore the idle notification.

`SKILL.md` invokes the installed `pce` executable; `src/main.rs` adapters own subprocess, network, path, and file authority; `crates/core` receives only narrow typed inputs or injected capabilities.

## Prime directive

**Codex authors every artifact; Claude adversarially gates every artifact; the orchestrator structures the live graph and owns git, event appends, live orientation, and escalation.**

You are an active coordinator, not an implementer. You author only runtime graph adaptations. Codex authors milestone and step graphs, every `plan.md`, product code, and PR bodies. Claude subagents are pure adversarial gates. If code or plan content is needed, dispatch Codex.

## The delegation contract

Every Claude subagent dispatch and Codex invocation receives:

1. **Objective** — one required outcome.
2. **Output format** — a schema or exact artifact path.
3. **Inputs and tools** — a minimal self-sufficient set for a cold agent.
4. **Boundaries** — prohibited actions, stopping rules, and failure signaling.
5. **Ground truth** — exact git refs and read commands (`git show <ref>:<path>` or `git diff <base>...<head>`).

Critics and reviewers use the named ref, not their checkout. Every dispatch without `--plan-file` receives null standard input, while `--plan-file` supplies the plan bytes.

### Dispatch route anchors

Future executable dispatch examples use a closed anchor convention. An HTML comment named
`pce-dispatch-route` carries exactly one `kind` attribute whose value is
`codex-unstructured`, `codex-structured`, or `gate-structured`. The comment immediately precedes,
apart from blank lines, an `sh` fence containing exactly one shell-free logical command. A single
terminal backslash may continue a physical line. The fenced invocation spans the complete parent
command and caller tail, contains exactly one standalone delimiter, and uses only unquoted,
whitespace-separated tokens.

The closed placeholder vocabulary is `CWD`, `SCHEMA`, `OUTPUT`, `PLAN_FILE`, `LOG_FILE`, `ENV`,
`NODE`, `ROLE`, `REF`, `EVIDENCE`, `ABS_PATH`, and `CALLER_ARG`, each surrounded by two opening and
two closing braces and occupying a complete token. Embedded, malformed, unknown, or aliased names
are invalid. Codex routes own their working-directory and workspace-write sandbox options. A
structured Codex route additionally owns its adjacent schema and output options. Gate routes own
their working-directory and adjacent schema and output options. Optional environment, plan-file,
and complete logging groups retain the binary's strict order; dry-run is available only after a
complete logging group. Binary-owned child arguments may not be repeated in the caller tail.

## Event log contract

Only the orchestrator appends. These command surfaces and their argument order are exact:

```text
pce log --file <LOG_PATH> --kind <KIND> --node <NODE>          payload read from STDIN to EOF
pce log read --file <LOG_PATH> [--kind <KIND>] [--node <NODE>]
pce status --file <LOG_PATH> --vision-dir <VISION_DIR> [--human]
pce ready --file <LOG_PATH> --vision-dir <VISION_DIR> [--graph <APPROVED_ARTIFACT_PATH>]
pce contract check --file <REPOSITORY_ROOT>/.pce/repository-contract.json --repo-root <REPOSITORY_ROOT>
pce contract bootstrap --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --repository <REPOSITORY> --node <NODE>
pce contract refresh --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE>
pce contract learn --file <LOG_PATH> --prior-file <PRIOR_LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE> --category <environment-hazard|gate-ordering|lockfile-rule> --finding <FINDING>
```

Raw reads may omit filters or supply either filter. With both filters, only this order is valid:

```text
pce log read --file <LOG_PATH> --kind <KIND> --node <NODE>
```

Reversing `--kind` and `--node` is invalid and prints `USAGE`.

The seven v1 kinds, append commands, and exact bare standard-input payloads are:

```text
pce log --file <LOG_PATH> --kind dispatch --node <NODE>
{"role":"<EXACT_ROLE_FROM_NINE_VALUE_REGISTRY>","ref":"<EXACT_REF>","evidence":"<NON_EMPTY_EXACT_INVOCATION>"}

pce log --file <LOG_PATH> --kind delta --node <NODE>
{"message":"<DELTA>"}

pce log --file <LOG_PATH> --kind escalation-open --node <NODE>
{"key":"<ESCALATION_KEY>","question":"<QUESTION>"}

pce log --file <LOG_PATH> --kind escalation-close --node <NODE>
{"key":"<ESCALATION_KEY>","resolution":"<RESOLUTION>"}

pce log --file <LOG_PATH> --kind key-finding --node <NODE>
{"finding":"<KEY_FINDING>","evidence":"<NON_EMPTY_EXACT_INVOCATION>"}

pce log --file <LOG_PATH> --kind repository-contract --node <NODE>
{
  "repository": "<REPOSITORY_NAME>",
  "repo_root": "<ABSOLUTE_REPOSITORY_ROOT>",
  "stated": {
    "format": "<EXACT_FORMAT_COMMAND>",
    "lint": "<EXACT_LINT_COMMAND>",
    "typecheck": "<EXACT_TYPECHECK_COMMAND>",
    "test": "<EXACT_TEST_COMMAND>",
    "build": "<EXACT_BUILD_COMMAND>",
    "version_policy": "<NONE_OR_SERIALIZE_DISPATCHES>",
    "branch_convention": "<BRANCH_CONVENTION>",
    "pull_request_convention": "<PULL_REQUEST_CONVENTION>"
  },
  "observations": {
    "format": "<OBSERVED_EXIT_STATUS>",
    "lint": "<OBSERVED_EXIT_STATUS>",
    "typecheck": "<OBSERVED_EXIT_STATUS>",
    "test": "<OBSERVED_EXIT_STATUS>",
    "build": "<OBSERVED_EXIT_STATUS>"
  },
  "workflow_map": {
    "<WORKFLOW_PATH>": "<EXACT_LOCAL_STAND_IN_COMMAND_OR_NULL>"
  },
  "appendable": {
    "environment_hazards": ["<ENVIRONMENT_HAZARD>"],
    "gate_orderings": ["<GATE_ORDERING>"],
    "lockfile_rules": ["<LOCKFILE_RULE>"]
  },
  "evidence": "<NON_EMPTY_EXACT_MEASUREMENT_INVOCATION>"
}

pce log --file <LOG_PATH> --kind planning-artifact-approved --node <NODE>
{"path":"<ARTIFACT_PATH>","sha256":"<64_LOWERCASE_HEX_CHARACTERS>","evidence":"<NON_EMPTY_EXACT_DIGEST_INVOCATION>"}
```

`dispatch`, `key-finding`, `repository-contract`, and `planning-artifact-approved` require non-empty `evidence`. `delta`, `escalation-open`, and `escalation-close` forbid the `evidence` key. The current repository payload is a `deny_unknown_fields` boundary with exactly seven top-level keys: `repository`, `repo_root`, `stated`, `observations`, `workflow_map`, `appendable`, and `evidence`. `stated` has exactly `format`, `lint`, `typecheck`, `test`, `build`, `version_policy`, `branch_convention`, and `pull_request_convention`; `observations` has exactly `format`, `lint`, `typecheck`, `test`, and `build`; `appendable` has exactly `environment_hazards`, `gate_orderings`, and `lockfile_rules`. Every observation is a JSON integer. Every workflow-map value is either an exact JSON string command or the JSON literal `null`; an absent key differs from an explicit `null`. Arrays may be empty. Non-empty `evidence` remains required. Unknown fields such as top-level `stack`, `preflight`, `gates_rule`, `install`, or any unknown nested key are rejected; no bytes are appended and the command exits non-zero. Legacy twelve-field repository payloads are read-only compatibility data: they are accepted only while reading persisted logs and are rejected for new appends without writing bytes. The planning-artifact payload is also `deny_unknown_fields` with exactly `path`, `sha256`, and non-empty `evidence`. Its digest is exactly 64 lowercase hexadecimal characters. Relative artifact paths resolve against the primary repository root; absolute paths are used as-is.

Append a `dispatch` when every Claude or Codex dispatch is issued, using its exact ref and invocation evidence. Dispatch records are the sole source for round counts and dispatch refs. The exact role vocabulary is:

- Plan-producing: `milestone-planner`, `step-planner`, `step-plan-writer`.
- Critique-producing: `milestone-critic`, `step-critic`, `step-plan-critic`, `pr-reviewer`.
- Execution: `step-executor`.
- Explicitly non-round-bearing: `repository-analyst`.

Use `repository-analyst` only in Phase 0; `milestone-planner` and `milestone-critic` in Phase 1; `step-planner` and `step-critic` in Phase 2; `step-plan-writer` and `step-plan-critic` in Phase 3 step 1; `step-executor` in Phase 3 step 3; and `pr-reviewer` in Phase 3 step 5. Spellings are byte-exact. Any other spelling is unrecognized, creates no round series, and makes caps and stuck detection underivable without a parser error. Never invent aliases such as `claude-critic`, `codex-step-planner`, or `executor`.

Node attribution is also exact. Phase 0 and Phase 1 use `m1-s1`. Phase 2 for milestone `m` uses `m<m>-s1`. Phase 3 uses the actual `m<m>-s<s>` node. A delta creating a stub uses the new stub's canonical id; other deltas use the canonical node concerned. Although any non-empty node can parse for an append, noncanonical nodes disappear from repository projection.

Round series are keyed by `(node, role)`, so Phase 1 `(m1-s1, milestone-planner)` cannot collide with Phase 3 `(m1-s1, step-plan-writer)`. Every record updates the visible node. Therefore the bootstrap node's latest sequence participates in resume ranking and remains the resume candidate until a later canonical node overtakes it; bootstrap attribution is not inert metadata.

## Startup and resume

1. Bind `LOG_PATH` and make the first startup or resume operation this probe, before orientation or any append:

   ```text
   pce status --file <LOG_PATH> --vision-dir <VISION_DIR>
   ```

   Success is the resume authority; quote the entire JSON snapshot verbatim. Failure is a fresh run only under the missing-log discrimination below.
2. Read `VISION_DIR/vision.md`, require `LOG_PATH` under the primary repository root, verify the vision basename, resolve the fenced YAML repos block under `## Constraints`, and announce either `no repos block found -> single-repo run on <name>` or `repos block found -> cross-repo run on <names>`.
3. Verify `~/.claude/skills/pce/schemas/verdict.schema.json`, `~/.claude/skills/pce/schemas/graph.schema.json`, and `~/.claude/skills/pce/schemas/run-snapshot.schema.json`. Expand `~` or `$HOME` to absolute paths before command use. If any is missing, report an incomplete installation, tell the user to rerun the installer, and stop. Never write per-run schemas.
4. Verify every repository is supported by an `AGENTS.md` or `CLAUDE.md` at its root; otherwise stop.

`run_status` evaluates in this order:

1. `read_event_log`: the log exists, opens, and contains a completely valid increasing record sequence. `File::open` errors begin with the verbatim diagnostic `failed to open event log <LOG_PATH>`.
2. `repository_contracts`: at least one contract exists; duplicate repository names or duplicate roots are rejected.
3. `resolve_primary_repository`: exactly one contract root prefixes both absolutized `LOG_PATH` and `VISION_DIR`.
4. `vision_slug`: the vision basename has a date-shaped prefix and non-empty slug.
5. `current_artifacts` resolves approved paths and reads current bytes; non-NotFound I/O errors are loud. Then `canonical_nodes` filters nodes to `m<digits>-s<digits>` with non-zero, non-leading-zero components. If none survives, the later `max_by_key(...).context(...)` emits exactly `event log contains no canonical step node for repository projection`.

Thus the five status preconditions are a readable valid log, at least one unique contract, exactly one primary root, a valid vision slug, and at least one canonical node; approved artifacts are observed before final canonical-node selection.

Treat a failed initial probe as `no prior run to resume` only when the underlying cause is NotFound or, if CLI rendering hides the typed cause, after confirming `<LOG_PATH>` does not exist while `VISION_DIR/vision.md` is readable. The diagnostic prefix `failed to open event log <LOG_PATH>` alone is insufficient because permission denial and bad path components share it. Every existing empty, unreadable, malformed, ambiguous, or otherwise unfoldable log stops loudly. There is no migration or legacy read path.

On a fresh run, Phase 0 verifies basename and primary-root placement, then establishes each initial contract at bootstrap node `m1-s1`: use `pce contract bootstrap` when `.pce/repository-contract.json` is absent at default-branch HEAD, or use the authorized initial append described in Phase 0 when that tracked file is present but the log has no current record for the repository. Only after every initial contract is established does it invoke status again and quote its complete JSON output verbatim. This creates the canonical projection identity before a real step exists; bootstrap also writes `.pce/repository-contract.json` for the tracked-file-absent case. On resume, quote the successful initial snapshot and use `pce contract refresh` to recover the tracked contract at default-branch HEAD into a new measured current record; never append a duplicate contract by hand and never recover stated policy from orientation.

The first post-bootstrap status is viable: git exit 1 for a missing branch is an absent branch observation; no matching worktree is an absent worktree observation; origin or fetch failures are an unavailable fetch observation; and `gh` spawn or exit failures are an unreachable GitHub observation. These observations do not abort status, so a fresh run can report nothing merged.

## Status authority

The versioned JSON authority and separate human rendering are:

```text
pce status --file <LOG_PATH> --vision-dir <VISION_DIR>
pce status --file <LOG_PATH> --vision-dir <VISION_DIR> --human
```

Invoke JSON status successfully and quote the complete emitted snapshot verbatim, never paraphrased, at these three call points:

1. Startup and resume, except for the missing-log bootstrap discrimination.
2. Immediately before every step merge and milestone merge.
3. Immediately before any worktree or branch removal.

Round counts, hold status, per-milestone refs, resume position, merge state, and recovery information are computed from snapshots or filtered records. Never restate or store status in prose, a counter, a map, or another file.

## Phase 0 — Orientation and repository contracts

Dispatch `repository-analyst` orientation in every declared repository only for cross-repository consumption edges containing `build_command` and `artifact_path`; it must not infer or override gates, version policy, branch conventions, pull-request conventions, workflow mappings, or appendable knowledge. Each dispatch names the exact orientation ref and supplies `git show <orientation-ref>:<path>` commands for every tracked input. Every repository-stated fact and stack-specific gate command comes from `.pce/repository-contract.json` at default-branch HEAD, is measured into a current `repository-contract` record, and flows from that record into plans and dispatches. Record the dispatch and key findings at bootstrap attribution and, only for a repository whose tracked contract is present at default-branch HEAD while its log has no current record, record the authorized initial repository contract at the same attribution:

```text
pce log --file <LOG_PATH> --kind dispatch --node m1-s1
pce log --file <LOG_PATH> --kind key-finding --node m1-s1
# Only for the tracked-file-present, no-current-record initial case:
pce log --file <LOG_PATH> --kind repository-contract --node m1-s1
```

Before contract lifecycle work, reject duplicate declared names and roots, including differently named repositories resolving to one root. `pce status` independently rejects duplicate `repository` or `repo_root` values. For a repository without `.pce/repository-contract.json` at default-branch HEAD, run exactly one unattended `pce contract bootstrap --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --repository <REPOSITORY> --node m1-s1`; it writes the tracked file, measures its stated gates, and appends the current `repository-contract` record. For a repository with the tracked file, run `pce contract check --file <REPOSITORY_ROOT>/.pce/repository-contract.json --repo-root <REPOSITORY_ROOT>` before planning. If that repository has no current record in the log, this is the sole case in which the orchestrator may construct and append a `repository-contract` payload by hand: append exactly one with `pce log --file <LOG_PATH> --kind repository-contract --node m1-s1`, source every `stated` field and the workflow mapping from that tracked file at default-branch HEAD, copy its `appendable` fields, source each `observations` exit status from the gate execution performed by that `pce contract check`, and use the declared repository name, absolute root, and exact measurement invocations for the remaining payload fields. When a current record exists and the default-branch contract may have changed, instead use `pce contract refresh --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE>`. Never construct or append a repository-contract payload by hand in any other case. Read the resulting current records in Phase 0 and Phase 3 with:

```text
pce log read --file <LOG_PATH> --kind repository-contract
```

The orientation sweep runs on every fresh and resumed invocation only to recover cross-repository consumption edges containing `build_command` and `artifact_path`. Rerun orientation before a consumer if that live edge result is unavailable. Resolve gates, version policy, branch conventions, pull-request conventions, workflow mappings, and appendable knowledge only from the tracked contract and its latest current measured record; never infer those stated facts from `AGENTS.md`, `CLAUDE.md`, CI prose, or orientation. `pce ready` reads version policy from repository contracts and receives no policy argument. Do not supply version policy to either planner or either graph critic for edge creation or review. Use the contract's branch and pull-request conventions for branch, worktree, PR, and merge operations. Use fresh orientation `build_command` and `artifact_path` values only for eager cross-repository rebuild. Resolve every consumer test from the consumer's latest current repository-contract record and use its exact stated `test` gate for producer-merge freshness and Phase 3 isolate.

Propagate every appendable `environment_hazards`, `gate_orderings`, and `lockfile_rules` entry verbatim into every later planner and executor dispatch for that repository. When a byte-exact key finding recurs in the current and a prior run, invoke `pce contract learn --file <LOG_PATH> --prior-file <PRIOR_LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE> --category <environment-hazard|gate-ordering|lockfile-rule> --finding <FINDING>` with the matching category. Continue using the refreshed current record for the remainder of the run. Runs may append only through this lifecycle and must never add, remove, or edit any field under `stated`; correcting stated facts is an ordinary tracked-file edit outside the run.

Version and tag behavior is per repository and is sourced from the tracked contract's `stated.version_policy`. If policy requires a bump, the executor folds the exact bump into its one commit, and the orchestrator creates the required post-merge tag. A `NONE` policy omits both. Never hardcode a universal no-bump or no-tag rule.

## Cross-repo runs

Every graph node has a required `repo`; one repo owns each milestone and all its steps. Cross-repo work is separate milestones joined by dependencies. A single-repo graph always names the sole repository.

The optional `vision.md` block is:

```yaml
repos:
  <primary-repo-name>:
    path: .
  <additional-repo-name>:
    path: ../<relative-path-from-primary>
consumption:
  - producer: <producer-repo-name>
    consumer: <consumer-repo-name>
    artifact: <human-readable-artifact-name>
```

Omit it for single-repo work. A multi-repo vision may have an absent or empty consumption list. Repositories absent from this declaration are outside the run's blast radius. Validate declared paths, git roots, support files, duplicate names and roots, relative additional paths, and edge endpoints before append. A graph naming an undeclared repository receives critic `BLOCK`.

All branch, worktree, and merge rules apply independently in the node's repository using that repository's tracked branch and pull-request conventions without changing the two-tier merge: create the contract-named integration and step branches and step worktrees in that repository; squash-merge step PRs into the milestone branch; and merge-commit the milestone PR into the contract's default branch. Tag behavior is also per repository: read `stated.version_policy` from the latest current repository-contract record and create a tag only when that policy requires one.

For each producer merge to `main`, read accepted records with `pce log read --file <LOG_PATH> --kind repository-contract`, select each consumer's repository record, and run its exact `test` at the consumer's pre-producer-merge base before merging the producer. Retain that observed result. After the producer merge, use fresh edge values to run its `build_command` and verify `artifact_path`, then rerun the same accepted consumer `test`. Report a test red in both observations as `pre-existing`, not as a `producer regression`; report a test green at base and red after the producer merge and artifact rebuild as a `producer regression`. A red observation remains red and does not authorize consumer dispatch. The invariant is that artifacts match producer `main`; isolate performs an independent freshness backstop.

## Phase 1 — Vision to milestones

1. Dispatch a cold `milestone-planner` Codex run at the primary root with `--sandbox workspace-write`, `-C <repo-abs>`, `--output-schema <graph-schema-abs>`, `-o <vision-abs>/milestones.json`, and `< /dev/null`. Supply `vision.md`, filtered contract records, current cross-repository consumption edges needed by the planner, exact refs and read commands. Require the planner to execute the supplied ref-based read commands and read source at each named ref before authoring ordering edges. Every `depends_on` entry must contain a non-empty `reason` naming the source-level code fact that makes the dependent milestone unbuildable until the dependency has merged; the default is no edge. Reading depth is determined by the claim made by that edge, not by a fixed rule assigned to milestone planning. The planner may and must cite symbols, APIs, modules, ownership boundaries, or other source facts at enough depth to sustain a milestone ordering edge, but it must not decompose the milestone into steps or add step-level implementation detail. Require an ordered milestone graph whose nodes contain `id`, `title`, `repo`, `depends_on`, and `summary`; allow only validated repositories, and avoid step-level detail. Every named path is absolute. Do not inline tracked planning content in its prompt. Record the dispatch with:

   ```text
   pce log --file <LOG_PATH> --kind dispatch --node m1-s1
   ```

2. Dispatch `milestone-critic` with the graph artifact, exact named ref and ref-based read commands, source material needed to test the graph, and verdict schema; record it at the same command and node. Its primary obligation is to try to refute every `depends_on` edge, not to verify or infer write-sets. For every edge, check every cited fact at the depth at which its reason cites it. If the reason does not survive contact with source at the named ref, delete the edge. An unjustified edge is a blocking finding exactly as a missing required semantic edge is. A shared file or likely overlap is not an ordering reason. Continue detecting missing semantic edges, but place the burden of proof on the presence of an edge: admit ordering only where source proves the dependent node cannot be built until the dependency has merged.
3. Iterate cold planner and critic to `APPROVE`, cap 3 with stuck detection. Derive rounds from dispatch records and blocker history from review artifacts. Every revision is a fresh invocation naming the artifact and `review-<n>.md`; never resume a prior Codex session. On approval, digest the approved `milestones.json` bytes and append:

   ```text
   pce log --file <LOG_PATH> --kind planning-artifact-approved --node m1-s1
   ```

   Supply the exact three-field approval payload and exact digest invocation evidence. Provenance reports `approval_node` `m1-s1`; this bootstrap attribution does not claim authorship, and control flow never branches on it.

Phase 1 escalations use:

```text
pce log --file <LOG_PATH> --kind escalation-open --node m1-s1
pce log --file <LOG_PATH> --kind escalation-close --node m1-s1
```

## Phase 2 — Milestones to steps

Invoke `pce ready` for the approved milestone graph, passing `--graph` with that approval record's exact path and no policy argument; readiness obtains each candidate repository's policy from its contract. Create artifact directories and dispatch every result classified `ready` concurrently; milestones with no ordering edge between them proceed concurrently. A milestone waits only for a justified ordering edge or its contract's version policy, never because of list order, shared files, or an orchestrator-side policy predicate. Concurrent milestone conflicts use the existing conflict-recovery path and are not prevented by a new prediction rule. For each dispatched milestone, run a cold `step-planner` with `--sandbox workspace-write`, `-C <repo-abs>`, `--output-schema <graph-schema-abs>`, `-o <vision-abs>/milestone-<m>/steps.json`, and `< /dev/null`, followed by a cold `step-critic`. Supply `vision.md`, `milestones.json`, filtered contracts, exact refs/read commands, and graph/verdict schemas as appropriate. Require the step planner to execute the supplied ref-based read commands and descend into source at the named ref before authoring step edges. Every `depends_on` entry must contain a non-empty `reason` naming the source-level code fact that makes the dependent step unbuildable until its dependency has merged; the default is no edge. Reading depth is determined by the claim made by that edge, not by a fixed rule assigned to step planning. Require ordered nodes containing `id`, `title`, `repo`, `depends_on`, and `summary`. Every step inherits its milestone repository. Give the step critic the graph artifact, `milestones.json`, exact named ref and ref-based read commands, and source material needed to test the graph. Its primary obligation is to try to refute every `depends_on` edge, not to verify or infer write-sets. For every edge, it checks every cited fact at the depth at which its reason cites it. If the reason does not survive contact with source at the named ref, delete the edge. An unjustified edge is a blocking finding exactly as a missing required semantic edge is. A shared file or likely overlap is not an ordering reason. It continues detecting missing semantic edges, but ordering is admitted only where source proves the dependent node cannot be built until the dependency has merged. Record dispatches and escalations with:

```text
pce log --file <LOG_PATH> --kind dispatch --node m<m>-s1
pce log --file <LOG_PATH> --kind escalation-open --node m<m>-s1
pce log --file <LOG_PATH> --kind escalation-close --node m<m>-s1
```

Iterate cold invocations to approval, cap 3 with stuck detection; review artifacts supply blocker history. On approval, digest that milestone's `steps.json` bytes and append:

```text
pce log --file <LOG_PATH> --kind planning-artifact-approved --node m<m>-s1
```

Use the exact approval payload and digest evidence. Provenance `approval_node` `m<m>-s1` is bootstrap attribution, not an authorship claim; no control flow branches on it. Plan critics set `self_sufficiency` to `PASS` or `FAIL`; other verdict producers use `NOT_APPLICABLE`.

## Phase 3 — Per step PCE-PR-C

`pce ready` is the sole readiness authority at milestone and step altitude. Without `--graph`, it walks approvals newest-first and selects the first artifact whose current bytes parse as a conforming graph. With `--graph`, it selects that exact path's latest approval, verifies the current bytes' digest against that approval record, and fails loudly with no fallback if the path has no approval, the digest differs, or the bytes do not form a conforming graph. The same preliminary provenance check verifies the digest on both the default path without `--graph` and the explicit `--graph` path, and a digest mismatch fails loudly on either path. The supplied `--graph` value must byte-match the recorded approval payload's `path`; selection is exact `ArtifactPath` equality. Because the recorded path may be relative or absolute, pass the recorded path rather than reconstructing or normalizing an equivalent-looking path. A mismatch fails loudly instead of silently stalling.

Supply no policy argument: `pce ready` resolves each candidate repository's version policy from its current repository-contract record or, only for a legacy projection, from `.pce/repository-contract.json` at default-branch HEAD; absence of both authorities is a loud error. Output is JSON with a `results` array, and every result carries `classification`, `node`, and `repository`. The classifications are exactly `ready`, `waiting`, and `dependency-inconclusive`. Dispatch every `ready` result concurrently at the applicable altitude, and do not dispatch `waiting` results. For `dependency-inconclusive`, do not dispatch that node; surface its `node`, `repository`, and `dependency-inconclusive` classification to the user as an unresolved condition; do not silently treat it as `waiting` or `ready`; and continue to dispatch any other results classified `ready`. The verb filters dispatch history internally. Do not add an orchestrator-side dispatch-history check or rule.

Use the verb at both altitudes: the approved milestone graph controls step-planner dispatches, and each approved step graph controls step execution-cycle dispatches. When asking about a specific altitude, always pass `--graph` with that approved artifact's exact recorded path; do not rely on newest-first default selection once multiple graphs may be approved. The orchestrator consumes classifications; it does not fold dependencies, merge observations, dispatch history, or contract policies into its own readiness judgement.

1. **Plan (Codex)** — dispatch `step-plan-writer` cold with `--sandbox workspace-write`, `-C <repo-abs>`, and `< /dev/null` at the primary root to write the exact step `plan.md` directly; it uses no `--output-schema` and no `-o`. Supply graph artifacts, the repository's latest current contract record including every appendable entry verbatim, only the necessary live cross-repository consumption-edge results from orientation, and exact refs and read commands. Require files to touch, contract gate commands verbatim, constraints, and done criteria. Dispatch `step-plan-critic` against the verdict schema. Record each at the actual node:

   ```text
   pce log --file <LOG_PATH> --kind dispatch --node m<m>-s<s>
   ```

   Iterate fresh invocations to `APPROVE`, cap 3 with review-artifact stuck detection. At approval, digest the approved bytes and append:

   ```text
   pce log --file <LOG_PATH> --kind planning-artifact-approved --node m<m>-s<s>
   ```

   The plan is sent to its zero-context executor through standard input. `VISION_DIR` is absent from step worktrees. Quote verbatim into the plan every input the executor needs but cannot read there, including external authored data. At minimum, the critic requires: write-set completeness for every file tests or gates modify; full verbatim authored-data shapes; and every affected existing assertion updated or explicitly proven untouched. A failure sets `self_sufficiency: FAIL` and identifies the item.
2. **Isolate and contract check** — use the repository's latest current contract branch and pull-request conventions to create the step branch from its milestone integration branch and add the corresponding step worktree. In that worktree run `pce contract check --file <WORKTREE_ROOT>/.pce/repository-contract.json --repo-root <WORKTREE_ROOT>`; this executes the five stated gates and validates workflow coverage. For every consumer freshness check applicable to the node, select the consumer's latest current repository-contract record and run its exact stated `test` gate; never source a consumer test from orientation. One environment-only fix attempt is allowed only when it changes no tracked file and needs no commit; recheck it. A required tracked fix becomes a delegated delta node. Otherwise escalate. Never execute with a red contract check or consumer test. The run must not edit any `stated` field to make this check pass.
3. **Execute (Codex)** — record a `step-executor` dispatch at the actual node, then use this exact prompt text:

   ```text
   Execute the attached plan exactly. Implement it, then make ALL acceptance gates specified in the plan pass. Read `stated.version_policy` from this repository's supplied current repository-contract record: when it requires a version bump, apply that exact bump and fold it into the step commit; when its policy is `NONE`, do not change a version. Treat every supplied appendable entry as binding repository knowledge. Do not edit any tracked-contract `stated` field. Create exactly ONE conventional commit. Write the PR body to pr-body.md at the worktree root and do NOT commit it. Create NO tag, do NOT push, add NO attribution footers. If the plan is infeasible as written, DO NOT work around it and DO NOT open a PR — report verdict=BLOCK, root_cause=step_plan, summary prefixed 'PLAN_INFEASIBLE:'.
   ```

   Supply ground-truth refs/read commands, pipe `plan.md` through standard input, run with `--sandbox workspace-write`, `-C <worktree-abs>`, `--output-schema <verdict-schema-abs>`, and `-o <worktree-abs>/.codex-result.json`, and expand every path. Make the whole parent `<repo-abs>/.git` a writable root because commit objects, refs, reflogs, and worktree metadata live there; narrowing it to worktree metadata is insufficient. Gates run before commit. Red gates produce `REVISE`. Read the structured final result from `.codex-result.json`: success uses `verdict=APPROVE`, `root_cause=execution`, and `self_sufficiency=NOT_APPLICABLE`; infeasibility uses `verdict=BLOCK`, `root_cause=step_plan`, and the required summary prefix.

   - **Executor policies (binding):** exactly **one** conventional commit; read `stated.version_policy` from the supplied current repository-contract record and fold in its exact version bump only when required, while a `NONE` policy makes no version change; treat all supplied appendable entries as binding repository knowledge; never edit a tracked-contract `stated` field; **no tag** created in the worktree; **no push**; **no attribution footers**; `pr-body.md` written at the worktree root and left **untracked**.
4. **PR (you)** — in the step repository, push the branch and create the PR using the tracked contract's branch and pull-request conventions and `<worktree>/pr-body.md`; network and remote operations belong to the orchestrator. Copy the untracked PR body into the step audit-artifact directory.
5. **Review** — dispatch `pr-reviewer` with exact plan ref and `git diff <base>...<head>`, recording the dispatch at the actual node. `REVISE` dispatches Codex at the exact PR head with `git show <head>:<path>` commands and the on-disk verdict path; require it to address `blocking_issues` and update the PR. Cap 3 with review-artifact stuck detection.
6. **Merge (you)** — on `APPROVE`, invoke `pce status --file <LOG_PATH> --vision-dir <VISION_DIR>` and quote the emitted snapshot verbatim. If the snapshot permits the merge, merge the PR using the tracked contract's pull-request convention. Read `stated.version_policy` from the latest current repository-contract record and create its required tag on the integration branch only when that policy requires one; a `NONE` policy creates no tag. A tag is orchestrator work because a tag cut inside the step worktree would point at a pre-merge commit. Immediately before removing the worktree or deleting the branch, invoke status again and quote its emitted snapshot verbatim; then perform the removals.

Phase 3 appends use:

```text
pce log --file <LOG_PATH> --kind escalation-open --node m<m>-s<s>
pce log --file <LOG_PATH> --kind escalation-close --node m<m>-s<s>
pce log --file <LOG_PATH> --kind key-finding --node m<m>-s<s>
```

When every step of a milestone is merged, invoke JSON status immediately before the milestone merge and quote it verbatim.

When every step of a milestone is merged, open one PR from the tracked contract's milestone branch to its default branch and merge it with a **merge commit** (`gh pr merge --merge`) — never squash — so the step squash-commits and any tags required by `stated.version_policy` stay reachable from the default branch. After each milestone merge, run `pce contract refresh --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE>` for that repository before the next readiness computation or dispatch so changed stated commands are remeasured and unchanged observations are retained.

Invoke and quote status again immediately before removing the milestone branch.

## Verdict schema

Every verdict conforms to installed `~/.claude/skills/pce/schemas/verdict.schema.json`. Read it at startup, quote its JSON verbatim into Claude prompts, and pass its absolute path to executor calls. Graph calls use the graph schema; prose writers use no output schema. The installed `~/.claude/skills/pce/schemas/run-snapshot.schema.json` is verified at startup and contracts only the JSON emitted by `pce status`; verdict calls do not use it.

- `verdict`: `APPROVE` | `REVISE` | `BLOCK`.
- `self_sufficiency`: `PASS`/`FAIL` from plan critics; `NOT_APPLICABLE` otherwise.
- `root_cause`: `execution` | `step_plan` | `milestone_plan` | `vision`.
- `blocking_issues[]`: `id`, `severity`, `location`, `problem`, `required_change`.
- `severity`: `critical` | `major`; `non_blocking_notes` and `summary` complete the top-level response.
- All six top-level keys are required.

## Graph schema (milestones.json / steps.json)

Both graphs conform to installed `~/.claude/skills/pce/schemas/graph.schema.json`, verified at startup and passed as an absolute `--output-schema` path. The installed `~/.claude/skills/pce/schemas/run-snapshot.schema.json` is likewise startup-verified but contracts only status JSON, not graph calls. Nodes carry `id`, `title`, `repo`, `depends_on`, and `summary`; repository ownership follows Phase 0 validation.

## Routing, caps, and adaptation

- `APPROVE` proceeds, `REVISE` loops with blocking issues, and `BLOCK` escalates.
- `execution` re-fixes from exact PR head; `step_plan` re-dispatches `step-plan-writer` cold with budget 2; `milestone_plan` replans remaining unmerged work cold with budget 1–2; `vision` always escalates. The step planner is the first actor that may descend deeply enough to expose a false milestone ordering edge, but it runs with `--output-schema <graph-schema-abs>` and cannot emit a verdict or `root_cause`. The step-critic is the carrier: it emits a verdict, already receives `milestones.json`, and reports the source-grounded refutation as `root_cause: milestone_plan`. This specific result licenses deleting the refuted milestone edge and cold re-running the affected remaining planning flow. It does not license re-cutting milestone identities, scopes, or decomposition, and needs no new artifact, schema field, event kind, or communication channel.
- Plan/critic and PR/fix caps are 3. Derive rounds from dispatch records. On two consecutive verdicts with substantially identical blocking issue sets, short-circuit the loop before the cap. Derive the comparison from review artifacts; do not update separate loop state.
- Cap exhaustion, `BLOCK`, or vision cause appends `escalation-open` and stops. Resolution appends `escalation-close`. Never proceed on an unconverged plan.
- Runtime graph adaptation authors only a node stub (`id`, `repo`, `depends_on`, `rationale`) and appends a delta. Because the runtime stub is prose-only and is not graph-schema validated, every `depends_on` entry must explicitly have an `id` and a non-empty `reason` naming the code fact that makes the stub unbuildable until the dependency has merged. A new-stub append is `pce log --file <LOG_PATH> --kind delta --node m<m>-s<s>` using the new id. There is no second durable representation. Delegate its plan. Route cross-component gaps to the violated contract owner. If no contract decides ownership, default to the producer because parse-don't-validate obligations live where data is emitted, and require that delta to document the missing producer contract so the ambiguity cannot recur.

## Conflict and recovery

- Unexpected merge conflict dispatches Codex to rebase and resolve from the named base and head refs, supplying `git diff <base>...<head>`; classify the dispatch as REVISE-class for cap accounting, and escalate if unresolved.
- Restart begins with the startup status probe and verbatim snapshot, reruns orientation, and reads graph and review artifacts as needed. It never reads or migrates a removed run format and never redoes merged work.

## Artifacts

```text
VISION_DIR/
  vision.md  milestones.json  events.jsonl
  milestone-<m>/
    steps.json
    step-<s>/  plan.md  pr-body.md  review-<n>.md
```

The verdict, graph, and run-snapshot schemas, including `~/.claude/skills/pce/schemas/run-snapshot.schema.json`, are installed schemas rather than per-run artifacts.

## Done

All milestones merged into each repository's tracked default branch. Post a final summary: milestones/steps delivered, version bumps and tags actually required by `stated.version_policy` in each repository's latest current `repository-contract` record, and any escalations. Then stop.
