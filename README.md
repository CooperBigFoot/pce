# pce

`pce` packages a small Rust CLI, two Claude Code skills, and an installer for
running a Planner-Critic-Executor / PR-review multi-agent development workflow.
The workflow is referred to by the skills as PCE-PR-C.

- `pce vision new "<name>"` creates an idempotent dated vision directory under
  `planning/`, seeds it with the fixed `vision.md` template, and prints the
  relative path.
- `/to-vision` turns the current conversation's shared understanding into the
  seven-section `vision.md` used by the workflow.
- `/pce` takes a vision directory, decomposes it into milestones and steps,
  drives plan, critique, execution, PR review, and merge, and persists progress
  in `state.json` so runs can resume.
- `install.sh` builds the release binary, symlinks it to `~/.local/bin/pce`,
  symlinks the two skills into `~/.claude/skills/`, and verifies the install.
