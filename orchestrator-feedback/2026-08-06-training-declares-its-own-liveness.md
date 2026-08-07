# PCE workflow feedback: training declares its own liveness

- Date: `2026-08-06`
- Orchestrator: Claude Code, Opus 5, fresh ultracode session
- Run: `palaestra/planning/2026-08-06-training-declares-its-own-liveness` (cross-repo: `palaestra` + `nostos`, both Python/uv)
- Outcome: `blocked at Phase 0, then unblocked by a tracked-file workaround` — no milestone graph authored, no dispatch issued

## Executive summary

The run could not reach the end of Phase 0's first command. `pce contract check` runs stated gates
under a macOS Seatbelt profile that permits writes only under the repository root, and `uv` must
write its cache before it runs anything. Every gate in both repositories therefore failed with
`Operation not permitted (os error 1)` on `~/.cache/uv`, and no repository contract could be
measured — which makes `pce status`, `pce ready`, and every dispatch unreachable, because status
precondition 2 requires at least one contract.

Three findings are new. The highest-impact one is the Seatbelt profile above (F1). It is made
unworkaroundable-in-place by F2: the hermetic environment already reported in
`2026-08-05-dispatch-spawns-with-an-empty-environment.md` also applies to contract-measurement
gates, so `UV_CACHE_DIR` — the escape hatch `uv` documents for exactly this situation — cannot
reach the child. F3 is a fresh-run contradiction: `pce contract bootstrap` fails unless the event
log already exists, but on a fresh run bootstrap is the command that writes the log's first record,
and hand-creating the file to satisfy it produces an empty log that `SKILL.md` declares a loud stop.

A fourth finding reproduces `2026-08-01-bootstrap-is-rust-only.md` exactly, in a second repository
on a second date, and confirms that the workaround that report prescribes does work — with one
undocumented detail that cost a parse-error round trip.

I did not modify `pce`; the operator ruled that out for this run.

## Evidence reviewed

- Terminal output of `pce contract check` against both repositories, sandboxed and unsandboxed.
- Terminal output of `pce contract bootstrap --repository nostos`.
- `strings /Users/nicolaslazaro/.local/bin/pce` (the installed binary; no local build was made).
- `pce` at `3edd669558bb808497a78537971a2be5ed270b10` (`3edd669`, "a-gate-runs-what-was-built/milestone-5").
- `uv 0.12.1 (329541a50 2026-07-31 aarch64-apple-darwin)`, macOS Darwin 24.6.0.
- `palaestra/.pce/repository-contract.json` at `fc73348`; `nostos` at `ba914ad`, no tracked contract.
- Prior reports `2026-08-01-bootstrap-is-rust-only.md` and
  `2026-08-05-dispatch-spawns-with-an-empty-environment.md`.

## What worked

### The mandatory status probe as the first operation

- Evidence: `pce status --file …/events.jsonl --vision-dir …` failed with
  `failed to open event log …` / `Caused by: No such file or directory (os error 2)`.
- Effect: the typed cause was visible in the CLI rendering, so the missing-log discrimination
  `SKILL.md` requires could be made directly rather than by the documented fallback of confirming
  the path does not exist. Fresh-run classification took one command.

### Contract measurement running before any planning

- Evidence: the blocker surfaced at Phase 0's first gate, before a milestone planner, critic, or
  any Codex child had been dispatched.
- Effect: an environment fault that would have failed *every* executor in *every* worktree was
  found at zero dispatch cost. Had gate measurement been deferred to Phase 3 isolate, the same
  fault would have surfaced after a milestone graph, step graphs, and plans had all been authored
  and approved. This ordering is worth preserving.

## Friction and failures

### F1 — Contract-measurement Seatbelt profile denies every write outside the repository root, so no `uv` gate can run

