---
name: work-graph
description: Supervise and mechanically promote one frozen work-package graph. Use `/work-graph <vision-dir>` to resolve the active graph, persist launch configuration, preserve failure evidence, apply only goal-preserving recovery, notify the human at ruling boundaries, and merge only an assembly-completed run.
---

# Work a frozen graph

A driver journal is evidence, not narration. This skill turns that evidence into a durable account,
keeps the foreground driver moving across its repeated exits, and resolves only what can be proved to
preserve the vision's goal.

`$1` is an existing vision directory. Run commands from the repository that owns it. Reconstruct
state from disk on every invocation; a previous conversation is never authority. Criteria invariance
must be a refusal in the installed `pce` binary before this skill may exercise graph-revision
authority; a prose promise or harness hook is not a substitute.

## 1. Resolve the only admissible graph

Require exactly one existing vision directory. List `graph.vN.json`, parse `N` as an integer, and
select the highest version that exists and passes:

```bash
pce graph check --file <vision-dir>/graph.vN.json
```

Store that exact path as `<frozen-graph>` and use it in **every** driver, status, render, and overrule
command until a higher frozen version appears. Re-resolve the highest frozen version immediately
before every command, because the human may freeze a successor during supervision. Never select by
modification time and never use `graph.json`, a backup, or a lower frozen version.

If `graph.json` exists and differs from `<frozen-graph>`, report it as an unapproved working draft.
Do not run it. If no frozen graph exists, or the highest one is invalid, report the defect and stop.

Read `<vision-dir>/supervision.md` and `<vision-dir>/supervision-state.json` before reporting or
launching. The state records the frozen version, journal byte offset and event count, last render,
active attempts and pane ids, repeated failures, driver tmux session, and last terminal outcome.
Reconcile it against the append-only journal rather than trusting it. On resume, first append and
then report the last unresolved park, its reason, and its evidence location.

## 2. Capture launch configuration once

Derive the required repository **names** from the union of `packages[].repositories` in
`<frozen-graph>`. Do not infer them from nearby directories or prior commands.

`<vision-dir>/run.json` is the durable launch authority. On first launch, ask for source worktree
paths and optional prepare commands only for those exact names, plus any non-default driver limits
the human wants. Also ask for the names of environment variables already exported in the driver's
launch shell that every package and gate worker workspace requires. Store names only, never values.
Require absolute worktree paths and verify each repository with `git -C <path> status`. Write this
shape atomically:

```json
{
  "repositories": {"NAME": "/absolute/source/worktree"},
  "prepare": {"NAME": "COMMAND"},
  "environment": ["NAME"],
  "override_risk_ordering": false,
  "retry_limit": null,
  "local_patch_limit": null,
  "environment_failure_limit": null,
  "wait_timeout_ms": null,
  "tmux_session": "pce-work-<vision-slug>"
}
```

On later invocations, reuse these values without asking. Compare the file with the current frozen
graph: remove configured repositories and prepare commands whose names are absent, write the
filtered file atomically, and report every dropped name. If a later graph adds a repository, ask
only for that repository and extend `run.json`. Reject duplicate names, relative paths, prepare
commands without a matching repository, malformed types, and unknown fields. Reject duplicate or unset environment names before launch, and report each unset name. Environment names must match
`[A-Za-z_][A-Za-z0-9_]*` and must not claim binary-owned `TMPDIR`, `PCE_DISPATCH_TMPDIR`,
`PCE_WORKTREES`, or `PCE_WORKTREE_*`. Never fill a missing value by guesswork. Never write an
environment value to `run.json`, `supervision.md`, or the driver journal.

Use `<vision-dir>/driver-journal.jsonl` as the one journal. If another plausible journal already
exists, report the ambiguity and stop rather than starting a second history or editing either file.

## 3. Launch and relaunch the foreground driver

Build an argument vector from `run.json`; do not interpolate it through `eval`. Every launch is the
following command with the stored optional flags included only when non-null or true:

```bash
pce package driver-run --graph <frozen-graph> \
  --journal <vision-dir>/driver-journal.jsonl \
  --repository <NAME=SOURCE_WORKTREE>... [--prepare <NAME=COMMAND>]... \
  [--worker-env NAME]... [--override-risk-ordering] [--retry-limit N] [--local-patch-limit N] \
  [--environment-failure-limit N] [--wait-timeout-ms N]
```

`--worker-env` is names-only pass-through from the driver's launch environment. It applies to the
package worker and every gate worker spawned in that workspace, including later attempts after a
driver restart. Criteria commands continue to inherit the driver's full launch environment, as do
`--prepare` commands in driver-owned materializations; they are not restricted to the declared
worker set. Every relaunch must reuse the stored environment names so the journaled contract remains
unchanged.

