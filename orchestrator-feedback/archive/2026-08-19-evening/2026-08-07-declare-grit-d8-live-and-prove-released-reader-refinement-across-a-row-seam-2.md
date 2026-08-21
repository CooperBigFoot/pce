# PCE workflow feedback: prime-agent wedge captured live (GD2 issuance 8)

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 6, GD2 issuance 8
- Outcome: `recovered` — wedge captured, leaf killed, GD2 re-dispatched at issuance 9 uncharged

## Executive summary

This is the fourth known instance of a prime-agent wedge and the first captured before the
process was killed. The captures narrow it to a specific, reproducible-looking state:

**The daemon recovery journal records `tool_execution_start` with no matching
`tool_execution_end`, the session file's last record is an `assistant` message whose final
block is a `toolCall` with no answering `toolResult`, and the whole process tree burns 0.54
seconds of CPU over two hours with no socket open and nothing written to the worktree.**

The agent is not slow, not waiting on the network, and not doing work. It is parked in
`uv__io_poll`/`kevent` on every thread waiting for a tool result that never arrives, and the
tool appears never to have executed — a running recursive `glob` would consume CPU, and none
was consumed.

Two secondary findings that matter independently of the wedge:

1. `~/.prime/agent/sessions/*.jsonl` **contains the worker transcript that is unrecoverable
   from the dispatch pane.** This is the missing evidence named as finding 1 of the first
   report for this run. It is already on disk; nothing needs to be built to retain it, only
   located.
2. `grep -rl <agent-name> ~/.prime` returns session files belonging to *other* visions. The
   authoritative mapping is `createCommand.sessionPath` in the daemon worker record. Using
   grep instead produces a confident, entirely wrong diagnosis.

## Evidence reviewed

All captured before the kill and retained in the vision directory:

- `planning/.../wedge-evidence-GD2-8-sample.txt` (115,908 bytes) — `sample 19343 10`
- `planning/.../wedge-evidence-GD2-8-context.txt` (12,339 bytes) — `ps` tree, `ps -M`, `lsof -p`, `lsof -i`
- `planning/.../wedge-evidence-GD2-8-session.jsonl` (171,249 bytes) — the worker's prime-agent session, 23 records
- `planning/.../wedge-evidence-GD2-8-daemon-worker.json` (1,454 bytes)
- `planning/.../wedge-evidence-GD2-8-3d2a7b5c0aa9.recovery.jsonl` (20,433 bytes)
- `planning/.../driver-journal.jsonl` events 59-63
- `pce package driver-status` recovery block for GD2, before and after the kill

## What worked

### The environment-failure budget absorbed the kill without charging the package

- Evidence: `driver-status` recovery for GD2 immediately after re-dispatch —
  `{"dispatches_remaining":2,"local_patch_remaining":1,"next_rung":"retry","retry_remaining":1}`,
  identical to the value before the kill. The journal records
  `worker-environment-failed … Exited { code: ExitCode(143) }` followed by
  `worker-dispatched GD2 issuance 9`.
- Effect: killing a wedged leaf is a free, correct recovery action. It does not consume a
  retry, a local patch, or the package's one-shot overrule, and it is not a park. This is the
  behaviour that makes "capture then kill" safe as a standing procedure.

### The driver re-dispatched automatically within 20 seconds

- Evidence: journal moved 59 → 63 events between the `kill` and a check 20 seconds later.
- Effect: no supervisor relaunch was needed; the driver's own recovery ladder handled it.

## Friction and failures

### 1. A wedged agent is indistinguishable from a working one at the journal level

- Severity: `high`
- Phase: `execution`
- Observation: from the driver journal alone, GD2 issuance 8 looked identical to a healthy
  long-running act for two hours. The last journal record was `dispatch-pane-opened` at
  11:37:57; nothing further was ever written. The driver did not time out, and
  `--wait-timeout-ms` is unset in this run's `run.json`.
