# Brief: a run's panes belong in their own herdr session, not the human's

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-20.

## The problem, measured

`herdr workspace list` on the operator's session returns **157 workspaces, 134 of which are pce
work-package leftovers** — labels of the shape `<PACKAGE>:<repository>:attempt-<n>`, including
`WP1:orthographos:attempt-1..5` from a vision that finished weeks ago. The operator's own seven
named workspaces (`pce`, `taqsim`, `pourpoint`, `bluesmith`, `hfx`, `RivRetrieve`,
`grant-proposal-nik`) are lost among them.

Two causes, and only the second is worth engineering:

1. **Cleanup misses.** pce composes exact workspace closure and emits `dispatch-pane-cleanup` on the
   normal path, but a killed driver, a wedged worker, or a spawn failure leaves its workspace behind
   permanently. Nothing sweeps them later.
2. **The panes are in the wrong place entirely.** The operator never needs to see a work-package
   worker's pane; they supervise through orchestrator panes and the journal. Worker panes are
   machinery, and they are currently created in the same session a human is attached to.

## What herdr 0.8.2 offers — verified against the installed binary

- `herdr session list` → `default running /Users/nicolaslazaro/.config/herdr
  /Users/nicolaslazaro/.config/herdr/herdr.sock`. **Sessions are separate servers with separate
  sockets.**
- `herdr --session <name> <subcommand>` selects a session and resolves to
  `~/.config/herdr/sessions/<name>/herdr.sock`. Probed live:
  `herdr --session pce-work workspace list` →
  `{"error":{"code":"server_not_running","message":"no herdr server is running at
  /Users/nicolaslazaro/.config/herdr/sessions/pce-work/herdr.sock; run 'herdr session attach
  pce-work' to start or attach it"}}`. The flag is honoured; the session's server must exist first.
- `herdr session` also has `attach`, `stop`, `delete`; deleting the default session is refused.
- **`HERDR_SESSION` does not work as a client selector.** The string exists in the binary, but
  `HERDR_SESSION=pce-work herdr workspace list` returned the *default* session's 157 workspaces.
  Do not take the environment-variable route; it silently targets the wrong server.

## Your task

Let a run place its herdr objects in a named session that the operator never attaches to.

**Change:** thread an optional session name through every herdr invocation pce composes — worktree
creation, pane run, pane close, workspace close, and any read or observation call. In
`crates/core/src/herdr_dispatch.rs` the invocations are pure argv vectors, so this is a prefix on
each composed vector (`--session <name>` before the subcommand), plus the execution sites in
`src/main.rs` (`Command::new("herdr")` around `:4000` and `:7238`, and wherever the 0.8.2 pane-run
dispatch now executes).

**Decisions delegated to you:**

1. **Where the name comes from.** Recommendation: a `herdr_session` field in `run.json` alongside
   `tmux_session`, so it is launch configuration the skill already persists and a relaunch cannot
   silently change it. A CLI flag on `driver-run` is the alternative; if you take it, say how a
   relaunch is kept consistent.
2. **Whether the session is journaled.** Recommendation: yes — record the session name in the
   dispatch or launch record, because a pane the operator cannot see must still be findable from the
   run proof. `dispatch-worker-identified` already carries `pane_id`, `workspace_id` and
   `session_path`; a herdr session name belongs beside them.
3. **What happens when the named session's server is not running.** The probe above shows the error
   is typed and clear. Decide between refusing at launch with that message (recommended — it is a
   launch precondition like any other) and starting the session automatically. If you start it
   automatically, say how you avoid racing a concurrent driver doing the same.
4. **Backward compatibility.** With no session configured, behaviour must be exactly as today.
   Existing journals name workspaces in the default session; do not break their cleanup.

**Also in scope, small:** a sweep verb or a documented recipe for closing stale work-package
workspaces — the ones matching `<PACKAGE>:<repository>:attempt-<n>` whose run is no longer live.
134 of them exist right now. If you make it a verb, it must refuse to close a workspace belonging to
a live dispatch; if you make it a recipe, put it in the work-graph skill's recovery section.

**Out of scope:** moving the *driver* itself. Drivers run in tmux (`run.json` `tmux_session`) and the
operator watches those deliberately. This brief is about worker panes only.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `de04614`, pushed and clean.
- Merge only in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness`; full suite there (`cargo fmt --check`,
  `cargo clippy --workspace --all-targets`, `cargo test --workspace`). Known parallel-load flake:
  `tests/dispatch.rs gate_execution_echoes_large_input_without_deadlock`.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh`; that symlink
  is a fleet install. The supervisor owns installation.
- Installed herdr is **0.8.2**; pce enforces `>=0.8.2,<0.9.0` since `de04614`.
- Key code: `crates/core/src/herdr_dispatch.rs` (argv composition — worktree create at `:540+`,
  pane run at `:432+`, pane close at `:301`), `src/main.rs` herdr execution sites and the
  `run.json` parsing that already handles `tmux_session`, `worker_env`, `prepare`, and limits.

## Tests

The composed invocations carry `--session <name>` when configured and are byte-identical to today's
when not. A run.json without the field still parses. Prefer asserting the composed argv, since that
is exactly the surface the 0.8.2 breakage proved is worth pinning.

## The waiting consumer

The operator, who supervises four concurrent runs through orchestrator panes and reads the journals
for everything else. A healthy first pass: a driver launched with a session configured creates no
workspace visible in `herdr workspace list` on the default session, its workers run and complete
normally, `herdr --session <name> workspace list` shows them, and cleanup removes them there.
