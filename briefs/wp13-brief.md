# WP13 — one bad finding must not kill the run, and panes must not accumulate

A gate named a witness commit it had orphaned. The validation caught it, correctly — and then killed
a graph that was seven packages of eight complete, on real code, unattended. Everything that gate had
proved was lost with it.

Separately, every dispatch leaves a herdr pane behind and nothing removes it.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `main` at commit `a058b2f` in a **new worktree**.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in `/Users/nicolaslazaro/Desktop/work/pce` replaces the binary those runs
invoke, and a `git checkout` there rewrites the skills they read.

1. Work in your own git worktree, never the main checkout. Keep your own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.
4. **Do not touch `/Users/nicolaslazaro/Desktop/work/RivRetrieve` or `/tmp/pce-work-package-worktrees`.**
   They hold the evidence from the first real run. Build your own fixtures elsewhere; WP11 added
   `PCE_WORK_PACKAGE_WORKTREE_ROOT` for that.
5. **Panes you create while testing are the operator's live terminal.** Close what you open. Never
   close a pane you did not create — the operator and six other runs are working in this herdr
   session right now.
6. A sibling package is concurrently making dependent packages build on their dependencies, touching
   `herdr_dispatch.rs`, the driver and `src/main.rs`. Keep your changes where they belong and expect
   to rebase.

## The tools, stated rather than assumed

**herdr** — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; the checkout is
0.8.0. The installed binary is the authority.** Relevant verbs, from `herdr pane`:

```
herdr pane list [--workspace <workspace_id>]
herdr pane get <pane_id>
herdr pane close <pane_id>
```

`herdr agent start` on 0.7.1 creates a pane — that behaviour was measured, and it contradicts the
0.8.0 skill document, which is why the installed binary is the authority. herdr does not recognise
prime-agent, so a worker shows as `unknown`. **Never consult herdr for completion.**

**prime-agent** — installed 0.7.2, `prime-agent -p`, reads piped stdin.

## What already exists

- `crates/core/src/package_gate.rs` — the findings format and `validate_package_gate_repositories`.
- `src/main.rs` — `resolve_package_gate_commit` and `validate_package_gate_refs`, which resolve each
  ref, require reachability from a named ref or HEAD, require the witness to be the repair's sole
  direct parent, and confirm ancestry. This is the code that aborts.
- `crates/core/src/package_driver.rs` — `judge_finding_replay`, which already models a rejected
  finding with a typed reason: `witness-passed`, `repair-failed`.
- WP11 added worktree removal on completion, retention on failure or park. Panes are the missing
  half of that same idea.

---

## Part one: a malformed finding is rejected, not fatal

### What happened

RR5's gate returned six findings. Four were accepted. One was correctly rejected as `repair-failed`.
One named `8a94aea48a29e8a4ece4f9082d9d36b12eeb43e0`, a commit that exists in the object database and
is reachable from nothing — the gate had reset past its own witness and reported it anyway.

Validation raised an error, the error propagated, and the driver exited:

```
Error: finding refs do not satisfy the WP6 witness/repair contract
Caused by: package gate ref `8a94aea…` resolves but is not reachable from any ref or HEAD
```

Seven completed packages, and the run died on one agent's bookkeeping mistake. WP6's brief said a
malformed finding *is rejected as malformed*. The implementation treats malformed as fatal.

### What must be true

**A finding that fails validation is rejected, with a typed reason, and the run continues.** The
driver already has the vocabulary for this: a replay judgement is `accepted` or
`rejected { reason }`. A structurally malformed finding is a third rejection reason alongside
`witness-passed` and `repair-failed`, not an exception.

**Rejection is visible.** A rejected finding is evidence about that gate, and it must be readable in
the journal and on the rendered page. A run in which a gate produced four good findings and one
unusable one should say exactly that.

**The gate's other findings still count.** Four accepted findings were lost with the crash. They must
survive one sibling being malformed.

**Blast radius is one finding, then one package.** If a gate produces nothing usable at all, that is
a fact about the package, not about the graph. Decide what a package whose gate produced no usable
output means — completed, failed, or retried — and say why. It must not be silently treated as a gate
that found nothing, because *attacked and found nothing* and *could not be understood* are different
facts and this project has a long record of what conflating them costs.

