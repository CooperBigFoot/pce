# PCE workflow feedback: 2026-08-19-incidence-python-binding (second report)

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill, session 3a3b5e2d`
- Run: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-19-incidence-python-binding`, graph.v2.json, plan version 2
- Outcome: `paused` (driver alive but wedged ~14h; escalated to the human, nothing killed)

Supersedes nothing in the first report. Covers only the plan-version-2 relaunch under `pce`
`101d5e85e2f262a5d5aff6c377799a6e71257e2a92060501047cd3c9c2419a4c`.

## Executive summary

The v2 repartition worked exactly as intended: the plan advance carried IPB1-IPB4 completions and
amendments, cleared IPB6's disputed park, and dispatched the new IPB8 alongside a re-dispatched IPB6.
IPB8's worker then completed its whole act in 255 seconds, committed it, wrote `{"outcome":"done"}`,
and exited cleanly.

Fourteen hours later the driver has still not emitted `worker-done` for IPB8. It sits at 0.15 CPU
seconds with no descendants, blocked, while IPB6's worker tree stays alive but idle. One slow or hung
worker therefore withheld an unrelated, already-proven package indefinitely. The highest-impact finding
is that concurrent dispatch has no per-worker harvest: a finished worker's proof is not journaled while
a sibling is still in flight.

Two supporting findings: the announced `dispatch-worker-identified.session_path` is null in practice, and
no available `herdr pane read` source returns a worker agent's transcript even when its pane is alive
and correctly identified — so the evidence route the first report asked for is still closed.

## Evidence reviewed

- `driver-journal.jsonl` (111 records; last write 2026-08-19T19:59:31Z local, observed 2026-08-20T07:53Z)
- `package-outcomes/IPB8/7.json`, `.pce/package-results/IPB8/23.json`
- `/bin/ps` on driver pid 88148 and on the IPB6 tree 89756 / 89787 / 89794 / 90158 / 90508 / 90746
- recursive `/usr/bin/pgrep -P` from both journaled root pids
- `git` observations in the IPB8 and IPB6 worker checkouts
- `herdr pane list --workspace w05`, `herdr pane read w05:p1 / w05:p2`, `herdr workspace get w04 / w05`
- `supervision.md` invocation 5

## What worked

### The plan advance carried proven work rather than redoing it

- Evidence: `{"event":"plan-version-advanced","from_plan_version":1,"to_plan_version":2,
  "carried_completions":["IPB1","IPB2","IPB3","IPB4"],"carried_amendments":[...]}`, followed immediately
  by `package-base-composed` for IPB8 and IPB6 rather than for the completed four.
- Effect: a repartition that added one package and re-edged one dependency cost zero re-execution of
  four completed packages and their gate-earned amendments.

### The repartition resolved the real defect

- Evidence: IPB8's worker committed `04d1655 feat(python): expose authoritative completed-run log` in
  `pce/2026-08-19-incidence-python-binding/IPB8/attempt-7`, 255 seconds after dispatch.
- Effect: the missing log accessor that made IPB5 unbuildable was built as soon as a package existed
  whose criterion required it. The park was a correct signal and the repartition was a sufficient answer.

## Friction and failures

### A completed worker is not harvested while a sibling worker is still in flight

- Severity: `high`
- Phase: `execution / driver harvest loop`
- Observation: IPB8 and IPB6 were dispatched concurrently at 19:59 local. IPB8's dispatch record
  `.pce/package-results/IPB8/23.json` was written at 20:03 with
  `"exit_status":{"kind":"exited","code":0}`, `"required_artifact_presence":"present"`,
  `"surviving_processes":"absent"`, and `package-outcomes/IPB8/7.json` contains `{"outcome":"done"}`.
  Its root pid 88676 is absent from `/bin/ps`. At 07:53 the next day the journal still ends at the two
  `dispatch-pane-opened` records; no `worker-done`, no `criterion-executed`, no `package-completed` for
  IPB8 exists.
- Evidence: journal records 106-111 unchanged for 13h54m; driver pid 88148 at `0:00.15` CPU, state `Ss+`,
  `pgrep -P 88148` empty.
- Inference: the driver appears to await the full in-flight set before processing any member's outcome,
  so the slowest concurrent worker gates every faster sibling. The dispatch layer had already persisted
  everything needed to journal IPB8.
- Impact: fourteen hours of wall-clock lost on a package that finished in four minutes, and a proven
  commit left outside the journal — the only admissible run proof. With `wait_timeout_ms: null` there is
  no bound on this.

### `dispatch-worker-identified.session_path` is null

- Severity: `medium`
- Phase: `evidence preservation`
- Observation: both v2 dispatch records carry `session_path: None`. The field was announced as the
  supported route to a worker's Prime transcript, replacing `grep ~/.prime`.
- Evidence: parsed directly from `driver-journal.jsonl` records for IPB8 issuance 7 and IPB6 issuance 8,
  both dispatched by the new binary.
- Inference: either the field is populated only on some dispatch paths, or it is emitted later in a
  worker's lifetime than `dispatch-worker-identified`.
- Impact: the skill instructs the supervisor to preserve the transcript via `session_path` before any
  worker kill and explicitly forbids the `~/.prime` fallback. With the field null, there is no
  sanctioned way to preserve a wedged worker's reasoning before signalling it.
- Follow-up observation (added 2026-08-20, after the human suggested the driver predated the install):
  it did not. `stat` on `/Users/nicolaslazaro/.local/bin/pce` gives mtime `2026-08-19T19:50:44 CEST`;
  `/bin/ps -p 88148 -o lstart=` gives `Wed Aug 19 19:59:29 2026`. The driver started nine minutes after
  the install, with the absolute path to that binary in its argv and its digest verified as
  `101d5e85...` beforehand. The null `session_path` is therefore not a stale-driver artifact.
