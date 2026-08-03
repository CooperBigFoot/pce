# PCE workflow feedback: dispatch completion is not tracked by construction

- Date: `2026-08-02`
- Orchestrator: Claude Code, Opus 5, session `1c3f11f1-b283-473d-8d06-9debc34227c0`
- Run: `planning/2026-08-01-catalogue-origins` in `RivRetrieve`
- Outcome: in progress (five providers merged to `main`; six reachable providers in planning or execution)

## Executive summary

This report covers ONE finding, at the user's request: **the orchestrator has no deterministic
mechanism that tells it when a dispatched worker has finished.** Whether a completed dispatch is ever
collected depends on the orchestrator remembering, at dispatch time, to construct a separate polling
task by hand. It is a per-dispatch act of memory, not a property of the dispatch mechanism.

It failed in this run. The orchestrator dispatched three Codex workers and created no poller. Nothing
was scheduled to observe their completion. The gap was surfaced by the user asking how the orchestrator
knew when workers finish — not by any workflow mechanism.

Per the user's instruction, this report states the problem and does not design a solution.

## Evidence reviewed

- `planning/2026-08-01-catalogue-origins/events.jsonl` — 168 `dispatch` records across the run
- 98 Codex transcript logs under the session scratchpad directory
- The session transcript, including two user interventions quoted below
- 8 workflow run directories under `subagents/workflows/wf_*`
- `/private/tmp/.../scratchpad/m2b.log` — a zero-byte Codex log

## Observed facts

### 1. Two dispatch channels exist, and only one is tracked

Claude subagent dispatches go through the Workflow tool. Those are harness-tracked: the run produced a
task notification for every workflow, including failures. When `wf_ee69c6e1-759` hit a session limit,
the notification reported `agents_done: 0`, `agents_error: 2` and the per-agent error text.

Codex dispatches go through `nohup codex exec … &` inside a Bash call that returns immediately. The
process is detached. The harness registers no task, the orchestrator receives no notification on exit,
and nothing appears in the user's interface. The user observed this directly:

> "How can I see that there are four codecs workers running? I don't see any background shell commands
> in the clot code interface."

The run made 98 such dispatches.

### 2. Completion awareness is a separately-constructed, hand-written poller

The orchestrator's compensating pattern is a second Bash call with `run_in_background: true` running
an `until` loop that greps each Codex log for the terminal marker `tokens used` and exits when all are
present. That exit is harness-tracked and produces the notification. Observed instances: `b5bq0gcdq`,
`blaevuvk2`, `bohi3uoj4`, `b6ilz677d`, `b0ox6t7k6`.

The poller is not created by the dispatch. It is a distinct decision the orchestrator must remember to
make, listing by hand the exact log paths it must watch.

### 3. The mechanism failed in this run

The orchestrator dispatched three Codex workers — the milestone-10 graph revision
(`m10-step-planner-r2`), the milestone-4 graph revision (`m4-step-planner-r3`), and the milestone-13
step-1 plan writer (`m13-s1-plan-writer-r2`) — and created no poller for any of them. It then reported
progress to the user and moved on.

Nothing was scheduled to observe those three workers. The omission was surfaced by the user asking:

> "If you cannot see them, how do you know when they're done? Do they report back? Do you get pinned?"

The orchestrator created poller `b0ox6t7k6` only after that question. Two of the three
(`m10-step-planner-r2`, `m4-step-planner-r3`) had already completed by then and were sitting
uncollected.

### 4. The same class of failure occurred earlier in the run, also caught by the user

Earlier the user intervened twice:

> "I do not see any worker"

> "I only see one worker. Can you please tell me if this is correct or you just forgot to dispatch?"

Investigation at that point established that no Codex process belonging to this run was alive: the
milestone-5 step-2 worker had finished uncollected, the milestone-3 step-2 re-check had never been
dispatched at all, and the second batch had nothing running. Both detection events in this run came
from the user, not from the workflow.

### 5. The poller detects success only, so a dead worker is indistinguishable from a working one

The `until` loop tests for the presence of `tokens used`, which Codex writes on normal completion. A
worker that dies without writing it never satisfies the condition, so the loop continues indefinitely
and no notification is ever produced. Silence is the same observable for "still running", "crashed" and
"never started".

`m2b.log` is a concrete instance of a dispatch that produced no output: 0 bytes, written `17:56`, with
no Codex process attached to it when checked at approximately `20:5x`. It was not noticed at any point
during the run and was found only while gathering evidence for this report.

### 6. A silent dispatch defect was found by manual inspection, not by any signal

Four step-plan-writer dispatches (`m4-s1`, `m7-s1`, `m12-s1`, `m13-s1`) ran to normal completion and
produced no `plan.md`. Each had been launched with `-C <worktree> --sandbox workspace-write` while its
brief named an output path under `planning/`, outside the sandbox's writable root. Codex refused with
`patch rejected: writing outside of the project; rejected by user approval settings`.

