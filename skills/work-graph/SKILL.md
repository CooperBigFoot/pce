---
name: work-graph
description: Supervise one frozen work-package graph through repeated driver runs. Use `/work-graph <vision-dir>` to resolve the active frozen graph, persist launch configuration, watch and render the journal, preserve failure evidence, apply only goal-preserving recovery, and notify the human when a ruling or terminal result requires attention.
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
the human wants. Require absolute worktree paths and verify each repository with `git -C <path>
status`. Write this shape atomically:

```json
{
  "repositories": {"NAME": "/absolute/source/worktree"},
  "prepare": {"NAME": "COMMAND"},
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
commands without a matching repository, malformed types, and unknown fields. Never fill a missing
value by guesswork.

Use `<vision-dir>/driver-journal.jsonl` as the one journal. If another plausible journal already
exists, report the ambiguity and stop rather than starting a second history or editing either file.

## 3. Launch and relaunch the foreground driver

Build an argument vector from `run.json`; do not interpolate it through `eval`. Every launch is the
following command with the stored optional flags included only when non-null or true:

```bash
pce package driver-run --graph <frozen-graph> \
  --journal <vision-dir>/driver-journal.jsonl \
  --repository <NAME=SOURCE_WORKTREE>... [--prepare <NAME=COMMAND>]... \
  [--override-risk-ordering] [--retry-limit N] [--local-patch-limit N] \
  [--environment-failure-limit N] [--wait-timeout-ms N]
```

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
briefing, push-notify, and stop with:

```bash
pce graph freeze --vision-dir <vision-dir>
```

The skill never runs that command.

## 7. Notify and stop at the boundary

When the driver exits, run `driver-status` with the same `<frozen-graph>` and journal and the stored
`--override-risk-ordering` when enabled, render once, and append the terminal interpretation before
any notification. Use:

```bash
herdr notification show "pce graph stopped" --body "<vision>: <package and reason, or finished>" --sound request
```

Use `--sound done` for `Finished`. Also notify immediately when a criterion ruling or recurring
repository-contract defect needs the human. Record the notification command and result. Then stop
polling. Do not silently relaunch a blocked driver.

If notification delivery fails, retain the failure in `supervision.md` and report it in chat; do not
claim that a push arrived.

## 8. Preserve the boundary

This skill may draft only a goal-preserving graph repartition under section 6. It does not write
`vision.md`, invoke `/to-graph`, weaken or rename criteria, freeze a graph, land branches or pull
requests, or invoke `/land-ticket`.

The append-only driver journal is the admissible run proof. `supervision.md` is explanation and
captured evidence, never a substitute for or repair of that proof. No important fact may exist only
in chat scrollback.
