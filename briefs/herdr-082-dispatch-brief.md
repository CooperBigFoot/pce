# Brief: herdr 0.8.2 removed the surface pce dispatches workers through

Status: READY TO DISPATCH — no grill. **This blocks all four live runs; nothing can dispatch a worker
until it lands.** The decisions below are evidence-resolvable: examine the evidence, decide,
implement, and record what you decided and why in your completion report and `CONTEXT.md`. Boundary:
if a decision would change ratified doctrine (an ADR, a frozen criterion), stop and report. Written
2026-08-20, minutes after the breakage.

## What broke

herdr was upgraded 0.7.1 → 0.8.2. Every worker spawn now fails immediately:

```
worker spawn failed: herdr command failed with exit status: 2 for
["agent", "start", "pce-58e238351c40c978d394194285f5",
 "--cwd", "/tmp/pce-work-package-worktrees/pce-58e238351c40c978d394194285f5/00-incidence",
 "--workspace", "w0T", "--tab", "w0T:t1", "--no-focus", "--",
 "/usr/bin/env", "-i", "HOME=…", "PATH=…", "PCE_DISPATCH_TMPDIR=…", "PCE_WORKTREES=…",
 "PCE_WORKTREE_0=…", "TMPDIR=…", "USER=…",
 "/Users/nicolaslazaro/.local/bin/pce", "dispatch", "package-worker",
 "--result", "…/.pce/package-results/IPB7/39.json",
 "--required-artifact", "…/package-outcomes/IPB7/12.json", "--",
 "/Users/nicolaslazaro/.local/bin/pce", "package", "agent", "--vision", "…", "--graph", "…",
 "--package", "IPB7", "--outcome", "…", "--brief", "…", "--", "prime-agent", "-p"]
: unknown option: --cwd
```

Observed live in taqsim (`planning/2026-08-19-incidence-python-binding`, IPB7 issuance 12,
`worker-spawn-failed`, driver exited).

## The API change, verified against the installed 0.8.2

**Before** (what `herdr_dispatch.rs:395-411` composes, and its doc comment says so explicitly —
"Compose the installed Herdr 0.7.1 agent-start command"):

```
herdr agent start <NAME> --cwd <DIR> --workspace <WS> --tab <TAB> --no-focus -- <arbitrary argv…>
```

**Now:**

```
herdr agent start <NAME> --kind <KIND> --pane <ID> [--timeout <MS>] [-- [AGENT_ARG]...]
```

Three independent breakages, not one renamed flag:

1. `--cwd`, `--workspace`, `--tab`, `--no-focus` are **gone**.
2. `--kind` is **required**, from a closed list: `pi, claude, codex, gemini, cursor, devin, agy,
   cline, omp, mastracode, opencode, copilot, kimi, kiro, droid, amp, grok, hermes, kilo, qodercli,
   qwen, maki`. **`prime-agent` is not on it.** pce does not start a "supported interactive agent" —
   it starts an env-scrubbed `pce dispatch package-worker` chain that itself execs `prime-agent -p`.
3. The trailing `--` arguments are now **agent arguments**, not a command vector. There is no longer
   a documented way for `agent start` to launch an arbitrary process.

`herdr agent start --help` also states: "The pane must be at its interactive shell prompt."

## What still works — verified, do not re-derive

- **`herdr worktree create`** is unchanged and still takes `--workspace --cwd --branch --base --path
  --label --no-focus --json`. The worktree phase of `compose` (`herdr_dispatch.rs:540-575`) needs no
  change, and it is what returns the location pce currently starts into.
- **`herdr pane split`** takes `--pane/--current --direction --ratio --cwd <PATH>
  --env <KEY=VALUE> --right-click --focus --no-focus`. It carries **cwd and environment**, which is
  most of what the old `agent start` gave us. Its help shows no trailing command vector.
- **`herdr pane`** also has `list, get, read, rename, input, send-keys, process-info, current`.
- **`herdr agent`** retains `list, get, read, send-keys, prompt, rename, focus, wait, attach,
  explain`.

## Your task

Re-seat worker dispatch on the 0.8.2 surface. **Read herdr's own documentation first** — 0.8.2's
release notes say its CLI help now points coding agents at a plain-text guide, a documentation index,
and a built-in control skill (`https://herdr.dev/agent-guide.md` is named in `herdr agent --help`).
Establish from that documentation what the supported way is to run a non-agent command in a pane,
rather than inferring one from `--help` alone. If the answer is "split a pane, then send the command
to it", say so and cite where the documentation says it.

**Properties that must survive — these are why the current code looks the way it does:**