Host only this foreground process in the named tmux session from `run.json`. Create the detached
session if absent, enable `remain-on-exit`, and relaunch an exited driver with `tmux respawn-pane -k`
rather than making anonymous sessions. Record the exact shell-escaped argv, launch time, tmux target,
and exit status in `supervision.md` before mentioning the launch in chat.

The driver exits after printing status on `Finished` or `Blocked`; an exited pane is normal. Relaunch
only after a goal-preserving resolution or a newly frozen graph makes progress possible. Never run
two drivers against one journal. The driver's own worker panes are not the supervision session.

## 4. Render and interpret the journal

While the driver pane is live, read new complete JSONL records about every 15 seconds. Track the
byte offset only after processing and persisting each record. About every two minutes, and once at
start and stop, run:

```bash
pce package render --graph <frozen-graph> \
  --journal <vision-dir>/driver-journal.jsonl \
  --output <vision-dir>/graph.html
```

The output path is stable. When a higher frozen version appears, switch every command to it together;
never mix graph versions.

Interpret typed events rather than scraping pane prose. Maintain cross-attempt patterns for at least:
`recovery-configured`, `worker-dispatched`, `dispatch-pane-opened`, `dispatch-pane-cleanup`,
`environment-preparation-executed`, `criterion-executed`, `worker-done`, `gate-finished`,
`finding-replayed`, `package-base-composed`, `package-completed`, `package-parked`,
`package-park-overruled`, `package-join-conflicted`, `worker-environment-failed`, and
`plan-version-advanced`.

Narration may state a theory, but must name the events and refs supporting it and label it as a
theory. Compare repeated reasons across attempts and plan versions; do not emit a mechanically
correct event feed that misses a recurring complaint. When the human must rule, paraphrase the facts
and stop. Do not recommend an answer: interpretive advocacy aimed at a human question launders the
agent's preference into the journal under the human's name.

Append every narration, theory, question, command, and result to `supervision.md` **before** putting
it in chat. Update `supervision-state.json` atomically after the append.

## 5. Preserve perishable evidence first

Treat `package-parked`, `worker-environment-failed`, and `package-join-conflicted` as interrupts.
Before analysis or narration, correlate package and issuance with the latest
`dispatch-pane-opened`. Immediately append an evidence section to `supervision.md` containing:

1. the raw journal record;
2. `herdr pane read <pane_id> --source recent-unwrapped --lines 10000 --format text` output, or the
   exact read failure if cleanup already won the race;
3. `herdr workspace get <workspace_id>` and `herdr worktree list --workspace <workspace_id>
   --json`, followed for every resolved checkout by `git -C <worktree> rev-parse HEAD`,
   `git -C <worktree> status --short --branch`, and `git -C <worktree> diff --stat`;
4. `<vision-dir>/package-outcomes/<PACKAGE>/<ISSUANCE>.json` and
   `<vision-dir>/.pce/package-briefs/<PACKAGE>/<ISSUANCE>.md`, or an explicit missing-file
   observation for either; and
5. for a join conflict, the base oid, dependency oids, conflicted paths, and `git` observations at
   those refs.

Persist partial evidence as each command completes; do not wait to assemble a perfect block in
memory. A `dispatch-pane-cleanup` never licenses omission. Delegate a large worktree or composed-base
inspection to a subagent that writes its full evidence directly into the vision directory and
returns only the finding's shape and evidence path to the supervisor.

## 6. Resolve only what preserves the goal

The test is not whether a problem looks technical. It is whether evidence at a named ref proves that
every frozen criterion remains reachable.

Act without prior approval, then append and report the action, only in these cases:

- **Refute one worker specification complaint.** Establish at a named ref that the alleged missing
  work is present or unnecessary and the frozen criterion still holds. Run:

  ```bash
  pce package driver-overrule --graph <frozen-graph> \
    --journal <vision-dir>/driver-journal.jsonl --package <ID> \
    --rationale 'author=work-graph supervisor; ref=<REF>; evidence_command=<COMMAND>; finding=<FACT>'
  ```

  The rationale must identify this supervising agent as author and include the exact ref, evidence
  command, and finding. An overrule is one-shot per package. A second independent refusal requires
  graph revision; do not spend another retry trying to recreate the cheap door.
- **Rebuild disposable state.** Re-run a configured prepare command, remove a corrupt disposable
  worktree, or clear a cache. Record what was removed and how it can be reconstructed.
- **Install a missing tool.** Record the exact install and undo commands in `supervision.md` before
  reporting them.