- Severity: `high` (blocking; the run cannot begin)
- Phase: `orientation / repository contracts`
- Observation: every stated gate fails before doing any work. First gate, palaestra:

  ```
  error: Failed to initialize cache at `/Users/nicolaslazaro/.cache/uv`
    Caused by: failed to open file `/Users/nicolaslazaro/.cache/uv/sdists-v9/.git`:
               Operation not permitted (os error 1)
  Error: failed to measure tracked repository contract
  Caused by: stated gate command `uv run ruff format --check .` exited with status 2
  ```

- Evidence, as an ordered elimination:
  1. The same command run directly: `56 files already formatted`, exit 0.
  2. Run under `env -i PATH=… HOME=… USER=…`: also green — so it is not the cleared environment
     (this distinguishes F1 from F2).
  3. `pce contract check` fails identically whether or not the *harness* sandbox is disabled — so
     it is not the calling agent's sandbox.
  4. The named file opens for both read and append from Python (`r OK`, `a OK`), is
     `-rw-r--r-- nicolaslazaro staff`, and carries no `chflags`. After `rm`-ing it, the same EPERM
     recurs on a path that no longer exists — so the denial is on *creating* in that directory, not
     on that file's metadata.
  5. `strings` on the binary shows `/usr/bin/sandbox-exec`, `(deny file-write*`, and
     `Seatbelt envelopes must use the contract-measurement process adapter`.
  6. Probe with cache redirected to `/private/tmp/claude-501/uv-cache-probe`:
     `failed to open file `…/CACHEDIR.TAG`: Operation not permitted` — `/tmp` is denied too.
  7. Probe with cache redirected inside the repository root: all five gates ran green
     (palaestra 338 tests passed, `uv build` succeeded; nostos 170 passed).
- Inference (distinguished from the above): the profile grants the workspace and denies the rest of
  the filesystem, and cargo repositories do not hit this because their gates need no writable
  location outside the workspace on a warm toolchain — or because `~/.cargo` is already exempted. I
  did not read the profile construction, only the strings, so which of those is true is unverified.
- Impact: total. `SKILL.md` requires a measured `repository-contract` record before anything else;
  without it `pce status` fails its second precondition, so readiness, dispatch, and merge are all
  unreachable. Any Python/uv repository is affected at the first command of the first phase. The
  same profile also denies `/tmp`, which made `pytest` fall back to the working directory and write
  `pytest-of-<user>/` into the repository — a second, quieter consequence, and one that dirties a
  worktree that the executor contract requires to carry exactly one commit.

### F2 — The hermetic environment extends to contract-measurement gates, so the per-tool escape hatch is unreachable

- Severity: `medium` (it converts F1 from configurable to structural)
- Phase: `orientation / repository contracts`
- Observation: exporting `UV_CACHE_DIR=<writable path>` in the calling shell and re-running
  `pce contract check` produced an error still naming `/Users/nicolaslazaro/.cache/uv`.
- Evidence: the error path in the output is the default cache, not the exported one; by contrast
  the same variable expressed through `uv`'s config file *was* honoured — a subsequent probe with
  `~/.config/uv/uv.toml` failed naming `/private/tmp/claude-501/uv-cache-probe`, the probe path.
  So the child reads `$HOME`-based config but receives no inherited environment.
- Inference: the empty-environment policy documented in
  `2026-08-05-dispatch-spawns-with-an-empty-environment.md` for `pce dispatch` also governs
  `contract check`. That report argues the hermetic default is defensible for dispatch, and I agree.
  The consequence here is narrower and worth stating separately: it removes the only mechanism by
  which an operator could point a tool at a permitted location without editing a tracked file.
- Impact: the workaround for F1 cannot be environmental. It must be a file inside the repository,
  which means a tracked change to every affected repository — see the workaround section.

### F3 — `pce contract bootstrap` requires the event log to exist, but on a fresh run it is what creates the log's first record

- Severity: `medium`
- Phase: `startup / orientation`
- Observation: on a fresh run with no `events.jsonl`, bootstrap exits non-zero without doing any
  work:

  ```
  Error: failed to open event log …/events.jsonl
  Caused by: No such file or directory (os error 2)
  ```

