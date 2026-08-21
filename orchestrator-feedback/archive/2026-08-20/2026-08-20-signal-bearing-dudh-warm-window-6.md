# PCE workflow feedback: pce and herdr diverged at the agent-start interface mid-run

- Date: `2026-08-20`
- Orchestrator: `Claude Code / work-graph skill, session b2d7df0c`
- Run: `bluesmith planning/2026-07-29-signal-bearing-dudh-warm-window`, plan version 11
- Outcome: `blocked; not repairable within the supervising skill's authority`

## Executive summary

A `herdr` upgrade landed three minutes before a dispatch and removed the `agent start` options `pce`
emits. Every worker spawn now fails immediately. `pce` at `main` is affected, not merely the installed
binary, so rebuilding does not help. The run itself is undamaged — the package retains its full
dispatch budget and the frozen graph needs no revision — but no work can proceed until the two tools
agree again.

## Evidence reviewed

- `driver-journal.jsonl` record 841, `worker-spawn-failed` for W8 issuance 99
- `herdr --version`, `herdr agent start --help`, and the binary's mtime
- `pce/crates/core/src/herdr_dispatch.rs` at `main` (5360580)

## Friction and failures

### `herdr agent start` dropped the options `pce` passes

- Severity: `high`
- Phase: `execution`
- Observation: dispatch fails with `herdr command failed with exit status: 2 ... unknown option: --cwd`.
- Evidence: the installed `herdr` is version 0.8.2 with mtime `2026-08-20 12:40`; the failing spawn is
  timestamped 12:43. Its usage line is now

  ```
  herdr agent start <NAME> --kind <KIND> --pane <ID> [OPTIONS] [-- [AGENT_ARG]...]
  ```

  `--cwd`, `--workspace`, `--tab` and `--no-focus` no longer exist. `pce` emits `--cwd` and
  `--no-focus` at six call sites in `crates/core/src/herdr_dispatch.rs` — lines 405/411, 553/568,
  683/693, 720/726, 768/778. Issuances 97 and 98 dispatched normally at 10:45 under the previous
  herdr.
- Inference: a breaking change in a dependency `pce` shells out to, landing between two launches of
  the same run.
- Impact: total. No worker can be dispatched by any vision on this machine until it is resolved.

### The coupling is unversioned

- Severity: `medium`
- Phase: `execution`
- Observation: `pce` discovers the incompatibility by shelling out and reading an error string.
- Evidence: the failure surfaces as `unknown option: --cwd` inside a `worker-spawn-failed` reason,
  after the base was composed and an issuance was consumed.
- Inference: there is no version assertion between the two tools.
- Impact: the failure appears at the worst moment — mid-dispatch, per package — rather than once at
  startup, and it reads as a run problem rather than an installation problem.

## Recommendations

### Assert the herdr version once, at driver startup

- Addresses: finding 2
- Change: check `herdr --version` against a supported range when the driver starts and refuse with a
  plain statement of the mismatch, rather than discovering it per dispatch.
- Location: driver startup, alongside the existing tool checks
- Trade-off: the range needs maintaining as herdr moves.
- Confidence: `high`

### Update the dispatch call sites to the `--kind`/`--pane` form

- Addresses: finding 1
- Change: adapt the six `herdr_dispatch.rs` call sites. Note the new interface is not a rename: it
  requires a pane already at an interactive shell prompt and an explicit agent kind, so pane creation
  and readiness become `pce`'s responsibility rather than `herdr agent start`'s.
- Location: `crates/core/src/herdr_dispatch.rs`
- Trade-off: more state for `pce` to manage; worth confirming against herdr's own migration guidance
  before writing it.
- Confidence: `medium` — the required shape is clear, the intended migration path is not.

## No-change decisions

The supervising skill did not attempt either fix. Reinstalling a different `herdr` is not available
(no older binary and no herdr source tree on this machine), and editing `pce` mid-run would change the
instrument the run depends on for its own proof.

## Suggested follow-up

Confirm whether any other run dispatched successfully after 12:40 today. If none did, every active
vision on this machine is blocked on the same cause and the fix is worth prioritising over the
per-vision work queued behind it.