- **Environment scrubbing.** The worker gets `/usr/bin/env -i` plus exactly the declared
  `--worker-env` names and `PATH`, `HOME`, `USER`, `TMPDIR`, `PCE_DISPATCH_TMPDIR`, `PCE_WORKTREES`,
  `PCE_WORKTREE_<n>`. `route_environment` (`src/main.rs:2749-2757`) is the ratified boundary that
  makes the worker-environment declaration meaningful rather than advisory. **Values must never reach
  the journal** — `worker-environment-declared` records names only.
- **Restart-stable agent identity.** `derive_herdr_agent_name` (`herdr_dispatch.rs:460+`) derives a
  deterministic name from length-framed vision and package identity. The driver finds its worker
  again by that name after a restart; keep it.
- **Exactly-one-pane identification.** `herdr_dispatch.rs:455-458` errors when more than one newly
  observed pane matches the run-owned start location and cwd. Whatever replaces `agent start`, the
  driver must still be able to name *its* pane unambiguously and record it in
  `dispatch-worker-identified` (which now also carries `session_path`).
- **Cleanup composition.** The module composes exact cleanup for the pane the start returned; a
  different creation path needs a matching closure path, or `dispatch-pane-cleanup` will stop being
  truthful.
- **The two-phase plan stays pure.** `compose` returns an ordered description; execution happens
  elsewhere. Keep that split — it is what makes the module testable without herdr.

**Decisions delegated to you:**

1. The replacement mechanism, established from herdr's documentation, not guessed. If it is
   `pane split --cwd --env …` followed by sending the command, decide how the command is delivered
   (`pane send-keys` typing a shell line is the obvious route and also the most fragile — quoting,
   readiness, and echo all become failure modes; say how you handle each).
2. Whether the pane `worktree create` already returns can be reused directly, since `agent start`
   now *requires* an existing pane and `worktree create` produces one. This may be the smallest
   correct change; establish it rather than assuming it.
3. How readiness is determined now that `--timeout`/`agent wait` semantics differ, and what a spawn
   failure looks like as a typed journal event. A spawn that fails must not be retryable forever —
   see `briefs/composition-retry-storm-brief.md`, which is the same lesson from this morning.
4. Whether to pin a minimum herdr version and refuse loudly on a mismatch. Recommendation: yes, and
   report the detected version in `recovery-configured` or at launch, so the next upgrade produces a
   named refusal instead of `unknown option`.

**If the adaptation turns out to be larger than a re-seating — for example if no supported path
exists to run an arbitrary command in a pane — stop and report rather than half-landing it.** The
fallback is reinstalling herdr 0.7.1, which is the operator's call, not yours.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `5360580`, pushed and clean.
- Merge only in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness`; full suite there (`cargo fmt --check`,
  `cargo clippy --workspace --all-targets`, `cargo test --workspace`). Known parallel-load flake:
  `tests/dispatch.rs gate_execution_echoes_large_input_without_deadlock` — verify in isolation.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh`. That symlink
  is a fleet install across four runs; the supervisor owns installation and will confirm it.
- Installed herdr is **0.8.2**; `herdr server` is running and was restarted by the update, so there
  is no client/server version skew. `~/.local/bin/pce` is at digest `101d5e85…`.
- Key code: `crates/core/src/herdr_dispatch.rs` (whole module; `agent_start` at `:392-412`,
  worktree composition at `:540-575`, agent-name derivation at `:460+`, pane-ambiguity error at
  `:455`), `src/main.rs:2749-2757` (`route_environment`), and the dispatch execution path in
  `src/main.rs` around `:7238` and `:4000` where `Command::new("herdr")` is invoked.
- Live evidence: taqsim's journal tail holds the failing `worker-spawn-failed` record with the exact
  argv above.

## Tests

Ship tests that would have caught this. At minimum: the composed dispatch invocation is asserted
against the 0.8.2 flag surface (the existing tests assert `"herdr"` and the old argv — they passed
while production broke, which is the real lesson); environment scrubbing still admits exactly the
declared names plus the fixed five; and a spawn failure produces a typed event and does not loop.

## Waiting consumers — all four, all stopped

- **taqsim** `2026-08-19-incidence-python-binding` — plan version 3, IPB7 issuance 12 spawn-failed.
  Seven of eight packages complete; this is the last one before assembly.
- **bluesmith** `2026-07-29-signal-bearing-dudh-warm-window` — plan version 11 just frozen, W8
  dispatched at issuance 99 and will fail identically.
- **pourpoint** `2026-08-07-declare-grit-d8-...-row-seam` — plan version 14, GD2 parked at 29.
- **hfx** `2026-08-07-close-the-seven-basin-coverage-gap` — plan version 8, SB3 ladder exhausted.

A healthy first pass: a driver dispatches a worker into a fresh worktree pane, the worker's
environment contains exactly the declared names plus the fixed five, `dispatch-worker-identified`
names one pane and one session path, and the driver observes the worker to completion and cleans the
pane up. Verify it against a real run, not only a unit test.