- Evidence: `driver-journal.jsonl` events 56-59; `run.json`.
- Inference: with no timeout configured, a wedged worker holds the driver indefinitely. Whether
  a default timeout exists was not determined from the binary.
- Impact: 122 minutes of wall-clock lost before a human asked whether the run was still moving.
  Detection depended entirely on a human prompting for status.

### 2. The wedge state is legible but only outside the workflow's own artifacts

- Severity: `high`
- Phase: `execution / recovery`
- Observation: the decisive facts — `tool_execution_start` with no `tool_execution_end`, and a
  `toolCall` with no `toolResult` — live in `~/.prime/agent/daemon-workers/<id>/<worker>.recovery.jsonl`
  and `~/.prime/agent/sessions/<session>.jsonl`. Nothing in the vision directory, the driver
  journal, or the dispatch pane exposes them.
- Evidence:
  - recovery journal final three entries, verbatim:
    `{"busy":true,"operation":"message_start","recordedAt":"2026-08-19T09:38:47.974Z"}`,
    `{"busy":true,"operation":"message_end","recordedAt":"2026-08-19T09:38:51.966Z"}`,
    `{"busy":true,"operation":"tool_execution_start","recordedAt":"2026-08-19T09:38:51.975Z"}`
  - session `01a01962-73ad-71ce-…`: 23 records, first `09:37:58.418Z`, last `09:38:51.975Z`,
    final record `role: assistant`, block types `['thinking','toolCall']`, no `toolResult` after it.
- Inference: the daemon knows it is stuck (`busy: true` for two hours with an unclosed
  `tool_execution_start`) and nothing surfaces that to the driver.
- Impact: diagnosis required knowing that `~/.prime` exists and how its records map to a
  dispatch. Every prior instance was killed on sight, so this state has never been recorded.

### 3. The unanswered tool call was a recursive glob over `$HOME`

- Severity: `medium`
- Phase: `execution`
- Observation: the final `toolCall` was an `ipython` block running
  `glob.glob('/Users/nicolaslazaro/**/pourpoint-0.3.0-*.whl', recursive=True)` plus the same
  over `/tmp/**` and `/private/tmp/**`. The worker had just read `validate_live_environment`
  (`scripts/released_wheel_proof.py:418-427`) and learned it needs `POURPOINT_RELEASE_WHEEL`,
  so it went looking for the wheel on disk.
- Evidence: `wedge-evidence-GD2-8-session.jsonl` record 22.
- Inference — clearly marked as inference: this is *correlation*, not established cause. A
  recursive glob over a home directory is a plausible trigger for a tool-execution stall, but
  the captures show **zero CPU**, so the glob does not appear to have run at all. If it had
  run and hung, CPU would be non-zero. The likelier reading is that the tool execution never
  started; the glob is what it would have executed.
- Impact: unknown. Recorded because it is the only distinguishing content in the stuck call,
  and because three of four prior instances were killed before this could be checked.

### 4. `grep` over `~/.prime` misattributes sessions across visions

- Severity: `medium`
- Phase: `recovery / diagnosis`
- Observation: `grep -rl "pce-197909c135c90740778d59999dbb" ~/.prime` returned two session
  files. The larger (`01a0197a-…`, 559 KB, 49 records) is an active, healthy agent working on
  `camels-four-quadrant@v3` / `trust-gridded-525` in `/Users/nicolaslazaro/Desktop/work/camels-trust`
  — a different vision entirely. Only `01a01962-…`, named by `createCommand.sessionPath` in the
  daemon worker record, belongs to this dispatch.
- Evidence: both session files; `wedge-evidence-GD2-8-daemon-worker.json` field
  `createCommand.sessionPath`.
- Impact: a diagnosis based on the grep result would have concluded the agent was healthy and
  working, and would have been completely wrong. Caught here only because the session content
  named an unrelated project.

## Recommendations

