# PCE workflow feedback: engine stage contracts (completion)

- Date: `2026-08-01`
- Orchestrator: Claude Code (Opus 5), same session, continued
- Run: `RivRetrieve`, `planning/2026-07-28-engine-stage-contracts`
- Outcome: `delivered` — all 7 milestones merged to `main`; vision complete
- Companion to: `2026-07-31-engine-stage-contracts.md`, which reported the same run
  while paused at 5 of 7 milestones. That report stands; this one covers milestone 7
  and the findings only completion could produce.

## Executive summary

Milestone 7 merged to `main` at `a134b06`. Final log: 343 records — 197 `dispatch`,
98 `key-finding`, 35 `planning-artifact-approved`, 4 `escalation-open` and 4
`escalation-close` (all closed), 3 `delta`, 2 `repository-contract`. Milestone 7
alone consumed 65 dispatches: 17 plan writers, 20 plan critics, 12 executors,
7 PR reviewers, 5 graph critics, 4 graph planners.

**The most important finding is that no gate in this workflow asks whether a
decomposition serves the human's actual working method.** The milestone planner
inferred, correctly against the vision's acceptance criteria, that eleven unported
providers' legacy pipelines should be *deleted*. Two adversarial graph critics
approved that graph. Four step plans were written, critiqued, approved and executed
against it. Nothing was wrong by the process's own standards.

It was still the wrong build. The human's method for porting the remaining providers
is to *read the current implementation and write the three engine files* — and
deletion removes the input to step one of that method. An agent opening
`providers/ch_foen/` after the merge would find only catalogue files and would not
spontaneously reconstruct `retrieval.py` from git history. The knowledge stayed
*recoverable* but stopped being *discoverable*.

This surfaced only because the human asked, in passing, what the ~6,000-line
deletions were. Three step commits were then discarded, the graph re-planned to
relocate rather than delete, and the milestone re-executed. Critics test whether an
edge survives contact with source. Nothing tests whether the decomposition survives
contact with intent.

**The second finding is that an enforcement step needs a gate class the workflow does
not have.** `m7-s5` installs the lint rule that vision criterion 3 names. Its round-1
plan passed all eighteen of its own gates *with the entire lint half inert*: the gate
ran `ruff check --select TID251`, and `--select` on the command line **replaces**
configured selection, so the rule fired whether or not `extend-select = ["TID251"]`
existed in `pyproject.toml`. Deleting that line from the fully-applied plan produced
`All checks passed!`. A conventional plan critique would have approved it. The defect
surfaced only because the critique prompt explicitly required *planting violations and
deleting the configuration*.

**The third is a recurrence that a written warning did not prevent.** The
formatter-spillover class — a plan asserting a `ruff format --check` count that its own
prescribed content falsifies — appeared **three times** in this milestone, despite the
shared plan-writer contract carrying an explicit "MEASURE the list, do not assume it"
section from earlier in the run. Writer discipline is not sufficient for this class.

## Evidence reviewed

- `planning/2026-07-28-engine-stage-contracts/events.jsonl` — 343 records at completion.
- Milestone 7 artifacts: `steps.json` (two approved versions), five `plan.md`, and
  nineteen review documents across `step-1` through `step-5`.
- The three discarded delete-variant commits, preserved at `refs/superseded/m7-s{2,3,4}-delete-variant`.
- Merge history on `main`: `1d0388c`, `9b76bea`, `9155798`, `e5465b0`, `709892c`, merged
  by `a134b06`.
- Orchestrator-run measurements: gate re-runs outside the sandbox on every step commit,
  plus four independently planted enforcement violations against `m7-s5`'s delivery.

## What worked

**Lesson transfer between sibling steps, measurably.** `m7-s2` took three plan rounds.
Before dispatching `m7-s3` and `m7-s4`, I injected `m7-s2`'s paid-for lessons into both
writer prompts — the bare-`python` gate defect, the shapes-vs-substance coverage trap,
the global-aggregate subject mismatch, country-named fixtures, mixed test files, and
generator tests hiding inside `test_*_observations.py`. `m7-s3` then **approved at round
1**, and the test-accounting defect class that cost `m7-s2` two rounds did not recur in
either sibling. This is the single cheapest quality lever observed in the run.

**Mutation testing as the primary PR gate, again.** Across milestone 7's PR reviews:
17/17 killed (`m7-s2`), 18 planted with one verified-equivalent survivor (`m7-s3`),
16 with one equivalent (`m7-s4`), 25 across seven criteria (`m7-s5`). Two of `m7-s2`'s
mutants settled a question I had raised and could not settle by reading: removing the
Ruff exclusion moved lint from 1 to 18 errors, and removing the ty exclude moved
whole-project ty from 122 to 176. Both exclusions were proven load-bearing rather than
assumed.

