# pce

`pce` packages a small Rust CLI, seven Claude Code skills, and an installer for
running a Planner-Critic-Executor / PR-review multi-agent development workflow.
The workflow is referred to by the skills as PCE-PR-C.

Post-run orchestrator reviews can be recorded using the
[PCE orchestrator feedback guide](orchestrator-feedback/README.md).

- `pce vision new "<name>"` creates an idempotent dated vision directory under
  `planning/`, seeds it with the fixed `vision.md` template, and prints the
  relative path.
- `/to-vision` turns the current conversation's shared understanding into the
  seven-section `vision.md` used by the workflow.
- `/pce` takes a vision directory, decomposes it into milestones and steps,
  drives plan, critique, execution, PR review, and merge, and persists progress
  in `state.json` so runs can resume.
- `install.sh` builds the release binary, symlinks it to `~/.local/bin/pce`,
  and symlinks and verifies the current seven-skill set in
  `~/.claude/skills/`: `pce`, `to-vision`, `domain-modeling`,
  `grill-with-docs`, `chart-program`, `work-ticket`, and `land-ticket`.

## Program layer

The Program layer coordinates a multi-vision idea through Chart, Work, and Land.
`/chart-program` maintains the Program Map (`pce:program`) and its vision-sized
Effort tickets (`pce:ticket`), `/work-ticket` prepares one ticket for the
single-vision workflow, and `/land-ticket` records its delivery and reveals the
next Frontier. See the [Program-layer workflow](docs/program-layer.md) for the
complete state model and operational guide.