- **Repartition packaging.** Draft the next `graph.json` by splitting, merging, re-edging, renaming,
  or adding packages while preserving every existing criterion's `name`, `input`, `observation`,
  and `command` byte-for-byte. Run `pce graph check` on the draft. State that a split forfeits the
  old package's carried completion. Do **not** freeze it; present the exact human command:

  ```bash
  pce graph freeze --vision-dir <vision-dir>
  ```

Never modify any repository participating in the run. Never edit a criterion command. Never edit or
truncate the journal. Never use a flag to relax criteria invariance; none exists.

After the second identical environment failure, stop local repair. Record the two failures side by
side and surface the recurring fact as a repository-contract defect. Local patches that hide the
same false repository assumption are not recovery.

A driver refusal because a criterion changed, disappeared, or was split into renamed halves is a
human ruling, not an invitation to evade the floor. Draft and mechanically check the proposed graph,
show the old and proposed criterion text and the park reasons side by side, append the complete
briefing, push-notify, and stop. The freeze will refuse the changed draft unless the human writes an
exact revision record containing their identity, the predecessor criterion, the successor criterion
(or removal), and their rationale, then explicitly supplies it:

```bash
pce graph freeze --vision-dir <vision-dir> \
  --criterion-revisions <human-authored-record-path> \
  --repository <NAME>=<SOURCE_WORKTREE> [...]
```

The human-authored JSON record has this exact typed shape; `successor: null` means removal:

```json
{
  "schema_version": 1,
  "ratified_by": "<human identity>",
  "revisions": [{
    "previous_package": "<package id>",
    "predecessor": {"name":"...","input":"...","observation":"...","command":"..."},
    "successor": {"name":"...","input":"...","observation":"...","command":"..."},
    "rationale": "<human-authored rationale>"
  }]
}
```

The skill never writes the revision record, supplies `--criterion-revisions`, or runs the freeze.
Only the human ratifies a frozen criterion revision. Ratification forfeits the affected package's
carried completion. Mechanical freezes remain definition-preserving and can never carry a revision
record.

## 7. Notify or promote at the terminal boundary

When the driver exits, run `driver-status` with the same `<frozen-graph>` and journal and the stored
`--override-risk-ordering` when enabled, render once, and append the terminal interpretation before
any notification or repository mutation.

A blocked, parked, partially complete, `assembly-failed`, or otherwise non-finished status never
pushes a ref, opens a pull request, or invokes a merge command. Notify and stop as before:

```bash
herdr notification show "pce graph stopped" --body "<vision>: <package and reason>" --sound request
```

Also notify immediately when a criterion ruling or recurring repository-contract defect needs the
human. Record the notification command and result. Do not silently relaunch a blocked driver. If
notification delivery fails, retain the failure in `supervision.md` and report it in chat; do not
claim that a push arrived.

Only a journal whose active plan version ends in `assembly-completed`, and whose `driver-status` is
`Finished`, enters sections 8 through 10. This is a mechanical consequence of completed proof, not
another human ruling.

## 8. Establish the exact promotion inputs

Promotion currently supports exactly one repository. Require the union of graph repository names to
contain one name and `run.json` to map exactly that name to one source worktree. Refuse before any
push otherwise. Resolve all of the following once and append them to `supervision.md`:

- repository identity from `gh repo view --json nameWithOwner,defaultBranchRef,url` executed in the
  source worktree;
- remote `origin` and the authenticated actor from `gh api user --jq .login`;
- the frozen graph's and journal's SHA-256 digests computed from their bytes with
  `shasum -a 256`;
- the final `AssemblyRepositoryComposed.base_oid` for the repository in the active plan version;
- every package input named by the final assembly event, its final issuance, local branch
  `pce/<vision>/<package>/attempt-<issuance>`, and the event's exact oid; and
- assembly branch `pce/<vision>/assembly-v<plan_version>` at the recorded assembly oid.

Every package-input branch must resolve locally to the oid proved by the journal. The assembly oid
must resolve to a commit and equal the final recorded assembly oid. A missing or mismatched ref is a
terminal promotion failure, not a reason to reconstruct history.

Fetch the remote default branch immediately before promotion. Require the fetched default-branch oid
to be an ancestor of the assembly oid. This check proves that the already-executed assembly is the
tree being proposed rather than a stale authored-base composition. If it is not an ancestor, append
the two oids, notify the human that default-branch movement requires the driver to compose and prove
a successor assembly, and stop without pushing. Never rebase, merge, or resolve this difference in
the skill.