**Honest `BLOCK` behaviour, repeatedly, under pressure to fabricate.** Executors
returned `BLOCK` rather than inventing gate results on four occasions, and every one was
a real defect or a real environment failure. The `m7-s4` executor blocked twice on `uv`
failures with zero tests collected; the `m7-s2` executor blocked on a plan prescribing a
bare `python` that exits 127. Prompts that say "an honest BLOCK is a good outcome; a
fabricated pass is the worst possible one" appear to work.

**Digest pinning with open-and-close readings.** After an author-vs-gate race (below), I
pinned the expected digest in every subsequent critique prompt and required the critic to
re-read it at the end and report both. Every later critique reported identical readings.
Cheap, and it converts a silent corruption into a loud stop.

**Graph-critic findings surviving three stages.** The `pl_imgw` `ImgwCacheClient`
straddling hazard was found by the *graph* critic, carried into the `m7-s4` plan-writer
prompt as an explicit directive, and verified in the delivered commit by the PR
reviewer's call-graph trace. Without the first catch, a naive move would have produced a
reference copy whose `query` calls absent methods — readable-looking and useless.

## Friction and failures

### F1 — No gate tests the decomposition against the human's working method (highest impact)

Described in the summary. Mechanically: `milestone-critic` and `step-critic` are both
specified to attack `depends_on` edges — "try to refute every edge", "the burden of proof
is on the presence of an edge". That is edge-level review. Neither role is asked whether
the *set* of nodes builds the thing the human will actually use, or whether a node's
chosen mechanism (delete vs relocate) serves downstream work the vision does not describe.

The vision listed "porting all thirteen providers" as an explicit non-goal, so the
follow-on work was known to exist and known to be out of scope — and the graph still
chose a mechanism that would have made that follow-on materially harder. Nothing in the
process was positioned to notice.

Cost: three executed step commits discarded, one graph re-plan, four plan re-writes,
roughly a third of milestone 7's dispatch budget.

### F2 — Enforcement steps have no "prove the rule bites" gate class

`m7-s5` round 1. Eighteen gates green with the lint half inert. Also in the same review:
the structural check matched `.clip(` as its window-clipping pattern, and `.clip(`
occurs **zero times in this codebase, ever** — the real idiom is a polars boolean filter
present verbatim in the archived `za_dws` retrieval. And the operation check missed
`dt.convert_time_zone`/`dt.replace_time_zone`, which sit verbatim in two archived
transforms and are the idioms a polars-native port will actually produce.

All three are the same shape: a rule that is *present* and *passing* and *not
protecting anything*. A plan critique that reads the rule and checks the gate output
cannot find them. Only planting violations can.

### F3 — Formatter spillover recurred three times against a standing written warning

`m7-s2` round 1, `m7-s4` round 1, and latent in `m7-s5`. In each case the plan asserted
an exact `ruff format --check` count that its own newly-authored content falsified —
a 140-character line in a test literal, a docstring instruction that left three
`module.py` files formatter-positive, a parenthesised assignment collapsing to exactly
the 120-char limit.

The shared plan-writer contract already carried an explicit section telling writers to
materialize their content in a scratch copy and measure. It was not enough. Notably the
*reliable repair* is always the same and is not "fix the number": it is to make the
prescribed content format-clean, after which the count returns to baseline on its own.

### F4 — Author-vs-gate race, second distinct mechanism

Earlier in this run a liveness check failed because `pgrep` matched the prompt
*filename* while the process command line held the expanded prompt *text*. This time I
saw the artifact written, noted in the same breath that the writer process was still
alive at 4m30s, and dispatched the critic anyway. The file passed through three digests.

Detection was **incidental**: `pce status` reported a `current_sha256` in its
`digest-mismatch` condition that did not match the digest I had computed minutes
earlier. That is luck, not a control.

### F5 — `pce ready` changed its CLI surface mid-run, in the opposite direction from last time

Earlier in this run, a stale binary *rejected* `--policy` and the human force-rebuilt to
restore it. Later, a newer binary had *removed* `--policy` entirely: passing it printed
`USAGE` and exited non-zero. Re-invoking without it succeeded.

Its output was also hard to interpret: with five m7 nodes it returned exactly one result,
`{"classification":"waiting","node":"m7-s5"}`. The other four were absent rather than
reported ready or waiting, and I could not determine from the output whether absence
meant already-dispatched, already-branched, or something else. I recorded that I was
explicitly *not* resting the merge decision on it.

### F6 — `merge_status` goes `inconclusive` on PR-selector collisions

