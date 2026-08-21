# Brief: a signal to a dispatch client destroys an hour of agent work, and the waiting side is never told

Status: GRILLED 2026-08-18 — ready to dispatch. Doctrine is recorded in
`docs/adr/0021-a-dispatched-agent-outlives-the-client-that-started-it.md` and the
`Dispatch durability` term, the `Idle eviction → Residency sweep` alias, and the
`Dispatch death cause` ambiguity in `CONTEXT.md`. Target repository:
`/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`. This brief supersedes the earlier
draft `prime-agent-eviction-brief.md`, whose diagnosis was disproven during the grill; that
file has been removed so no agent can pick it up.

## Read this first: the earlier diagnosis was wrong

An earlier draft blamed prime-agent's idle-eviction sweep for evicting sessions with an
in-flight `prompt_and_wait`, on the theory that `lastActivityAt` goes stale during one long
headless prompt. **That theory is false, and an agent that "fixes" it will harden a code path
that never executed and write a regression test that passes for the wrong reason.** The
evidence:

- Every real eviction logs `Evicted idle worker ...`
  (`packages/coding-agent/src/modes/daemon/daemon-supervisor.ts:877`). That string does not
  appear anywhere in `~/.prime/agent/logs/daemon.sock.890b2c64.log`. The sweep evicted nothing.
- The kill arrives as a **client command over the socket**. The stack recorded in that log is
  `handleLine → handleCommand → stopWorker`. Inside `handleCommand` only two paths reach
  `stopWorker`: `complete_owned_session` (`daemon-supervisor.ts:1546`) and a root-session
  `kill` (`:1930`). The sweep calls `stopWorker` from `runIdleEvictionSweep`, an entirely
  different stack, and it is silent.
- `canEvictWorker` (`packages/coding-agent/src/core/session-action-store.ts:406-422`) requires
  *every* session in the worker to be non-streaming with zero attached clients and idle past
  the threshold, which defaults to 90 minutes. The kills happened at 56 and 72 minutes.
- The repeating `Timed out draining daemon mutations for idle eviction` line is the sweep being
  **locked out**, not being aggressive. It waits for `waitForDrain(0, ...)`
  (`daemon-supervisor.ts:849-853`) — zero in-flight mutating commands daemon-wide — and
  `prompt_and_wait` is a mutation held open for the whole hour an agent thinks
  (`daemon-supervisor.ts:1359`, `mutationDrain.begin()`). With concurrent runs the count never
  reaches zero, so the sweep timed out every five minutes for hours and reclaimed nothing.

## What actually happens

1. herdr's server log records pane 806 (pid 69491 — the exact process pce started at
   00:24:29 for that dispatch) exiting on **SIGTERM** at `2026-08-18T01:21:08.040`.
2. Six milliseconds later, at `01:21:08.046`, the daemon logs the shutdown that destroys
   worker `b7845a22762e` and its three sessions.
3. The link between them is `print-mode.ts:80-93`: `prime-agent -p` installs handlers for
   SIGINT, SIGTERM and SIGHUP, and each one calls `connection.dispose()`. For an owned session
   `dispose()` sends `complete_owned_session`
   (`packages/coding-agent/src/modes/agent-connection/daemon-agent-connection.ts:1328`), which
   the supervisor executes as "stop this worker" — destroying every session in it, including
   the prompt that was mid-flight.

So a shutdown signal delivered to the client is a death sentence for an hour of uncommitted
agent work. The same shape appears at 00:24:28 for worker `5efa192e4fb2` (2 sessions).

**Who sends the first SIGTERM is not established, and this work does not wait on the answer.**
It is not pce — the pce repository contains no process-killing code at all. It is not herdr
closing a pane on command — no `pane close` request appears in the herdr log at that instant;
herdr's teardown lines come *after* the child's exit, so herdr is the reaper, not the trigger.
The strongest remaining suspect is prime-agent's own stale-daemon reaper, which signals whole
**process groups** rather than processes (`packages/coding-agent/src/cli/daemon-ps.ts:1058`,
via `signalProcessGroupOrProcess` at `packages/coding-agent/src/utils/child-process.ts:54-66`,
which does `process.kill(-pid, signal)`); reaping one dead daemon there would take out every
live process sharing that group, a sibling dispatch's client included. Acceptance criterion 7
below either catches this or clears it. Either way the fix must make the run correct without
knowing the sender.

