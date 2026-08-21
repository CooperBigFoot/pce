We are converting a stuck vision into a work-package graph. Its vision lives at:

  /Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core

I have rebuilt the software I use to develop and implement visions. PCE no longer decomposes a
vision into milestones and steps driven by an orchestrator agent. It now turns a vision into a
work-package graph, and a driver in the binary walks that graph: it composes each package onto its
dependencies' completed work, dispatches prime-agent workers concurrently, executes each package's
acceptance criteria as real shell commands, sends a gate to attack the built artifact, and finally
assembles every package and re-runs all criteria against the assembly.

Run `/to-graph` pointed at that directory. That skill does the whole job in one go: it reads the
vision, works out what has already landed, partitions the remaining acceptance criteria into work
packages, writes each criterion as an executable command, types the dependency edges, checks the
graph mechanically, renders it as an artifact for me to look at, discusses it with me until it is
settled, and only then freezes it.

## This vision spans two repositories

- `/Users/nicolaslazaro/Desktop/work/taqsim` — Python, uv. Gates: `uv run pytest`,
  `uv run ruff check src/ tests/`, `uv run ty check`. Currently at `ceb7dc1`.
- `/Users/nicolaslazaro/Desktop/work/incidence` — Rust, cargo. Gates: `cargo test --workspace`,
  `cargo clippy --workspace --all-targets`. Currently at `fa57982`, with milestone 2 merged via
  pull request #12 on `CooperBigFoot/incidence`.

Both have a tracked contract at `.pce/repository-contract.json`; read both. A package may name one
repository or both, and criteria commands must be written for whichever repository they run in.

This is the first deliberately multi-repository run of the new harness, so treat anything awkward
about two repositories as a finding worth reporting rather than something to work around quietly.

## The previous run is stuck, and its status output lies

The old run reached milestone 3 and blocked: two plan loops exhausted their round caps, and a
human-authorised recovery could not be enacted. It cannot be finished under the old workflow. That
is why we are converting.

**`pce status` reports `merge-status=inconclusive` for all 18 steps. Do not read that as "nothing
landed."** taqsim's integration branches were deleted from its remote, which permanently flips every
step to inconclusive regardless of what actually merged. Meanwhile `incidence` genuinely has merged
work through milestone 2, and still has its `pce/incidence-core/m3-s*` branches locally; taqsim has
no `pce` branches at all.

So establish ground truth yourself, from git and GitHub in both repositories, and state plainly which
acceptance criteria are already satisfied by merged work and are therefore carried by no package.
Getting this wrong in either direction is expensive: claiming something landed that did not leaves a
hole, and rebuilding something that did wastes a package.

## Things to know

- Convert from where the work stands, not from the beginning.
- Do not run the old `/pce` orchestrator on this vision, and do not modify `events.jsonl`. There is
  an `events.jsonl.bak-before-escalation-repair` beside it from an earlier hand repair; leave both
  alone.
- taqsim's working tree is dirty: `uv.lock` is modified and `CONTEXT.md` and `docs/` are untracked.
  Leave them exactly as they are; do not commit or revert them.
- Criteria are bets. Most test files a criterion names will not exist yet, and making them exist and
  pass is the worker's job. A criterion may never be satisfied merely by a test existing.
- When the graph runs, each repository gets its own preparation command — `uv sync` for taqsim,
  whatever is right for incidence. Say what you would use for each, since I will need them on the
  command line.
- I want to collaborate on this. Expect me to ask questions about the rendered graph and to push
  back. Answer them plainly and fix anything my questions expose. Do not ask me to approve edge
  kinds, the partitioning, or how criteria are worded — those are yours.

## Report back with

- What the graph says is being built, and what it deliberately does not cover.
- Which criteria are already satisfied by merged work, and how you established that given that the
  status output is unreliable here.
- Which package is worth taking first, and the exact `pce package driver-run` command that would
  start it, including both `--repository` flags and both `--prepare` commands.
- Anything about two repositories that the tooling made awkward.