All four wrote `tokens used`, so a poller watching them would have fired a normal completion
notification. The failure was visible only in the final message inside each transcript, and was found
because the orchestrator manually inspected the logs after a poller fired. Had no poller existed — the
condition described in finding 3 — the four failures would have remained undetected.

## Inference, distinguished from the above

The facts above establish that detection depended on orchestrator memory and on user intervention. The
following is inference, not established by the evidence:

- That the orchestrator "forgot" is the plain reading, but the transcript cannot distinguish forgetting
  from a deliberate choice to poll later. The observable is only that no poller existed at the time the
  user asked.
- The run's dispatch volume (98 Codex dispatches, 168 recorded dispatch records) plausibly contributes
  to omission rates, but the evidence does not measure this and no causal claim is supported.

## Impact

- Completed work sat uncollected on at least two occasions in this run, both surfaced by the user.
- A zero-output dispatch (`m2b.log`) went undetected for the remainder of the run.
- Four dispatches produced no artifact while reporting normal completion; detection required manual
  transcript reading.
- The user cannot see Codex workers in their interface at all, so the user's own ability to audit
  progress depends entirely on the orchestrator's narration, which is the thing that failed.

## Requirement

Stated as a requirement rather than a design, per the instruction accompanying this report:

**Every dispatch, on every channel, must produce a completion signal to the orchestrator as an
inherent property of having been dispatched — not as a consequence of a second action the orchestrator
must remember to take.** The signal must fire on abnormal termination and on producing no output, not
only on normal completion, because the present failure mode renders those three states
indistinguishable.

No mechanism is proposed here.

## No-change decisions

None. This report covers a single finding at the user's request; it is not a full review of the run,
and the absence of other findings here should not be read as their absence from the run.

## Suggested follow-up

A full orchestration review of this vision remains unwritten and would cover material this report
deliberately excludes — among it the graph-revision rounds that destroyed two step specifications and
had to be recovered from Codex transcripts, the merge-train conflict resolutions that introduced two
untested defects into `AGENTS.md` on `main`, and the several instances where an adversarial gate caught
a defect in already-merged code rather than in the artifact it was commissioned to review.

---

# Addendum: a second always-on gap, found while executing

Recorded after the report above, during the execution phase of the same run. It is the same shape as
the finding above — something that should hold by construction instead depends on remembering — so it
belongs here rather than in a new report.

## Observed facts

### 7. A personal workflow instruction in global agent config reaches every sandboxed executor

`/Users/nicolaslazaro/.codex/AGENTS.md` contains 21 references to `clog`, the operator's personal
session-journaling tool. Codex reads that file in every repository, so every dispatched executor
inherits the instruction. `clog` writes to `~/Documents/claude-journal/`, which is outside every
executor sandbox's writable root, so the command always fails with `Operation not permitted`.

Measured consequences in this run:

- Two step plans (`m7-s1`, `m12-s1`) ended with a `clog log …` action and were reported
  `PLAN_INFEASIBLE` by their executors — **after their implementations were complete and every gate
  was green.** The failing action was the last one.
- The orchestrator responded by removing `clog` from the plans and from the plan-writer brief.
  Verified afterwards: `clog` occurs zero times in all five step plans, zero times in the repository's
  `AGENTS.md`, and zero times in the repository's `CLAUDE.md`. Executors nevertheless kept attempting
  it, because the instruction was never in any of those places.
- The source was located only by searching outside the repository. The repository-scoped fix could not
  have worked, and three rounds were spent before the cause was found.

### 8. The same class as finding 3, in a different register

Findings 1-6 concern a dispatch whose completion must be observed by remembering to observe it. This
is an instruction whose *scope* is wider than its author intended and which no repository-level
inspection reveals. In both cases the failure is invisible from inside the artifact being worked on.

## Requirement

Stated as a requirement, not a design, consistent with the instruction governing this report:

**An agent dispatched to perform repository work must receive repository obligations only.** Where the
harness composes instructions from outside the repository — global agent config, operator preferences,
personal tooling — those must be distinguishable from repository requirements at the point the agent
reads them, so that an agent can tell "this repository requires X" from "this operator habitually does
X", and so that a plan reviewer inspecting the repository can see the full set of instructions the
executor will actually receive.

No mechanism is proposed here.

## Impact

Two false `PLAN_INFEASIBLE` reports on complete, green implementations; three revision rounds spent on
a cause that was not present in anything under review. No incorrect code was produced or committed —
in both cases the orchestrator verified the gates independently outside the sandbox and committed the
work, so the cost was time and misdirected diagnosis rather than correctness.