## The three defects to fix, as one package

1. **A signalled client destroys instead of releasing.** On SIGINT/SIGTERM/SIGHUP, a headless
   client with work in flight must release ownership of its agent and exit, never issue an
   order that destroys a session mid-prompt. The mechanism already exists:
   `promote_owned_session` (`daemon-supervisor.ts:1548`, `promoteOwnedWorker` at `:2132`)
   clears `ownerClientId` so the worker becomes resident, and `detach` releases the client's
   attachment. Decide and justify what happens when promotion itself fails — a client that
   cannot release must not fall back to destroying.
2. **Reclamation starves on a busy daemon.** `waitForDrain(0, ...)` makes the sweep's ability
   to run depend on unrelated sessions being idle, so on this machine it has reclaimed nothing
   for days and roughly ninety agent processes have orphaned to PID 1 (some five days old).
   The sweep must be able to act on a genuinely idle worker while other workers are streaming.
   The fence exists to keep eviction from racing a mutation against the worker *being evicted*;
   preserve that property, do not simply lower the number.
3. **A dying session is not reported to whoever is waiting.** The supervisor does write a
   failure back on this path (`daemon-supervisor.ts:1370-1379`), so "the error is never sent"
   is not the whole story — establish empirically where it is lost: a closed client socket, or
   a client that receives it and keeps waiting anyway. Whatever the answer, a client whose
   session dies must fail within seconds instead of waiting out
   `DAEMON_LONG_RUNNING_REQUEST_TIMEOUT_MS` (24h,
   `daemon-agent-connection.ts:102`). Do not implement this as "forward the error message" alone
   — criterion 4 exists specifically to break that implementation.

## Grilled decisions (settled — do not reopen)

- **Durable, not disposable.** A dispatched agent's work outlives its client. Rationale and
  accepted cost are in ADR-0021.
- **Defects 1 and 2 ship together.** Durability without working reclamation converts every
  abandoned agent into a permanent orphan, which is the state this machine is already in.
- **The signal's origin is out of scope** as a blocker. Report what criterion 7 shows.
- **Reattaching a released agent is out of scope.** That is later pce driver work.
- **Do not re-fix pce-side matters**: attempt-scoped temporary directories (landed 2026-08-18)
  and the driver closing dead or inconclusive dispatches on restart (ADR-0020) are done.
- **Cleaning up the ~90 existing orphans is a hand action**, not code in this package.

## Acceptance criteria

1. **Signal releases instead of destroys** — input: start a headless `-p` session, let its
   prompt begin, send SIGTERM to the client process. Observation: the client exits, and the
   daemon still lists that session streaming, with the worker no longer client-owned.
2. **Work survives its client** — input: signal the client of a session whose prompt writes a
   named file after roughly three minutes of work. Observation: the file appears on disk after
   the client process is gone.
3. **Death is reported in seconds** — input: with a prompt in flight, SIGKILL the worker
   process. Observation: the client's await rejects within ten seconds naming the worker's
   death, and the client process exits non-zero.
