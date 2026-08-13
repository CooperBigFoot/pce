# Task: repair the PCE orchestration tool

You are fixing a Rust CLI + workflow doctrine called **PCE**, which orchestrates fleets of
coding agents. It works, but it is slow and it dead-ends. Six repositories currently depend
on it and four of their runs are stuck right now.

Decide your own method. Nothing below tells you how to work, in what order, or how to use
your own harness — that is deliberate. What follows is only: what must be true when you are
done, what you must not touch, and what will destroy live work if you get it wrong.

---

## Where the code is

Main checkout: `/Users/nicolaslazaro/Desktop/work/pce` — **do not work here.** See Boundaries.

Orient yourself from `AGENTS.md`, `skills/pce/SKILL.md` (the workflow doctrine, ~540 lines),
`crates/core/src/run_state.rs` (state derivation and dispatch admission), `src/main.rs` (CLI),
and `tests/dispatch.rs`. Evidence for every defect below lives in `orchestrator-feedback/` —
real reports from real runs. Read the ones you need.

---

## Boundaries — violating these breaks live work

Six PCE runs are executing right now against this tool. The installed surface is symlinked
directly into this repository:

```
~/.local/bin/pce        -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce    -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in the main checkout **immediately replaces the binary those six
runs invoke.** An edit to `skills/pce/SKILL.md` in the main checkout is live at the next skill
load. This has already caused a silent cross-run failure once.

Therefore:

1. Work in a **git worktree**, not the main checkout.
2. Export `CARGO_TARGET_DIR` to a path of your own. Never let a build populate the main
   checkout's `target/`.
3. Never run `install.sh`.
4. Never modify any file under the main checkout path.
5. Do not push, merge, tag, or open PRs. Leave your work on a branch.

**Before anything else:** the main checkout has an uncommitted patch in
`crates/core/src/run_state.rs` adding a `PCE_DEFECT_ROUND_CAP` environment variable. It must
not ship — it makes an identical event log replay differently depending on the environment.
Ensure it is gone from your branch, and add a test proving no environment variable can change
dispatch admission.

---

## Success criteria

These are the acceptance tests. All must pass.

**1. Every existing event log still replays.** Seventeen logs, ~5,000 records, across six
repositories at `/Users/nicolaslazaro/Desktop/work/{RivRetrieve,palaestra,hfx,taqsim,bluesmith,pourpoint}/planning/*/events.jsonl`.
These are append-only histories of real runs. New code must read every one of them without
error. Records written by the old code stay meaningful.

**2. Four stuck nodes become dispatchable.**

| Log | Nodes |
|---|---|
| `RivRetrieve/planning/2026-08-11-the-store-is-the-only-copy/events.jsonl` | `m1-s2` |
| `taqsim/planning/2026-08-10-incidence-core/events.jsonl` | `m3-s4`, `m3-s9`, `m3-s11` |

Each hit a per-`(node, role)` cap of 3 and the binary now refuses the exact dispatch forever.
A human authorised one more attempt on `m1-s2`; it was recorded as an `escalation-close` event
and refused anyway, byte-identically, because admission never reads escalations.

**3. `m3-s11` routes to a code repair, not a plan rewrite.** Its PR review returned findings
whose `root_cause` is `step_plan`. `SKILL.md` contains two contradictory rules for this: stage
6 says dispatch a code fix at the PR head; the "Routing, caps, and adaptation" section says
re-dispatch the plan writer. The orchestrator picked the second and dead-ended. Stage 6 wins.

**4. The repository's own gates pass** — whatever `.pce/repository-contract.json` states.

---

## What must be true when you are done

### Nothing may permanently refuse

Today, cap exhaustion is a wall with no door. A step that cannot converge must **park**:
stop, stay resumable, and let the rest of the run continue around it. It must never become
permanently undispatchable. A human's recorded decision must be able to unblock it.

### The cap is a spending limit, and should say so

It is called a "defect-round cap" and it counts something else — every validated production,
including the initial implementation. So a cap of 3 is one build plus two repairs. Rename it
to what it measures, and set it generously, because parking is now the real stop rather than
the number.

Related and clearly wrong: a dispatch whose structured `root_cause` names an *upstream*
artifact currently charges the role that reported it. In one run the orchestrator invalidated
a plan by rebasing a worktree; the executor correctly refused to work around it, returned
`BLOCK` with `root_cause=step_plan` — the exact behaviour the doctrine demands — and was
charged a round for noticing. Upstream causes must not consume the reporting role's budget.

### The two sandboxes must agree on a writable temp directory

`pce contract check` derives its sandbox temp root from the caller's `TMPDIR`. A dispatched
child gets no `TMPDIR` at all and can only write `/tmp`. They permit disjoint roots, so any
tool with an out-of-repo cache (`uv`, `pip`, cargo) satisfies exactly one of them.

The route contract mandates exactly three forwarded environment entries and treats that
cardinality as falsifiable from both sides. Keep that. **Have the binary synthesize `TMPDIR`
itself**, to a root it guarantees both sandboxes permit — binary-owned, not forwarded from
the operator's shell. Evidence: `orchestrator-feedback/2026-08-11-the-mandated-three-env-entries-strand-tmpdir.md`.

### Thirteen mechanical defects, each with evidence in `orchestrator-feedback/`

1. `pce contract refresh` rewrites the tracked contract unconditionally, leaving it dirty with
   a whitespace-only change — while the doctrine forbids a run from editing that file. Write
   only on semantic change.
2. `pce contract bootstrap` leaves the contract untracked, so a fresh worktree has none and
   its contract check fails. Commit it.
3. `pce vision check` hides a stdin-only input contract behind a structural parse error.
4. `pce status` inherits force-color variables into `gh --json`, producing invalid JSON.
5. `pce dispatch check-in --file` demands an absolute path; `pce log`, `pce status` and
   `pce ready` all accept relative. Make it consistent.
6. `pce dispatch gate` exits 0 on an artifact it judged schema-violating. Exit status should
   reflect the outcome.
7. `pce dispatch -o` writes the structured result into the worktree it is measuring, dirtying
   the tree the executor is required to keep clean.
8. PR reviews and pre-PR falsification reviews collide on one artifact index inside a single
   step directory; following the rule literally overwrites an approved review.
9. `pce status` probes a hard-coded, unrelated tag in every target repository. Derive it from
   the repository contract.
10. Installed JSON schemas are not validated against the strict structured-output dialect
    Codex requires. One became invalid mid-run — two objects declared `required` arrays
    omitting a key present in `properties` — and every executor dispatch died with
    `400 invalid_json_schema` while Claude gates kept passing, because a gate returning an
    empty findings array never exercises the item schema. Validate at startup.
11. `docs/codex-non-interactive.md` has been wrong for 26 days: it instructs a `tasks.json`
    state file that ADR 0002 abolished, bounds gate loops by a round count the convergence
    signal replaced, and treats a step as runnable only once its `blocked_by` steps merged,
    which the dispatchable set replaced.
12. `SKILL.md` never states that milestone integration branches must be pushed when created
    and retained while any run can still use them as merge authority. Deleting one after its
    milestone merges flips every step to `inconclusive` permanently. This cost six unlandable
    steps and five manually-restored branches on one run.
13. Mutation testing should be expressible as a declared gate in the repository contract, with
    each repository naming its own command — `cargo-mutants`, `mutmut`, Stryker. Not every
    PCE repository is Rust. Motivation: reviewers are currently hand-simulating a mutation
    tester and finding one surviving mutant per round, which cost three duplicate nodes and
    five exhausted caps in a single milestone.

---

## Do not touch

**Fourteen open GitHub issues labelled `pce:ticket` on `CooperBigFoot/pce`** (#40, #42, #43,
#44, #45, #46, #47, #105, #133, #134, #135, #165, #167, #183). These are open *questions*
awaiting a human design conversation, not work items. Do not answer them, design against
them, or edit them. Several of the defects above sit inside their territory; fix the defect,
leave the question alone.

**The knowledge-interface question.** PCE has seven overlapping knowledge stores —
`CONTEXT.md`, `docs/adr/`, both halves of `.pce/repository-contract.json`, event-log
key-findings, the program map, and `SKILL.md`. Unifying them is an open design question.
Do not attempt it, and do not add a new knowledge store.

**Your own harness doctrine.** Do not run `refine` against yourself, and do not persist
global harness entries. Improvise your workflow freely for this task; do not make it permanent.

---

## What I care about, in one line

PCE is a workflow that lets one person direct a fleet of agents that outclass them
technically. It is currently too slow, spends a third of its budget arguing about plan
documents before any code exists, and stops dead in states no human can unstick. Make it
produce, and make it impossible for it to strand itself.