Create `<vision-dir>/.pce/promotion-state.json` atomically before the first external mutation. It
records schema version 1, repository identity, default branch and fetched oid, graph path and digest,
journal path, assembly and attempt refs with oids, authenticated actor, current phase, pull-request
URL when known, merge oid when known, and a chronological operation list. Reconcile it only with
read-only `git` and `gh` observations. If it records any failed operation, report the recorded partial
state and stop; an invocation never retries a failed promotion.

## 9. Render the pull request as the proof record

Write `<vision-dir>/.pce/promotion-pr-body.md` atomically from the frozen graph versions and typed
journal records. The body is generated evidence, not a diff summary or review request. Include:

1. vision name, repository, journal repository-relative path and SHA-256, frozen graph path and
   SHA-256, and all plan versions traversed;
2. one row per package naming its final attempt ref when it is a retained assembly input, each
   authored and amended criterion, and the final corresponding command and exit status;
3. every accepted gate finding with gate and finding ids, command, witness and repair exit statuses,
   and every repository witness/repair ref;
4. every `package-park-overruled` rationale verbatim in a fence longer than any fence contained in
   the rationale;
5. every `plan-version-advanced` event, its carried completions and amendments, plus a structural
   diff of the adjacent frozen graphs stating which package definitions and dependency edges were
   revised;
6. every human-ratified criterion revision, quoting the predecessor criterion bytes, successor
   criterion bytes (or removal), ratifier, and human rationale verbatim. Put each value in a fence
   longer than every fence contained in that value. Refuse before pushing if a journal revision
   cannot be correlated exactly with its versioned frozen revision record and adjacent frozen
   graphs; and
7. every final `assembly-criterion-executed` command and exit status against the composed whole,
   including paired amendment proof, followed by the terminal `assembly-completed` event.

Quote journal text without paraphrasing. Successful stdout/stderr and transient absolute working
paths stay in the hashed journal rather than bloating or leaking machine paths into the PR. Include
failure output only where a carried recovery or overrule needs it to remain intelligible. Refuse if
any referenced graph version is absent, if a final package/assembly proof cannot be correlated, or
if the UTF-8 body exceeds GitHub's 65,536-byte body limit. These refusals happen before pushing. The
PR title is `<vision>: promote proven assembly`.

## 10. Push, wait for required checks, and merge once

Push the assembly ref and all package-attempt refs retained as final assembly inputs to `origin` in
one `git push --atomic` invocation using explicit `<oid>:refs/heads/<name>` refspecs. Never force-push and never push any
other ref. Record the exact refspecs and command result in `promotion-state.json` and
`supervision.md` before proceeding.

Open the pull request with `gh pr create --base <default> --head <assembly-branch> --title ...
--body-file <promotion-pr-body>`. Record its URL. The authenticated GitHub actor is the committer;
do not synthesize or override an identity.

Inspect required checks with `gh pr checks <url> --required`. If any are pending, wait with
`gh pr checks <url> --required --watch --fail-fast`. A failed, cancelled, or timed-out required check
is a terminal promotion failure: record the exact check output, notify the human, and stop. The
absence of required checks permits immediate merge. Approval is never requested or awaited.

Fetch the default branch again after required checks and require its oid to equal the oid recorded
in `promotion-state.json`. Movement is a terminal promotion failure; do not merge an assembly whose
base changed while CI ran.

Merge exactly once with merge-commit mechanics:

```bash
gh pr merge <url> --merge --subject '<vision>: promote proven assembly' \
  --body 'Proof: <journal-path>
Frozen graph sha256: <digest>'
```

Do not squash, rebase, enable auto-merge, delete archaeology branches, or bypass branch protection.
After the command, observe the PR with `gh pr view --json state,mergedAt,mergeCommit,url`; only
`state=MERGED` is success. Record the merge oid and mark the promotion state `merged`. If push, PR
creation, check waiting, or merge fails, record exactly which prior operations succeeded, mark the
state `failed`, notify the human, and stop. Never retry, force, or continue into a different route.

On success, send the done notification, append it and its result, then report the merged PR URL as
the run's final line. No prose follows that URL.

## 11. Preserve the boundary

This skill may draft only a goal-preserving graph repartition under section 6 and may promote only
through sections 7 through 10. It does not write `vision.md`, invoke `/to-graph`, weaken or rename
criteria, author or supply a criterion revision record, freeze a graph, edit the journal, invoke
`/land-ticket`, or land any non-assembly branch. Criterion revision, freeze, and park-overrule remain
human rulings; promotion after proof is mechanical.

The append-only driver journal is the admissible run proof. `supervision.md` is explanation and
captured evidence, never a substitute for or repair of that proof. No important fact may exist only
in chat scrollback.