4. **Silence is impossible even when the reply channel is gone** — input: SIGKILL the worker
   *and* the supervisor together, so no failure message can ever be delivered. Observation: the
   client still exits non-zero within ten seconds rather than hanging. (Built to break the fix:
   an implementation that only forwards the supervisor's error fails here.)
5. **Reclamation works on a busy daemon** — input: idle-eviction threshold set to one minute;
   one released idle agent and one neighbour with a long prompt in flight on the same daemon.
   Observation: the idle one is reclaimed within two sweeps while the neighbour keeps running.
6. **A working agent is never reclaimed** — input: threshold set to one minute, a session with
   a ten-minute prompt in flight and no client attached. Observation: still alive and streaming
   after ten minutes.
7. **Stopping one agent does not touch its neighbours** — input: two dispatches sharing one
   daemon under one temporary directory; stop or reap one of them. Observation: the other's
   client process and worker are both still alive and its prompt still streaming.

8. **The installed binary carries the fix** — input: after installing your build, run the
   installed `prime-agent --version` (or the bundle's own identity check) and repeat criterion 1
   against a session started through the installed binary rather than the repo build.
   Observation: the signalled client releases its agent, exactly as in criterion 1.

**Installing is yours to do — do not stop for the human.** Work out how this repository installs
(`install.sh`, `package.json`, `prime-agent.sh`, `scripts/`) and replace the bundle at
`/opt/homebrew/lib/node_modules/prime-agent/dist/bundle/` yourself once criteria 1-7 pass
against your build. Two boundaries on that install, both load-bearing:

- **Never kill or restart a running daemon or worker to make the install take effect.** Six live
  PCE runs on this machine are using those daemons right now, and killing one destroys exactly
  the work this package exists to protect. Replacing the bundle on disk is enough: daemons
  started after the install run the fixed code, and existing ones age out on their own.
- **Verify the install landed** rather than assuming it: read back the installed bundle and
  confirm it contains your change, and record in your result what you ran and what you observed.

If the repository's install path cannot be executed without a daemon restart, do not restart —
stop there, install nothing, and report the exact command plus why it needs coordination.

## Environment facts you will need (you cannot ask)

- Repo: `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`. TypeScript, npm, `test.sh` at the
  root, biome for lint. The binary under test is the installed bundle named above, not the
  repo's build — check before assuming.
- Key code: `packages/coding-agent/src/modes/print-mode.ts` (signal handlers, `:80-93`);
  `packages/coding-agent/src/modes/agent-connection/daemon-agent-connection.ts`
  (`dispose` at `:1310-1337`, `promoteToResident` at `:1340`, 24h timeout at `:102`);
  `packages/coding-agent/src/modes/daemon/daemon-supervisor.ts` (`handleLine` error path
  `:1352-1382`, `handleCommand` `:1386`, `complete_owned_session` `:1537`, `promoteOwnedWorker`
  `:2132`, sweep `:796-886`, constants `:148-152`);
  `packages/coding-agent/src/modes/daemon/mutation-drain-latch.ts`;
  `packages/coding-agent/src/core/session-action-store.ts` (`canEvictWorker`,
  `isIdleEvictionThresholdMet`, `:373-422`);
  `packages/coding-agent/src/cli/daemon-ps.ts` (reaper, `:1058`);
  `packages/coding-agent/src/utils/child-process.ts` (`signalProcessGroupOrProcess`, `:54`).
- Live evidence, already read and quoted above — re-read it rather than trusting this summary:
  `~/.prime/agent/logs/daemon.sock.890b2c64.log` (supervisor, 78 lines, the whole incident),
  `~/.prime/agent/logs/agent.jsonl` (same events, structured),
  `~/.config/herdr/herdr-server.log` (pane 806 death at `2026-08-18T01:21:08.040`, pane 804 at
  `00:24:29.143`).
- Context for the caller: pce starts each dispatch as `herdr agent start ... -- /usr/bin/env -i
  ... TMPDIR=<dispatch tmpdir> prime-agent -p` (`crates/core/src/herdr_dispatch.rs:700-731` in
  `/Users/nicolaslazaro/Desktop/work/pce`). `TMPDIR` fixes the daemon socket directory, so
  dispatches sharing a `TMPDIR` share one supervisor — which is why one worker's death took out
  its neighbours' sessions.
- herdr, for reference on pane teardown: `/Users/nicolaslazaro/Desktop/thirdparty/herdr`,
  `src/pane.rs:1280-1325` (`shutdown_pane_processes`: HUP → TERM → KILL, 250ms grace each,
  applied to the pane's whole process group).