### Surface `busy` staleness from the daemon recovery journal to the driver

- Addresses: findings 1 and 2
- Change: when the daemon has an unclosed `tool_execution_start` older than a configurable
  threshold, emit a typed driver event (for example `worker-stalled` carrying the worker id,
  the operation, and its `recordedAt`) rather than leaving the driver blind. The daemon already
  records everything needed; only the propagation is missing.
- Location: daemon worker recovery journal writer, and the driver's dispatch supervision loop.
- Trade-off: a threshold too low would flag legitimate long tool executions; the event should be
  informational, not a kill signal, and the human or supervisor decides.
- Confidence: `high` — this is the smallest change that turns a two-hour invisible stall into a
  journal record.

### Record the worker's session path in the dispatch event

- Addresses: findings 2 and 4
- Change: include `session_path` (from `createCommand.sessionPath`) in the
  `dispatch-worker-identified` event, alongside the existing `agent_name`, `pane_id`,
  `workspace_id`, and `process`.
- Location: `dispatch-worker-identified` emission in the driver dispatch path.
- Trade-off: one more field; no behavioural change.
- Confidence: `high` — it makes the worker transcript addressable from the journal and removes
  the need for the grep that misattributes sessions across visions. It also largely resolves
  finding 1 of the first report for this run, without any pane-retention work.

### Document "process tree before any kill" as the wedge discriminator

- Addresses: findings 1 and 3
- Change: state the rule explicitly where recovery is described — an agent at 0% CPU **with a
  child process doing real work** is a worker foreground-waiting on its act and must never be
  killed; an agent tree whose *cumulative* CPU is under a second, with a clean worktree and
  nothing written, is parked machinery. Check the whole tree, always, before any kill.
- Location: `skills/work-graph/SKILL.md` section 5 or 6, and the equivalent recovery guidance
  for `/pce`.
- Trade-off: none. It codifies a discriminator that already exists as tacit knowledge.
- Confidence: `high`

### Set a default `--wait-timeout-ms` or document its absence

- Addresses: finding 1
- Change: either ship a default dispatch wait timeout, or state in the skill that leaving
  `wait_timeout_ms` null means a wedged worker holds the driver indefinitely and the supervisor
  is the only timeout.
- Location: `pce package driver-run` defaults, and `run.json` guidance in `skills/work-graph/SKILL.md`
  section 2.
- Trade-off: a default timeout risks killing legitimate long acts — which is precisely why the
  process-tree discriminator above should land first.
- Confidence: `medium`

## No-change decisions

- **Charging the kill to the environment budget rather than the package ladder.** Correct as
  observed: GD2's `dispatches_remaining`, `retry_remaining`, and `local_patch_remaining` were all
  unchanged after the kill and re-dispatch. An environment failure is not a park and correctly
  does not engage the one-shot overrule limit.
- **Automatic re-dispatch after `worker-environment-failed`.** Worked without supervisor
  intervention, 20 seconds from kill to new dispatch. No change.
- **The second-identical-environment-failure stop rule.** GD2 now has two environment failures in
  this run (issuance 2: exit 0 after 82s, produced nothing; issuance 8: exit 143 after 2h02m at
  0.54s CPU, killed). They have different signatures, so the stop rule is correctly not
  triggered. The rule as written distinguishes them properly.

## Suggested follow-up

- **A prime-agent wedge fix brief.** With this capture the ledger now holds four instances and,
  for the first time, a precise stuck state (`tool_execution_start` with no
  `tool_execution_end`, no CPU, no socket, no `toolResult`). That is enough to justify its own
  brief rather than further per-instance feedback reports.
- **Whether the wedge is tool-specific.** This instance stalled on an `ipython` tool call. If the
  other three instances' captures were retained anywhere, comparing the stalled tool across them
  would establish whether `ipython` is implicated or incidental. Marked as an experiment: the
  prior instances were killed on sight, so the data may not exist.