`m7-s2` and `m7-s3` both project `inconclusive` on the final snapshot, with
`cardinality: multiple-exact-matches`. Cause: each had an earlier delete-variant PR that
was **closed** when the milestone was redirected, and the later relocate-variant PR
reused the same head and base branch names. The projection sees two exact matches and
declines to choose. Both are provably merged — `9b76bea` and `9155798` are ancestors of
`main` — but a reader of `pce status` alone would not know that.

## Recommendations

**R1 — Add an intent check to graph critique.** Give `milestone-critic` and
`step-critic` one obligation beyond edge refutation: *state how a human or agent will
consume this milestone's output, and identify any node whose chosen mechanism makes
known follow-on work harder.* The vision's `Scope — Out` section already enumerates the
follow-on work; make critics read it as context for the decomposition, not just as a
scope fence. This run's central failure would have been caught by one question:
"after this merges, how does someone port provider N+1?"

**R2 — Add a `rule-enforcement` gate class.** When a step installs a lint rule,
structural check, or CI assertion, require the critique to prove it bites *in both
directions*: plant each violation the rule claims to prevent and confirm failure; plant
the rule's own removal (delete the config line) and confirm a test fails; and confirm
legitimate existing code stays clean. `m7-s5` needed all three, and the second one —
*can the rule detect its own removal* — is the one nobody thinks to ask.

**R3 — Make plan materialization mechanical rather than discretionary.** F3 recurred
three times against explicit written instruction. Consider a workflow step, or a `pce`
subcommand, that takes a `plan.md`, extracts its prescribed content into a scratch tree,
runs the repository contract's format/lint/typecheck commands, and emits the measured
counts for the plan to quote. Writers should not be trusted to remember; and every
critic that caught this did so by doing exactly that work by hand.

**R4 — Make process exit, not artifact presence, the documented dispatch gate.** State
plainly in `SKILL.md` that a Codex-authored artifact may not be gated until its process
has exited, and that liveness must be checked by PID rather than by pattern-matching a
command line. Optionally have `pce` offer a `digest` helper so pinning is one call.

**R5 — Version-pin or capability-probe the binary.** A live run should not have its
readiness authority change surface underneath it, twice, in opposite directions.
Either `pce` reports a contract version the skill can assert, or `SKILL.md` instructs
the orchestrator to probe `pce ready --help` once at startup and adapt.

**R6 — Report `ready` classifications for every node, including "not applicable".**
Absence is ambiguous. If a node is omitted because it is already dispatched or already
branched, say so.

**R7 — Disambiguate `merge_status` on multiple PR matches.** Prefer the merged PR when
exactly one match is merged and the others are closed; or report each match with its
state so the orchestrator can decide. `inconclusive` on a provably-merged step is a
false alarm that a reader must resolve by hand with `git merge-base`.

**R8 — Formalize a per-run lessons ledger.** The single highest-leverage thing I did in
milestone 7 was hand-carry `m7-s2`'s defects into `m7-s3`/`m7-s4`'s writer prompts, and
`m7-s3` approved at round 1 as a result. Make this structural: a run-scoped list of
"defects already paid for", appended on every REVISE verdict, and spliced automatically
into every subsequent plan-writer prompt in the same run.

## No-change decisions

- **Round caps of 3 remain right.** `m7-s2` needed all three and the third round closed
  the defect class exhaustively (167/167 tests accounted, zero silent omissions). A
  lower cap would have escalated a plan that was one round from correct.
- **`root_cause: step_plan` routing remains right.** Every REVISE in milestone 7 routed
  to the plan writer, not the executor, and in every case the plan really was the defect.
- **Squash steps, merge-commit milestones remains right.** The rebase of `m7-s4` onto a
  sibling that merged first applied with zero conflicts, confirming the graph's
  concurrency judgement empirically.

## Suggested follow-up

- The four ADRs this vision names as **binding** — `0007`, `0008`, `0009`, `0010` — are
  untracked in the target repository and appear in no commit. Execution was never
  blocked, because plan writers quoted their text verbatim under `Quoted authorities`,
  which is exactly what the zero-context-executor contract demands. But `main` now
  *mechanically enforces* an architecture whose rationale is not in the repository.
  Consider whether `SKILL.md` should require Phase 0 to verify that every ADR the vision
  declares binding is actually tracked, and escalate if not. This is a vision-authoring
  gap rather than a workflow bug, but the workflow is well placed to catch it.
- `planning/` is likewise untracked, so the entire event log, every plan and every
  review live only in the working tree. The skill requires `LOG_PATH` to sit under the
  primary repository root but never says it should be committed. Worth an explicit
  statement either way, since the log is the run's only durable record.
