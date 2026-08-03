# pce

`pce` packages a small Rust CLI, seven Claude Code skills, and an installer for
running a Planner-Critic-Executor / PR-review multi-agent development workflow.
The workflow is referred to by the skills as PCE-PR-C.

Post-run orchestrator reviews can be recorded using the
[PCE orchestrator feedback guide](orchestrator-feedback/README.md).

- `pce vision new "<name>"` creates an idempotent dated vision directory under
  `planning/`, seeds it with the fixed `vision.md` template, and prints the
  relative path.
- `pce vision check` reads one candidate `vision.md` from stdin, writes nothing,
  and refuses malformed acceptance criteria.
- `/to-vision` turns the current conversation's shared understanding into the
  seven-section `vision.md` used by the workflow, including the executable
  acceptance-criteria input/observation contract.
- `/pce` takes a vision directory, decomposes it into milestones and steps,
  drives plan, critique, execution, PR review, and merge, appends durable events,
  and derives current status on read from the event log, git, and GitHub
  authorities. A rewritable `state.json` is not the resumable source of truth.
- `install.sh` builds the release binary, symlinks it to `~/.local/bin/pce`,
  installs the rehydration hook, and symlinks and verifies the current
  seven-skill set in
  `~/.claude/skills/`: `pce`, `to-vision`, `domain-modeling`,
  `grill-with-docs`, `chart-program`, `work-ticket`, and `land-ticket`.

## Rehydration hook activation

After this change is merged, a human must run `./install.sh`; merging a pull
request does not activate a global hook. The installer links the repository
hook at `$HOME/.local/bin/pce-rehydrate` and structurally merges that literal
command into the user's global `$HOME/.claude/settings.json`. It registers only
the `resume` and `compact` matchers under `SessionStart`, and injects only the
derived recovery digest. A project-scoped `.claude/settings.json` or local
`.claude/settings.local.json` containing a `hooks` key overrides the
user-global hook block entirely.

Installation is a hard cutover because a run keeps the skill version loaded
when it was invoked. Finish or abandon every pre-cutover run before running
`./install.sh`; do not install midway through a run.
Python 3 is a host dependency for the settings merge; this adds no Cargo
dependency or C-toolchain risk.

The hook invokes `pce` by bare name through `PATH`. If `$HOME/.local/bin` is
absent from `PATH`, it silently no-ops. This intentional fail-open behavior
matches the installer's PATH warning. Each resume or compaction runs
`pce status`, which fetches each integration branch and calls `gh pr list` once
per canonical step node. This network work occurs inside Claude Code's hook
timeout; timeout or authority failure degrades to no injection. Status
derivation is not offline.

The automated suite proves preservation, self-consistency, and idempotence
against the documented settings shape. Together with the hook tests, it covers
event filtering, selection, fail-open behavior, status invocation, and stdout
shape. It does not prove that Claude Code accepts or invokes the configuration.
After installation, manually exercise a real Claude Code `SessionStart` and
verify invocation and recovery-digest injection for both `resume` and
`compact`, while `startup` remains uninjected. That exercise establishes the
settings contract and actual activation.

## Program layer

The Program layer coordinates a multi-vision idea through Chart, Work, and Land.
`/chart-program` maintains the Program Map (`pce:program`) and its vision-sized
Effort tickets (`pce:ticket`), `/work-ticket` prepares one ticket for the
single-vision workflow, and `/land-ticket` records its delivery and reveals the
next Frontier. See the [Program-layer workflow](docs/program-layer.md) for the
complete state model and operational guide.