- Evidence: `SKILL.md` § *Startup and resume* names bootstrap as the fresh-run path — "use
  `pce contract bootstrap` when `.pce/repository-contract.json` is absent at default-branch HEAD" —
  and the fresh-run sequence has no earlier command that appends. `pce log` was not invoked before
  this point precisely because no contract had been established yet.
- Inference: bootstrap opens the log for the read-and-validate pass before its append, and that
  pass does not treat NotFound as an empty history.
- Impact: the orchestrator must hand-create the file (`: > events.jsonl`) to proceed, and the same
  document states that "every existing empty, unreadable, malformed, ambiguous, or otherwise
  unfoldable log stops loudly". The documented fresh-run path therefore forces the orchestrator to
  manufacture a state the same document declares fatal, and leaves it there if the very next
  command fails — which is exactly what happened here, since bootstrap then failed again on F4. I
  removed the empty file rather than leave a poisoned resume.

### F4 — `bootstrap` is Rust-only: reproduced, plus one undocumented schema detail

- Severity: `low` (already reported and triaged; recorded as a second datapoint)
- Phase: `orientation / repository contracts`
- Observation: `pce contract bootstrap --repository nostos` (Python/uv, one GitHub Actions
  workflow) failed with
  `CI-derived bootstrap found no format gate command in workflows: ci.yml`, although `ci.yml`
  contains `run: uv run ruff format --check`.
- Evidence: reproduces `2026-08-01-bootstrap-is-rust-only.md` on its CI-derived branch, including
  that report's prediction that the diagnostic "reads as 'your workflows are missing a gate'". The
  installed binary's strings confirm the cargo-only candidate lists
  (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, …) and the CI-less guard
  `cannot bootstrap CI-less repository without Cargo.toml at default-branch HEAD`.
- Inference: none needed; the prior report already established the cause from source.
- Impact: small, because the prescribed workaround works. Hand-authoring
  `nostos/.pce/repository-contract.json` and running `pce contract check` against it measured all
  five gates green. One detail cost a round trip: the tracked contract's workflow stand-in is an
  internally-tagged enum whose variants are **uppercase**, and `"kind": "command"` is rejected with
  `unknown variant `command`, expected `COMMAND` or `NONE``. The message is good; the schema is
  discoverable only by provoking it, since no example of a repository *with* workflows exists in
  either repo here (palaestra has none and declares `"workflows": []`).

## The workaround actually used

Recorded because `SKILL.md` asks that improvisations become explicit rules, and because it is the
only Python-repo path I found that survives both the Seatbelt profile and the hermetic environment.

A tracked `uv.toml` at each repository root:

```toml
cache-dir = ".uv-cache"
```

with `.uv-cache/` and `pytest-of-*/` added to `.gitignore`. The path is relative, so it resolves
per-project and therefore also inside every step worktree — which matters, since worktrees are cut
from a ref and an untracked file would not be present in them.

Verified after applying it: both repositories measure all five gates green,
`git status --porcelain` stays clean apart from the intended edits, and the built sdist and wheel
contain no cache entries (`tar tzf dist/palaestra-0.1.85.tar.gz | grep -c uv-cache` → `0`; 38
entries, top level `PKG-INFO`, `pyproject.toml`, `README.md`, `src`). That last check matters
because `uv build` warns `The cache directory `.uv-cache` is inside the build source directory `.`
and may be included in distributions` — the warning is advisory here, but it would not be if a
repository's packaging did not honour `.gitignore`.

The cost is real and worth naming: every uv repository that runs PCE must now carry a `uv.toml`
whose only reason to exist is the sandbox profile, plus two `.gitignore` entries, plus a cold cache
per repository instead of one shared warm cache.

## Recommendations

### R1 — Permit the platform package-manager cache in the contract-measurement profile