- Settled 2026-08-20: dispatch records `dispatch_sequence` 29 and 30, written by the current binary after
  the wedged worker was killed, also carry `session_path: None`. All ten `dispatch-worker-identified`
  records in the journal (seq 1, 5, 9, 15, 19, 20, 23, 24, 29, 30) are null. The field is not populated
  on this event by this binary under any timing, so the skill's pre-kill transcript-preservation step
  cannot be followed as written.

### No `herdr pane read` source returns a live worker agent's transcript

- Severity: `medium`
- Phase: `evidence preservation`
- Observation: `herdr pane list --workspace w05` shows `w05:p2` alive and labelled
  `pce-ddf34b22b89661b3345a0747025d` — the worker agent named in the journal.
  `herdr pane read w05:p2 --source recent-unwrapped --lines 400 --format text` returns empty output.
  `--source screen` is refused: `invalid read source: screen`. `w05:p1` returns only the shell prompt.
- Evidence: commands and outputs recorded in `supervision.md` invocation 5.
- Inference: worker agents render through a mechanism the pane read sources do not capture, independent
  of whether the pane still exists.
- Impact: the first report's finding — that a park's reasoning is unrecoverable — is confirmed to be
  broader than pane lifetime. Even a live, correctly identified agent pane yields nothing. Combined with
  the null `session_path`, a supervisor facing a wedge has no route to the worker's own account of it.

### The kill discriminator excludes the case it is most needed for

- Severity: `medium`
- Phase: `recovery`
- Observation: the discriminator authorises a guarded kill only for a tree with cumulative CPU under one
  second, a clean worktree, and no writes. IPB6's tree has 51.02 CPU seconds, a dirty worktree
  (`?? bindings/python/benchmarks/`, `?? bindings/python/tests/test_sweep_baseline.py`), and clearly
  wrote work — yet it has been idle for ~11.5h, accruing about 0.07 CPU-seconds per three minutes.
- Evidence: two `/bin/ps` samples three minutes apart (90746: 0:41.68 -> 0:41.75; 90158: 0:08.19
  unchanged); `find -newermt '2026-08-19 20:30'` over the worktree returns nothing.
- Inference: the discriminator correctly protects a worker that did work and is foreground-waiting, but
  it cannot distinguish that from a worker that did work and then hung. Its inputs are cumulative, not
  rate-based.
- Impact: the supervisor is designated as the only timeout and then given no admissible action for the
  most likely wedge shape. I escalated instead of acting, which is the correct outcome under the current
  rule but leaves a fourteen-hour stall resolvable only by a human.

## Recommendations

### Journal `worker-done` per worker, as each one's dispatch result lands

- Addresses: "A completed worker is not harvested while a sibling worker is still in flight"
- Change: harvest and journal each worker's outcome when its own dispatch result is persisted and its
  root process is gone, rather than at the completion of the in-flight set. Everything required for
  IPB8 was on disk at 20:03.
- Location: driver-run's concurrent dispatch wait loop.
- Trade-off: the journal interleaves records from concurrent packages, so readers must not assume a
  contiguous per-package block.
- Confidence: `high`

### Add a rate-based limb to the wedge discriminator

- Addresses: "The kill discriminator excludes the case it is most needed for"
- Change: allow a guarded kill when the whole tree accrues less than a stated CPU delta over a stated
  observation window and no file under the worktree has been modified in that window, regardless of
  cumulative CPU or worktree cleanliness — with the same root-argv and attempt-identity guards and the
  same leaf-first TERM-then-KILL sequence already specified.
- Location: `.claude/skills/work-graph/SKILL.md`, section 3, the discriminator paragraph.
- Trade-off: a worker legitimately blocked on a long external wait could be killed; the window must be
  generous, and the charge should remain against the environment-failure allowance.
- Confidence: `medium`

### Populate `session_path` at dispatch, or state where else it appears

- Addresses: "`dispatch-worker-identified.session_path` is null"
- Change: emit the field on `dispatch-worker-identified`, or document the event that does carry it so a
  supervisor can find a transcript without `~/.prime`.
- Location: driver dispatch journaling.
- Trade-off: none identified.
- Confidence: `high`

### Give `herdr pane read` a source that captures agent output

- Addresses: "No `herdr pane read` source returns a live worker agent's transcript"
- Change: either add a read source that returns what a worker agent pane displays, or have the driver
  persist the worker's transcript to `<vision-dir>/worker-transcripts/<PACKAGE>/<ISSUANCE>.txt` on both
  park and wedge.
- Location: `herdr` pane read sources, or the driver's worker lifecycle.
- Trade-off: transcripts consume disk and may contain long tool output.
- Confidence: `medium`

## No-change decisions

- **`wait_timeout_ms: null` as the default.** The skill deliberately makes the supervisor the timeout and
  says so. The problem observed here is not the null default but the absence of an admissible supervisor
  action once a wedge is detected, which is recommended above.
- **Concurrent dispatch of IPB6 and IPB8.** Dispatching two independent packages together is correct and
  saved real time on IPB8's four-minute act. The defect is in harvesting, not in concurrency.

## Suggested follow-up

- The first report recommended retaining worker panes on park. This report shows pane retention alone
  would not have helped, because the panes yield no agent output through any available read source.
  The two findings should be considered together, and the durable-transcript recommendation is the one
  that actually closes the gap.
