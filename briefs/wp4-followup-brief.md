# WP4 follow-up — completion is a file on disk, not a herdr status

Your WP4 work stands. Composition, the agent-name derivation, and merge observation are verified and
correct, and your refusal to fabricate a completion from `unknown` → `agent_not_found` was the right
call. This closes the gap that refusal exposed.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Continue on `feature/wp4-herdr-dispatch`, worktree
`/Users/nicolaslazaro/Desktop/work/pce-wp4-dispatch`, last commit `aab3a72`.

That branch stacks on `feature/wp1-work-package-graph`, which stacks on
`repair/orchestration-dead-ends`. None of the three is merged.

## Context you need, stated rather than assumed

**Local checkouts of the tools involved:**

- herdr — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. Agent-facing guide at
  `skills/herdr/SKILL.md`, further docs under `docs/`.
- prime-agent — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`. Docs at
  `packages/coding-agent/docs/`. Headless invocation is `prime-agent -p "<prompt>"`; it also has
  `--mode json` for an event stream. It has **no** `--output-schema` and cannot be made to emit a
  schema-validated result object.

**Version reality, and this reverses earlier guidance you were given:**

- Installed and running: **herdr 0.7.1**.
- Checked out at the path above: **0.8.0**.

The checkout is *ahead* of what actually runs. **The installed binary is the authority.** Where the
checked-out skill and the installed behaviour disagree, the installed behaviour wins — which is why
your finding that 0.7.1 creates a pane on `agent start`, contradicting the 0.8.0 skill, was correct
and should be trusted over the document.

**herdr does not recognise prime-agent.** Its complete list of recognised agent kinds, from
`herdr integration`, is:

```
pi  omp  claude  codex  copilot  devin  droid  kimi
opencode  kilo  hermes  qodercli  cursor
```

`prime-agent` is absent, and `pi` is pi-mono's separate `pi` binary (herdr resumes it with
`pi --session <ref>`). So a prime-agent worker will be classified `unknown` exactly as your
`/usr/bin/true` worker was, and will vanish on exit without ever reporting `done`. The sequence you
observed is not a quirk of the test worker; it is what real workers will do.

**Therefore herdr's role narrows, deliberately.** It provides isolation, spawning, and a pane a
human can look at. It does not provide completion, and must not be asked to.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool right now:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in `/Users/nicolaslazaro/Desktop/work/pce` replaces the binary those runs
invoke.

1. Work in your existing worktree, never the main checkout. Keep your own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.
4. Additive only: the existing `pce dispatch codex` / `pce dispatch gate` verbs, the anchored role
   registry, the placeholder vocabulary and the route-anchor doctrine in `skills/pce/SKILL.md` all
   stay untouched. Retiring them is WP9's job.

---

## What must be true when you are done

### The worker records its own completion

The composed invocation wraps the real worker command so that, whatever the worker does, its exit
is written to a file when it stops. At minimum the file carries the exit status and the wall-clock
time it stopped.

The wrapper must survive the worker exiting non-zero, being killed by a signal, or terminating in
any ordinary way. Only the machine losing power should be able to prevent it.

### That file lives outside every worktree

This is not a preference. PCE has already been bitten by writing a dispatch result into the tree it
was measuring: the executor is required to leave its worktree clean, and the harness dirtied it.
A package may also span several repositories, so no worktree is the natural home anyway.

Put it somewhere the binary owns, derived from the vision directory and the package id.

### Completion is derived from that file, and from nothing else

A package dispatch is complete when its result file exists and parses. One completion record is
appended, carrying the exit status.

A **non-zero exit is a result, not an error.** The dispatch completed; the work failed. Those are
different facts and the log must distinguish them.

An issuance whose worker is gone with **no** result file is not complete and never becomes
complete by inference. It is a productless attempt, which the existing accounting already models,
and it stays open until reconciled.

### herdr is not consulted about completion, anywhere

No `herdr agent wait --status done`, no treatment of `unknown` as finished, no treatment of a
vanished pane or `agent_not_found` as success. If a pane still exists that is at most a hint the
worker is still running; it is never evidence that it finished.

This is PCE's own existing rule, restated: only a completion record or a durable reconciliation
closes a dispatch, and a child's observable behaviour is never read as a substitute.

### It survives a restart

The driver will be killed and restarted. Discard every scrap of in-process state, re-derive from
the event log and the filesystem, and reach the identical answer about which packages are running,
which finished, and with what status.

---

## Acceptance criteria

1. The composed argv wraps the worker such that an exit of `0` writes a result file recording `0`.
2. An exit of `3` writes a result file recording `3`, and produces one completion record whose
   status is `3` — not an error, not a missing completion.
3. A worker killed by `SIGKILL` before finishing writes **no** result file, and the issuance remains
   open and enumerable as unaccounted. Nothing infers success or failure from its absence.
4. The result file path is outside every worktree the package uses. Prove it with a two-repository
   package.
5. Completion state re-derives identically from disk alone, with no in-process state: same running
   set, same finished set, same statuses.
6. No code path in completion derivation consults herdr's agent status. Make this checkable rather
   than asserted.
7. Every existing WP4 test — composition, agent naming, merge observation — still passes unchanged.
8. WP1's five readiness waves still pass.

**Plus one real spawn, actually executed and reported.** A worker that sleeps about two seconds and
then exits `3`, started through herdr 0.7.1, producing exactly one issuance and one completion
carrying exit `3`. Gate it on herdr being available so CI can skip it, but run it once and report
the observed sequence, as you did last time. That report is what proved the previous gap; do it
again.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, and `cargo test --workspace`.

---

## Do not touch

- Composition, agent-name derivation, and merge observation — all verified, leave them.
- The step tier, `milestones.json`, `steps.json`, and their readiness path.
- What a worker's prompt actually says — that is WP5.
- Gates (WP6), the driver loop (WP7), retiring the old tier (WP9).
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce` — open questions, not work items.

## Report back with

- Where the result file lives and how its path is derived.
- Exactly what the wrapper does, and which ways of dying it survives.
- How "no result file" is kept distinct from "exited non-zero", in the log.
- The real-spawn evidence: the command, the observed transitions, the resulting records.
- Anything you found that contradicts what this brief assumes.
