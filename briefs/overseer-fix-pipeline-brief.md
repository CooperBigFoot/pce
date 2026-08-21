# Brief: the overseer cannot repair the binary it runs under

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-21 against `main` at `862f1c8`.

## The defect

The overseer's duties include dispatching a brief when a stop is a defect in the binary rather than a
decision, and installing the result when the fleet is quiet. Neither operation exists. The counters
exist and always read zero:

```
- Defect briefs dispatched: 0
- Pending installs: 0
- Blocker: installed `pce` lacks overseer defect-brief and repair-dispatch operations.
```

`pce --help` confirms it: `hold open|sift|feedback|feedback-list|list|register|runs|read|answer|route|close`
and `overseer view|heartbeat|serve|pass-complete`. Nothing dispatches, builds, installs, or announces
an update.

It has already cost a real finding. Asked to inspect the pce source, the overseer correctly diagnosed
a defect — `amendment_counterfactual` treats any successful `git revert --no-commit <repair_ref>` as
proof that a repair mattered, without verifying that the revert changed the tree, so a repair already
superseded by later work reverts to nothing, succeeds, and is accepted as proof. It could not
dispatch a brief for it. The finding sits in a hold thread the operator could not read and no run can
act on.

## What must become true

The pipeline the operator ran eight times by hand in one day: verified defect, brief, isolated
worktree, implementation, full suite, merge, build, install, and every live run told the binary
moved. The operator's stated boundary is that after a graph is frozen he is reachable for money and
for changing what done means. A defect in the binary is neither.

Shape is your decision. These constraints are ratified and hold:

- **The overseer never signs an attributed record.** Criterion revision, worker-environment extension,
  base-currency acceptance, park overrule, publication and spend keep requiring the human's name. A
  repair is none of those.
- **A brief is dispatched only against verified evidence.** This Program's scope rule is that nothing
  gets work without a finding behind it. A theory is not a finding; the overseer must be able to name
  the code fact.
- **Work happens in an isolated worktree with its own `CARGO_TARGET_DIR`**, never in the main
  checkout, while live runs continue against a stable installed binary. This is the ratified ordering
  constraint: PCE is maintained from outside PCE.
- **An install is requested once and performed at the next moment no live run is mid-dispatch**, never
  timed by the human. Safety is derivable — no live run mid-dispatch, checked against the diff's
  touched paths.
- **On this machine, building is installing.** `~/.local/bin/pce` is a symlink to
  `target/release/pce`, so a release build in the main checkout mutates every live run's floor
  instantly. An unmerged worktree build once populated the shared binary mid-run and a run on another
  repository failed against code that existed on no branch it could observe. Whatever you build must
  not be able to reach the installed path before its install moment.
- **In-flight work is recorded in the store**, so a respawned overseer cannot dispatch a second worker
  into the same files.
- **Every live run is told the binary moved**, with the digest, after the install.
- **A failed step is a recorded fact, not a silence.** Do not add another indicator wired to a claim.

## The update indicator

The operator asked for it explicitly: something that says an update is ready, which he accepts once,
and which then installs itself at a safe moment rather than asking him to pick one.

- The queue shows that a repair is built and waiting, with what it fixes in one line.
- The operator accepts once. He never times it, and he is never asked to check whether the fleet is
  quiet — the overseer already knows.
- After the install, the queue records that it happened and against which digest.
- If the fleet never goes quiet, that is visible rather than silent. A repair that waits forever
  because one run is permanently mid-dispatch must surface, not disappear.

## Out of scope

Waking a run when its hold is answered is `briefs/answered-hold-reaches-nobody-brief.md`. The
readability and conversation defects are `briefs/overseer-conversation-brief.md`. Do not build either
here.

The `amendment_counterfactual` defect named above needs its own brief with its own falsifier — a
witness commit where the check passes wrongly and a repair commit where it fails. Producing that
brief is exactly what this pipeline is for; do not fix that defect inside this work.

## Operating constraints for this dispatch

Read this before touching anything.

**Work in your own git worktree with your own `CARGO_TARGET_DIR`.** Never build in the main checkout
at `/Users/nicolaslazaro/Desktop/work/pce`. On this machine `~/.local/bin/pce` is a symlink to
`target/release/pce`, so a release build there replaces the operator's live binary and every parallel
agent's test binary while they are running. This has already caused one recorded incident.

**You are not alone.** Another agent is working from a different brief in a parallel worktree.
Per-brief file ownership is stated below. Keep edits to shared files narrow and in one region so the
merge stays mechanical, and never reformat or reorganise code you are not changing.

**The live hold store is not yours.** The overseer is stopped. `~/.pce/holds` holds four real sifted
cards which are the operator's actual pending decisions. They are excellent fixtures — copy the store
and work against the copy. Never write to `~/.pce/holds`.

**Report what you decided and why**, in your completion report and in `CONTEXT.md`. Prefer falsifiers
that fail against the code as it stands and pass after; the defects in this brief all shipped behind
passing criteria. If satisfying this brief appears to require editing a frozen criterion, stop and
report which one and why.

**Dispatch order.** This brief runs *after* `briefs/overseer-conversation-brief.md` has landed. It
adds an update indicator to the queue page that brief redesigns, and both add record kinds to
`crates/core/src/hold_store.rs`. Running them together makes one overwrite the other's design.

**Your files.** New verbs in `src/main.rs`, a new module in `crates/core`, additions to
`crates/core/src/hold_store.rs` and `skills/overseer/SKILL.md`, and the indicator in
`crates/core/src/overseer_view.rs`.

## What this does not change

- The four attributed doors, the hold lifecycle, the card discipline, or the wake-and-exit model.
- The append-only store.
- Any frozen criterion. If satisfying this brief appears to require editing one, stop and report which
  criterion and why.

## Falsifiers worth pairing

- A hold classified as a binary defect produces a dispatched brief recorded in the store, and the
  counter stops reading zero.
- A repair built while a run is mid-dispatch does not reach the installed path until that dispatch
  completes.
- A second overseer pass over the same defect dispatches no second worker.
- An install records the digest and notifies every registered run.
- A repair that cannot install because the fleet never goes quiet is visible in the queue rather than
  silently pending.
