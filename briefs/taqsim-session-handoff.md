We are running the second real-world experiment of a rebuilt PCE. Read this whole message before
doing anything; several days may have passed, so verify current state rather than trusting the
details below.

## What PCE is now

PCE used to decompose a vision into milestones and steps, driven by an LLM orchestrator following a
542-line `SKILL.md`. That is retired. PCE is now a specification-and-verification layer over a typed
work-package graph, and a driver inside the binary walks it:

- A **work package** is the largest unit whose acceptance criteria can be written as executable
  commands. Size is that property, never a scope judgement.
- **Edges are typed by why they exist**: `buildability` (a code fact, binds), `safety` (an
  irreversible act, binds), `risk-ordering` (a choice, overridable with `--override-risk-ordering`).
- The driver composes each package onto its dependencies' completed work, dispatches the whole ready
  antichain concurrently as prime-agent workers inside herdr worktrees, executes each package's
  criteria as real shell commands in a prepared detached clone, sends a gate to attack the built
  artifact, replays each gate finding's witness/repair commit pair before crediting it, and finally
  assembles every package and re-runs all criteria against the assembly.
- Recovery is a bounded ladder: retry, local patch carrying the failure evidence inline, then park.
- Nothing merges or opens pull requests. Promotion is a human act.

Skills: `/to-graph <vision-dir>` writes the vision, authors the graph, renders it, discusses it with
the human, and freezes it. `/land-ticket <n>` proves delivery from the driver's journal and asks the
human exactly one question. `/chart-program` surveys the Program. The old `/pce` orchestrator skill
is superseded.

## Where things stand

`main` is at `a15dca5`, built and installed. Verify with `git log --oneline -1` and
`~/.local/bin/pce --help`.

Seven defects were found by the first real run and all are fixed and merged: retry collided on
worktree and branch names; a failed spawn deadlocked the next start; dependents did not inherit their
dependencies; nothing integrated or gated the composed whole; one malformed gate finding aborted the
entire run; herdr panes accumulated; and a persistent environment failure looped forever.

Program #37 on `CooperBigFoot/pce` has two open tickets, #46 and #135. Everything else was closed in
an evidence audit: most tickets rested on measurements taken under the retired architecture, and this
Program's own scope rule is that nothing gets a ticket without a finding behind it.

## Experiment one, finished

RivRetrieve. Eight packages ran against real code; seven completed. It produced every defect above.
The work was integrated by hand, merged as PR #157, and ticket #12 closed with its evidence.

**Three findings are waiting to become tickets and should not be written until experiment two is
done**, because we want them written from a working workflow rather than a broken one:

1. `/land-ticket` proves delivery only from the driver's journal, so a ticket delivered by a driver
   that crashed has no path to landing — even when delivery is genuinely proved by a merged pull
   request and an executed test suite. Not a human-assertion escape hatch; the question is whether
   repository execution evidence is an alternative proof.
2. Agents running end-to-end drives inside their own test fixtures open real herdr panes and do not
   close them. The tool cleans up dispatch panes now; test fixtures are outside that.
3. An exhaustive match on package state has made the renderer a shared surface that three separate
   packages had to touch despite being told not to.

## Experiment two, yours

taqsim: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core`

It is a better test than the first for three reasons.

**It spans two repositories**, which has never been exercised: `/Users/nicolaslazaro/Desktop/work/taqsim`
(Python, uv, `uv run pytest`) and `/Users/nicolaslazaro/Desktop/work/incidence` (Rust, cargo,
`cargo test --workspace`). Both carry a tracked contract at `.pce/repository-contract.json`.

**The old run is genuinely stuck** — it blocked at milestone 3 with two plan loops past their round
caps and a human-authorised recovery that could not be enacted. The old workflow cannot finish it.

**Its status output lies.** `pce status` reports `merge-status=inconclusive` for all 18 steps because
taqsim's integration branches were deleted from its remote. `incidence` genuinely has merged work
through milestone 2 and still holds its `pce/incidence-core/m3-s*` branches; taqsim has no `pce`
branches at all. Ground truth has to come from git and GitHub directly.

The sequence is: `/to-graph` on that directory in a session opened in the taqsim repo, argue with the
rendered graph until it is right, freeze, then `pce package driver-run` with two `--repository` flags
and two `--prepare` commands. A ready-to-paste prompt for that session is at
`briefs/taqsim-to-graph-prompt.md`.

Your job in this session is to oversee that: verify state, review what the graph agent proposes,
watch the run, diagnose what breaks, and write briefs for fixes. Do not do the conversion here.

## How we work

- **Verify claims rather than relaying them.** Agents report success in good faith and are sometimes
  wrong. Read the journal, run the test, check the commit. Several real defects this week were found
  only because a reported result was checked.
- **Fixes go to prime-agent via a brief**, written to `briefs/`. Briefs carry all context — a
  dispatched agent cannot ask questions, so name every path, version, and constraint. Look at
  `briefs/wp11-brief.md` through `briefs/wp14-brief.md` for the shape.
- **Merging is done in a separate integration worktree**, never in the main checkout: a `git merge`
  there rewrites `skills/pce/`, which live runs read through a symlink. The worktree is
  `/Users/nicolaslazaro/Desktop/work/pce-integration` on `integration/work-package-harness`. Merge
  there, run the full suite, then fast-forward `main` and `cargo build --release`.
- Six other PCE runs share this machine and this binary. A rebuild swaps it under them.
- Lead with the plain-language point; put mechanism underneath or on request. Never ask what the
  human is afraid of — name the concrete failure yourself and let them react.

## Start by

Confirming `main`, the installed binary, the open tickets on #37, and whether taqsim has a graph yet.
Then tell me what you think the first move is.