- Addresses: F1
- Change: add the standard per-tool cache locations as writable roots in the Seatbelt profile used
  by the contract-measurement adapter — at minimum `~/.cache/uv` and `$TMPDIR`, alongside whatever
  cargo already relies on. `$TMPDIR` additionally stops `pytest` from writing `pytest-of-*/` into
  the repository under test.
- Location: the contract-measurement process adapter in `src/main.rs` (the code behind
  `Seatbelt envelopes must use the contract-measurement process adapter`) and its profile builder.
- Trade-off: a measured gate could then write outside the workspace, which weakens the hermeticity
  the profile buys. This seems the right trade: a package manager's cache is not run state, and the
  alternative — as this run shows — is that no Python repository can be measured at all.
- Confidence: `high` that this unblocks it; `medium` on the exact set, since I read strings rather
  than the profile.

### R2 — Let `bootstrap` create the event log, or say which command should

- Addresses: F3
- Change: treat NotFound as an empty history in the bootstrap read path and create the log on first
  append. Failing that, name the log-creating command explicitly in `SKILL.md` § *Startup and
  resume*, so the orchestrator does not have to invent `: > events.jsonl` and then reason about
  whether it has just poisoned its own resume.
- Location: `pce contract bootstrap`'s log-open path; or `SKILL.md` § *Startup and resume*.
- Trade-off: the code change slightly weakens the "every existing log must fold" invariant by
  adding one command allowed to create; the doc-only change costs nothing but leaves the
  empty-log-after-a-failed-bootstrap hazard in place.
- Confidence: `high`

### R3 — Ship a tracked-contract example that declares a workflow

- Addresses: F4's undocumented detail
- Change: include one example `.pce/repository-contract.json` with a non-empty `workflows` array
  showing `{"workflow": …, "stand_in": {"kind": "COMMAND", "command": …}}` and the `NONE` variant.
- Location: wherever the tracked contract is documented for hand-authoring — the same place
  `2026-08-01-bootstrap-is-rust-only.md` points operators when it recommends hand-authoring.
- Trade-off: none beyond keeping the example in sync with the payload boundary.
- Confidence: `high`

### R4 — Consider whether `contract check` should report the sandbox as the cause

- Addresses: F1's diagnosis cost
- Change: when a stated gate exits non-zero and its stderr contains an `os error 1` / `Operation
  not permitted` on a path outside the workspace, add one line naming the profile as the likely
  cause. This mirrors recommendation 1 of the empty-environment report, which asked the same of the
  spawn error.
- Location: the stated-gate failure path behind `failed to execute stated gate command \``.
- Trade-off: string-matching a child's stderr is fragile and could mislabel a genuine permission
  bug. Mark as `experimental` for that reason.
- Confidence: `experimental`

## No-change decisions

- **The hermetic environment itself.** F2 is a consequence of it, not an argument against it. The
  prior report's case for the policy stands; the only thing this run adds is that the policy's
  reach now includes gate measurement, which is worth documenting but not reversing.
- **Requiring five stated gates.** nostos's CI declares four (no build step) and `uv build` was a
  reasonable fifth. The fixed five did not cause a failure here — only a small authoring judgement
  — so the schema question raised as item 2 of `2026-08-01-bootstrap-is-rust-only.md` is not
  strengthened by this run.
- **Committing the workaround into the product repositories.** Unpleasant, but with F2 in force
  there is no environmental alternative, and a worktree-visible file must be tracked. If R1 lands,
  both `uv.toml` files should be deleted rather than kept.

## Suggested follow-up

- One issue for R1; it is the only finding that blocks a run outright, and it will recur on the
  next uv repository and on every worktree an executor is dispatched into.
- Fold R2 into whichever effort next touches the fresh-run path; it is cheap and the current
  behaviour hands the orchestrator a state the doctrine calls fatal.
- No follow-up for F4 beyond R3 — `2026-08-01-bootstrap-is-rust-only.md` already scopes the real
  work, and this run only adds a second confirming datapoint and the uppercase-variant detail.