**The validation itself stays exactly as strict.** Every check WP6 built — resolvability,
reachability, sole-direct-parent, ancestry, repository in scope — keeps its current strictness. The
change is what happens on failure, never whether it fails.

Nothing anywhere may treat an unusable finding as usable in order to keep going.

## Part two: panes are cleaned up by whoever made them

Every worker and every gate dispatch leaves a herdr pane. Nothing closes them, so a run of eight
packages leaves at least sixteen behind, and a retry adds more.

**Symmetry with WP11's worktree rule is the design.** WP11 removes a clean worktree whose package
completed and retains one belonging to a failed or parked package, because the failed one is
evidence a human may want to read. Panes follow the same rule for the same reason: close the pane of
a dispatch whose package completed, keep the pane of a failed or parked one.

Three constraints, and the first is the important one:

**Only close a pane this run created.** The operator works in this herdr session and six other runs
share it. Derive the target from the pane the dispatch itself started — the agent name is already
derived per attempt, and `herdr pane list` reports enough to identify it. Never close by position,
by pattern, or by anything that could match a pane the run did not open. A cleanup that can close the
operator's own terminal is worse than no cleanup.

**Failing to close a pane is not a failure of the package.** herdr being unavailable, a pane already
gone, a close that errors — record it and carry on. Nothing about the work's correctness depends on
tidiness, and this must never become a way for a completed package to be reported as failed.

**Completion is still never read from herdr.** This package touches herdr only to close what it
opened. The sentinel test proving completion is derived from the filesystem stays untouched, and
nothing here may consult pane state to decide whether work finished.

---

## Acceptance criteria

1. A finding whose refs fail any structural check is rejected with a typed reason and the driver
   continues. Cover, at minimum, an unresolvable ref and a ref that resolves but is unreachable —
   the case measured in production.
2. A gate returning four valid findings and one malformed one credits the four and rejects the one.
3. Every existing rejection reason keeps working, and every WP6 validation keeps its current
   strictness. The tests that prove each check still fail on a bad input all pass unchanged.
4. A rejected finding is visible in the journal and on the rendered page, distinguishable from an
   accepted one and from a gate that found nothing.
5. A package whose gate produced no usable finding at all reaches the outcome you chose, and the
   journal states which case it was. Explain the choice in your report.
6. A dispatch whose package completed has its pane closed; a failed or parked one keeps its pane.
7. Pane closing is scoped to panes this run created. Make that checkable rather than asserted — a
   test in the shape of WP4's herdr sentinel, proving no unrelated pane can be selected, is worth
   more here than an assertion in a comment.
8. A pane that cannot be closed is recorded and does not affect the package's outcome.
9. No code path consults herdr to decide completion. The existing sentinel test stays green.
10. Every existing test passes unchanged — readiness waves, herdr sentinel, RR2 briefs, `TMPDIR`
    headroom, the five gate tests, driver tests including the antichain barrier, recovery tests
    including the one proving that removing the ladder changes dispatch, WP11's retry-identity and
    spawn-failure tests, render tests, landing-skill doctrine tests.

**Plus one real end-to-end drive, executed and reported.** A two-package graph in a repository you
create, driven by real prime-agent 0.7.2 through real herdr 0.7.1, in which a gate emits one
malformed finding alongside a valid one — seed it deliberately rather than hoping — and the run
completes anyway. Report `herdr pane list` before and after, showing the panes you created and their
removal.

Then render it with `pce package render` and report the output path.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test in `tests/dispatch.rs` is known to fail intermittently and pass on an
isolated rerun.

## Do not touch

- The strictness of any WP6 validation. Only the consequence of failure changes.
- Composing dependents onto their dependencies, and the end-of-graph assembly — a sibling package
  owns both, concurrently.
- The graph schema, brief composition, criteria execution, coordinated replay, amendments, the
  recovery ladder's rungs and budgets, the wait mechanism.
- WP11's worktree removal and retention rules. Panes mirror them; they do not replace them.
- The live RivRetrieve run's evidence.
- The two open `pce:ticket` issues, #46 and #135.

## Report back with

- The new rejection reason and where it is recorded.
- What a package whose gate produced nothing usable becomes, and why you chose that.
- How a pane is identified as one this run created, and the test that proves an unrelated pane
  cannot be selected.
- What happens when a pane cannot be closed.
- The real drive, including the seeded malformed finding and the pane listing before and after.
- Anything you found that contradicts what this brief assumes.
